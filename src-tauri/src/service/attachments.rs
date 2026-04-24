use super::*;

impl AgentaService {
    pub async fn create_attachment(&self, input: CreateAttachmentInput) -> AppResult<Attachment> {
        self.create_attachment_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_attachment_from(
        &self,
        origin: RequestOrigin,
        mut input: CreateAttachmentInput,
    ) -> AppResult<Attachment> {
        if input
            .created_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.created_by = Some(origin.fallback_actor().to_string());
        }
        let summary = input
            .summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                input
                    .path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("attachment")
            })
            .to_string();
        let approval = self.approval_seed(
            origin,
            input.task.clone(),
            format!("Add attachment {summary} to task {}", input.task.trim()),
            actor_or_default(input.created_by.as_deref(), origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.create_attachment_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn list_attachments(&self, task_ref: &str) -> AppResult<Vec<Attachment>> {
        let task = self.store.get_task_by_ref(task_ref).await?;
        self.store.list_attachments(task.task_id).await
    }

    pub async fn list_attachments_page(
        &self,
        task_ref: &str,
        page: PageRequest,
    ) -> AppResult<PageResult<Attachment>> {
        let attachments = self.list_attachments(task_ref).await?;
        Ok(paginate_by_created_at(
            attachments,
            page,
            |attachment| attachment.created_at,
            |attachment| attachment.attachment_id,
        ))
    }

    pub async fn get_attachment(&self, reference: &str) -> AppResult<Attachment> {
        self.store.get_attachment_by_ref(reference).await
    }

    pub(super) async fn create_attachment_internal(
        &self,
        input: CreateAttachmentInput,
        mode: ApprovalMode,
    ) -> AppResult<Attachment> {
        self.enforce("attachment.create", mode).await?;
        let task = self.store.get_task_by_ref(&input.task).await?;
        let now = OffsetDateTime::now_utc();
        let attachment_id = Uuid::new_v4();
        let stored = self
            .store
            .persist_attachment_file(task.task_id, attachment_id, &input.path)
            .await?;
        let summary = input
            .summary
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| stored.original_filename.clone());
        let created_by = input
            .created_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        let attachment = Attachment {
            attachment_id,
            task_id: task.task_id,
            kind: input.kind.unwrap_or_default(),
            mime: stored.mime.clone(),
            original_filename: stored.original_filename.clone(),
            original_path: stored.original_path.clone(),
            storage_path: stored.storage_path.clone(),
            sha256: stored.sha256,
            size_bytes: stored.size_bytes,
            summary: summary.clone(),
            created_by: created_by.clone(),
            created_at: now,
        };
        let attachment_search_body = stored
            .extracted_search_text
            .as_deref()
            .map(|text| format!("{summary}\n{text}"))
            .unwrap_or_else(|| summary.clone());
        let activity = TaskActivity {
            activity_id: Uuid::new_v4(),
            task_id: task.task_id,
            kind: TaskActivityKind::AttachmentRef,
            content: summary.clone(),
            activity_search_summary: build_activity_search_summary(
                TaskActivityKind::AttachmentRef,
                &summary,
            ),
            activity_search_text: build_activity_search_text(
                TaskActivityKind::AttachmentRef,
                &attachment_search_body,
            ),
            created_by,
            created_at: now,
            metadata_json: json!({
                "attachment_id": attachment.attachment_id,
                "storage_path": attachment.storage_path,
            }),
        };
        let mut tx = self.store.pool.begin().await?;
        let result = async {
            let _ = self.store.get_task_by_ref_tx(&mut tx, &input.task).await?;
            self.store
                .insert_attachment_tx(&mut tx, &attachment)
                .await?;
            self.store.insert_activity_tx(&mut tx, &activity).await?;
            self.store
                .replace_activity_chunks_tx(&mut tx, &activity, &attachment_search_body)
                .await?;
            self.enqueue_sync_mutation_tx(
                &mut tx,
                SyncEntityKind::Attachment,
                attachment.attachment_id,
                SyncOperation::Create,
                &attachment,
                attachment.created_at,
            )
            .await?;
            self.queue_task_search_jobs_tx(&mut tx, &[attachment.task_id])
                .await?;
            tx.commit().await?;
            Ok::<(), AppError>(())
        }
        .await;

        if let Err(error) = result {
            let cleanup_path = self.store.attachments_dir.join(&stored.storage_path);
            let _ = fs::remove_file(cleanup_path).await;
            return Err(error);
        }

        self.search.trigger_index_worker(self.store.clone());
        Ok(attachment)
    }
}
