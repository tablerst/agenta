use super::*;

impl AgentaService {
    pub async fn create_task(&self, input: CreateTaskInput) -> AppResult<Task> {
        self.create_task_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_task_from(
        &self,
        origin: RequestOrigin,
        mut input: CreateTaskInput,
    ) -> AppResult<Task> {
        if input
            .created_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.created_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input.project.clone(),
            format!("Create task {}", input.title.trim()),
            actor_or_default(input.created_by.as_deref(), origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.create_task_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn create_child_task(&self, input: CreateChildTaskInput) -> AppResult<Task> {
        self.create_child_task_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_child_task_from(
        &self,
        origin: RequestOrigin,
        mut input: CreateChildTaskInput,
    ) -> AppResult<Task> {
        if input
            .created_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.created_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input.parent.clone(),
            format!(
                "Create child task {} under {}",
                input.title.trim(),
                input.parent.trim()
            ),
            actor_or_default(input.created_by.as_deref(), origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.create_child_task_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn get_task(&self, reference: &str) -> AppResult<Task> {
        self.store.get_task_by_ref(reference).await
    }

    pub async fn get_task_detail(&self, reference: &str) -> AppResult<TaskDetail> {
        let (
            task,
            note_count,
            attachment_count,
            latest_activity_at,
            parent_task_id,
            child_count,
            open_blocker_count,
            blocking_count,
        ) = self.store.get_task_with_stats_by_ref(reference).await?;
        Ok(task_detail_from_parts(
            task,
            note_count,
            attachment_count,
            latest_activity_at,
            parent_task_id,
            child_count,
            open_blocker_count,
            blocking_count,
        ))
    }

    pub async fn list_tasks(&self, query: TaskQuery) -> AppResult<Vec<Task>> {
        let (details, _, _) = self.collect_sorted_task_details(query).await?;
        Ok(details.into_iter().map(|detail| detail.task).collect())
    }

    pub async fn list_task_details(&self, query: TaskQuery) -> AppResult<Vec<TaskDetail>> {
        let (details, _, _) = self.collect_sorted_task_details(query).await?;
        Ok(details)
    }

    pub async fn list_task_details_page(
        &self,
        query: TaskQuery,
        page: PageRequest,
    ) -> AppResult<TaskListPageResult> {
        let (details, sort_by, sort_order) = self.collect_sorted_task_details(query).await?;
        let summary = build_task_list_summary(&details);
        let page = paginate_presorted_by_cursor(
            details,
            page,
            |detail| detail.task.created_at,
            |detail| detail.task.task_id,
        );
        Ok(TaskListPageResult {
            items: page.items,
            summary,
            limit: page.limit,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
            sort_by,
            sort_order,
        })
    }

    pub(super) async fn collect_sorted_task_details(
        &self,
        query: TaskQuery,
    ) -> AppResult<(Vec<TaskDetail>, TaskSortBy, SortOrder)> {
        let filter = self.resolve_task_filter(&query).await?;
        let mut details = self
            .store
            .list_tasks_with_stats(filter)
            .await?
            .into_iter()
            .map(
                |(
                    task,
                    note_count,
                    attachment_count,
                    latest_activity_at,
                    parent_task_id,
                    child_count,
                    open_blocker_count,
                    blocking_count,
                )| {
                    task_detail_from_parts(
                        task,
                        note_count,
                        attachment_count,
                        latest_activity_at,
                        parent_task_id,
                        child_count,
                        open_blocker_count,
                        blocking_count,
                    )
                },
            )
            .collect::<Vec<_>>();
        let sort_by = query
            .sort_by
            .unwrap_or_else(|| default_task_sort(query.version.as_deref(), &details));
        let sort_order = query.sort_order.unwrap_or_else(|| {
            if matches!(sort_by, TaskSortBy::TaskCode | TaskSortBy::Title) {
                SortOrder::Asc
            } else {
                SortOrder::Desc
            }
        });
        sort_task_details(&mut details, sort_by, sort_order);
        Ok((details, sort_by, sort_order))
    }

    pub async fn update_task(&self, reference: &str, input: UpdateTaskInput) -> AppResult<Task> {
        self.update_task_from(RequestOrigin::Cli, reference, input)
            .await
    }

    pub async fn update_task_from(
        &self,
        origin: RequestOrigin,
        reference: &str,
        mut input: UpdateTaskInput,
    ) -> AppResult<Task> {
        if input
            .updated_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.updated_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            reference.to_string(),
            format!("Update task {reference}"),
            actor_or_default(input.updated_by.as_deref(), origin),
            &ReferencedUpdatePayload {
                reference: reference.to_string(),
                input: input.clone(),
            },
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.update_task_internal(reference, input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn get_task_context(
        &self,
        task_ref: &str,
        recent_activity_limit: Option<usize>,
    ) -> AppResult<TaskContext> {
        let task = self.get_task_detail(task_ref).await?;
        let notes = self.list_notes(task_ref).await?;
        let attachments = self.list_attachments(task_ref).await?;
        let parent = match task.parent_task_id {
            Some(parent_task_id) => {
                let parent_relation = self
                    .store
                    .find_active_parent_relation(task.task.task_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::Conflict("task parent summary is out of sync".to_string())
                    })?;
                Some(
                    self.task_link_for_relation(parent_relation.relation_id, parent_task_id)
                        .await?,
                )
            }
            None => None,
        };
        let mut children = Vec::new();
        for relation in self
            .store
            .list_active_child_relations(task.task.task_id)
            .await?
        {
            children.push(
                self.task_link_for_relation(relation.relation_id, relation.target_task_id)
                    .await?,
            );
        }
        let mut blocked_by = Vec::new();
        for relation in self
            .store
            .list_active_blocker_relations(task.task.task_id)
            .await?
        {
            blocked_by.push(
                self.task_link_for_relation(relation.relation_id, relation.source_task_id)
                    .await?,
            );
        }
        let mut blocking = Vec::new();
        for relation in self
            .store
            .list_active_blocking_relations(task.task.task_id)
            .await?
        {
            blocking.push(
                self.task_link_for_relation(relation.relation_id, relation.target_task_id)
                    .await?,
            );
        }
        let recent_activities = self
            .list_task_activities_page(
                task_ref,
                PageRequest {
                    limit: Some(recent_activity_limit.unwrap_or(20).clamp(1, 50)),
                    cursor: None,
                },
            )
            .await?
            .items;
        Ok(TaskContext {
            task,
            notes,
            attachments,
            recent_activities,
            parent,
            children,
            blocked_by,
            blocking,
        })
    }

    pub(super) async fn create_task_internal(
        &self,
        input: CreateTaskInput,
        mode: ApprovalMode,
    ) -> AppResult<Task> {
        self.enforce("task.create", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let project = self
            .store
            .get_project_by_ref_tx(&mut tx, &input.project)
            .await?;
        let version_id = self
            .resolve_version_for_project_tx(&mut tx, project.project_id, input.version.as_deref())
            .await?;
        let now = OffsetDateTime::now_utc();
        let created_by = input
            .created_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        let mut task = Task {
            task_id: Uuid::new_v4(),
            project_id: project.project_id,
            version_id,
            task_code: clean_optional(input.task_code),
            task_kind: input.task_kind.unwrap_or_default(),
            title: require_non_empty(input.title, "task title")?,
            summary: clean_optional(input.summary),
            description: clean_optional(input.description),
            task_search_summary: String::new(),
            task_context_digest: String::new(),
            latest_note_summary: None,
            knowledge_status: KnowledgeStatus::Empty,
            status: input.status.unwrap_or_default(),
            priority: input.priority.unwrap_or_default(),
            created_by: created_by.clone(),
            updated_by: created_by,
            created_at: now,
            updated_at: now,
            closed_at: None,
        };
        task.closed_at = closed_at_for_status(task.status, now);
        task.task_search_summary = build_task_search_summary(
            task.task_code.as_deref(),
            task.task_kind,
            &task.title,
            task.summary.as_deref(),
            task.description.as_deref(),
        );
        task.task_context_digest = build_task_context_digest(&task);
        self.store.insert_task_tx(&mut tx, &task).await?;
        task.task_context_digest = self
            .refresh_task_context_digest_tx(&mut tx, task.task_id)
            .await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Task,
            task.task_id,
            SyncOperation::Create,
            &task,
            task.updated_at,
        )
        .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(task)
    }

    pub(super) async fn update_task_internal(
        &self,
        reference: &str,
        input: UpdateTaskInput,
        mode: ApprovalMode,
    ) -> AppResult<Task> {
        self.enforce("task.update", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let mut task = self.store.get_task_by_ref_tx(&mut tx, reference).await?;
        let previous_status = task.status;
        if let Some(version) = input.version {
            task.version_id = self
                .resolve_version_for_project_tx(&mut tx, task.project_id, Some(&version))
                .await?;
        }
        if let Some(title) = input.title {
            task.title = require_non_empty(title, "task title")?;
        }
        if let Some(task_code) = input.task_code {
            task.task_code = clean_optional(Some(task_code));
        }
        if let Some(task_kind) = input.task_kind {
            task.task_kind = task_kind;
        }
        if let Some(summary) = input.summary {
            task.summary = clean_optional(Some(summary));
        }
        if let Some(description) = input.description {
            task.description = clean_optional(Some(description));
        }
        if let Some(status) = input.status {
            task.status = status;
        }
        if let Some(priority) = input.priority {
            task.priority = priority;
        }
        task.updated_by = input
            .updated_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        task.updated_at = OffsetDateTime::now_utc();
        task.closed_at = closed_at_for_status(task.status, task.updated_at);
        task.task_search_summary = build_task_search_summary(
            task.task_code.as_deref(),
            task.task_kind,
            &task.title,
            task.summary.as_deref(),
            task.description.as_deref(),
        );
        task.task_context_digest = build_task_context_digest(&task);
        self.store.update_task_tx(&mut tx, &task).await?;
        task.task_context_digest = self
            .refresh_task_context_digest_tx(&mut tx, task.task_id)
            .await?;
        if previous_status != task.status {
            let content = format!("Status changed from {previous_status} to {}.", task.status);
            let activity = TaskActivity {
                activity_id: Uuid::new_v4(),
                task_id: task.task_id,
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
                created_by: task.updated_by.clone(),
                created_at: task.updated_at,
                metadata_json: json!({
                    "from_status": previous_status,
                    "to_status": task.status,
                }),
            };
            self.store.insert_activity_tx(&mut tx, &activity).await?;
            self.store
                .replace_activity_chunks_tx(&mut tx, &activity, &content)
                .await?;
        }
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Task,
            task.task_id,
            SyncOperation::Update,
            &task,
            task.updated_at,
        )
        .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(task)
    }

    pub(super) async fn task_detail_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
    ) -> AppResult<TaskDetail> {
        let (
            task,
            note_count,
            attachment_count,
            latest_activity_at,
            parent_task_id,
            child_count,
            open_blocker_count,
            blocking_count,
        ) = self
            .store
            .get_task_with_stats_by_ref_tx(tx, &task_id.to_string())
            .await?;
        Ok(task_detail_from_parts(
            task,
            note_count,
            attachment_count,
            latest_activity_at,
            parent_task_id,
            child_count,
            open_blocker_count,
            blocking_count,
        ))
    }

    pub(super) async fn refresh_task_context_digest_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
    ) -> AppResult<String> {
        let detail = self.task_detail_tx(tx, task_id).await?;
        let digest = build_task_context_digest_from_detail(&detail);
        self.store
            .update_task_context_digest_tx(tx, task_id, &digest)
            .await?;
        if self.search.vector_enabled() {
            self.store
                .upsert_search_index_job_tx(tx, task_id, None, OffsetDateTime::now_utc())
                .await?;
        }
        Ok(digest)
    }

    pub(super) async fn resolve_task_filter(&self, query: &TaskQuery) -> AppResult<TaskListFilter> {
        let project_ref = self
            .resolve_project_scope(
                query.project.as_deref(),
                query.version.as_deref(),
                query.all_projects,
            )
            .await?;
        Ok(TaskListFilter {
            project_id: match project_ref.as_deref() {
                Some(reference) => Some(self.store.get_project_by_ref(reference).await?.project_id),
                None => None,
            },
            version_id: match query.version.as_deref() {
                Some(reference) => Some(self.store.get_version_by_ref(reference).await?.version_id),
                None => None,
            },
            status: query.status,
            priority: None,
            knowledge_status: None,
            task_kind: query.task_kind,
            task_code_prefix: query.task_code_prefix.clone(),
            title_prefix: query.title_prefix.clone(),
        })
    }
}
