use super::*;

impl AgentaService {
    pub async fn create_note(&self, input: CreateNoteInput) -> AppResult<TaskActivity> {
        self.create_note_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_note_from(
        &self,
        origin: RequestOrigin,
        mut input: CreateNoteInput,
    ) -> AppResult<TaskActivity> {
        if input
            .created_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.created_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input.task.clone(),
            format!("Add note to task {}", input.task.trim()),
            actor_or_default(input.created_by.as_deref(), origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.create_note_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn list_task_activities(&self, task_ref: &str) -> AppResult<Vec<TaskActivity>> {
        let task = self.store.get_task_by_ref(task_ref).await?;
        self.store.list_task_activities(task.task_id).await
    }

    pub async fn list_task_activities_page(
        &self,
        task_ref: &str,
        page: PageRequest,
    ) -> AppResult<PageResult<TaskActivity>> {
        let activities = self.list_task_activities(task_ref).await?;
        Ok(paginate_by_created_at(
            activities,
            page,
            |activity| activity.created_at,
            |activity| activity.activity_id,
        ))
    }

    pub async fn list_notes(&self, task_ref: &str) -> AppResult<Vec<TaskActivity>> {
        let activities = self.list_task_activities(task_ref).await?;
        Ok(activities
            .into_iter()
            .filter(|activity| activity.kind == TaskActivityKind::Note)
            .collect())
    }

    pub async fn list_notes_page(
        &self,
        task_ref: &str,
        page: PageRequest,
    ) -> AppResult<PageResult<TaskActivity>> {
        let notes = self.list_notes(task_ref).await?;
        Ok(paginate_by_created_at(
            notes,
            page,
            |activity| activity.created_at,
            |activity| activity.activity_id,
        ))
    }

    pub(super) async fn create_note_internal(
        &self,
        input: CreateNoteInput,
        mode: ApprovalMode,
    ) -> AppResult<TaskActivity> {
        self.enforce("note.create", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let task = self.store.get_task_by_ref_tx(&mut tx, &input.task).await?;
        let now = OffsetDateTime::now_utc();
        let content = require_non_empty(input.content, "note content")?;
        let note_kind = input.note_kind.unwrap_or_default();
        let activity = TaskActivity {
            activity_id: Uuid::new_v4(),
            task_id: task.task_id,
            kind: TaskActivityKind::Note,
            content: content.clone(),
            activity_search_summary: build_activity_search_summary(
                TaskActivityKind::Note,
                &content,
            ),
            activity_search_text: build_activity_search_text(TaskActivityKind::Note, &content),
            created_by: input
                .created_by
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "cli".to_string()),
            created_at: now,
            metadata_json: json!({
                "note_kind": note_kind,
            }),
        };
        self.store.insert_activity_tx(&mut tx, &activity).await?;
        self.store
            .replace_activity_chunks_tx(&mut tx, &activity, &content)
            .await?;
        self.refresh_task_note_rollup_tx(&mut tx, task.task_id)
            .await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Note,
            activity.activity_id,
            SyncOperation::Create,
            &activity,
            activity.created_at,
        )
        .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(activity)
    }

    pub(super) async fn refresh_task_note_rollup_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
    ) -> AppResult<(Option<String>, KnowledgeStatus, String)> {
        let row = sqlx::query(
            r#"
            SELECT
                (
                    SELECT ta.activity_search_summary
                    FROM task_activities ta
                    WHERE ta.task_id = ?
                      AND ta.kind = ?
                    ORDER BY ta.created_at DESC, ta.activity_id DESC
                    LIMIT 1
                ) AS latest_note_summary,
                EXISTS(
                    SELECT 1
                    FROM task_activities ta
                    WHERE ta.task_id = ?
                      AND ta.kind = ?
                ) AS has_note,
                EXISTS(
                    SELECT 1
                    FROM task_activities ta
                    WHERE ta.task_id = ?
                      AND ta.kind = ?
                      AND json_extract(ta.metadata_json, '$.note_kind') = ?
                ) AS has_conclusion
            "#,
        )
        .bind(task_id.to_string())
        .bind(TaskActivityKind::Note.to_string())
        .bind(task_id.to_string())
        .bind(TaskActivityKind::Note.to_string())
        .bind(task_id.to_string())
        .bind(TaskActivityKind::Note.to_string())
        .bind(NoteKind::Conclusion.to_string())
        .fetch_one(&mut **tx)
        .await?;

        let latest_note_summary = row.get::<Option<String>, _>("latest_note_summary");
        let has_note = row.get::<i64, _>("has_note") > 0;
        let has_conclusion = row.get::<i64, _>("has_conclusion") > 0;
        let knowledge_status = if has_conclusion {
            KnowledgeStatus::Reusable
        } else if has_note {
            KnowledgeStatus::Working
        } else {
            KnowledgeStatus::Empty
        };
        self.store
            .update_task_note_rollup_tx(
                tx,
                task_id,
                latest_note_summary.as_deref(),
                knowledge_status,
            )
            .await?;
        let digest = self.refresh_task_context_digest_tx(tx, task_id).await?;
        Ok((latest_note_summary, knowledge_status, digest))
    }

    pub(super) async fn append_status_change_activity_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
        previous_status: TaskStatus,
        next_status: TaskStatus,
        created_by: &str,
        created_at: OffsetDateTime,
    ) -> AppResult<()> {
        let content = format!("Status changed from {previous_status} to {next_status}.");
        let activity = TaskActivity {
            activity_id: Uuid::new_v4(),
            task_id,
            kind: TaskActivityKind::StatusChange,
            content: content.clone(),
            activity_search_summary: build_activity_search_summary(
                TaskActivityKind::StatusChange,
                &content,
            ),
            activity_search_text: build_activity_search_text(
                TaskActivityKind::StatusChange,
                &content,
            ),
            created_by: created_by.to_string(),
            created_at,
            metadata_json: json!({
                "from_status": previous_status,
                "to_status": next_status,
            }),
        };
        self.store.insert_activity_tx(tx, &activity).await?;
        self.store
            .replace_activity_chunks_tx(tx, &activity, &content)
            .await?;
        Ok(())
    }

    pub(super) async fn append_system_activity_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
        content: &str,
        created_by: &str,
        created_at: OffsetDateTime,
        metadata_json: Value,
    ) -> AppResult<()> {
        let activity = TaskActivity {
            activity_id: Uuid::new_v4(),
            task_id,
            kind: TaskActivityKind::System,
            content: content.to_string(),
            activity_search_summary: build_activity_search_summary(
                TaskActivityKind::System,
                content,
            ),
            activity_search_text: build_activity_search_text(TaskActivityKind::System, content),
            created_by: created_by.to_string(),
            created_at,
            metadata_json,
        };
        self.store.insert_activity_tx(tx, &activity).await?;
        self.store
            .replace_activity_chunks_tx(tx, &activity, content)
            .await?;
        Ok(())
    }
}
