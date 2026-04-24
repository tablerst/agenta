use super::*;

impl AgentaService {
    pub async fn attach_child_task(&self, input: AttachChildTaskInput) -> AppResult<TaskRelation> {
        self.attach_child_task_from(RequestOrigin::Cli, input).await
    }

    pub async fn attach_child_task_from(
        &self,
        origin: RequestOrigin,
        mut input: AttachChildTaskInput,
    ) -> AppResult<TaskRelation> {
        if input
            .updated_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.updated_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input.child.clone(),
            format!(
                "Attach child task {} to parent {}",
                input.child.trim(),
                input.parent.trim()
            ),
            actor_or_default(input.updated_by.as_deref(), origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.attach_child_task_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn detach_child_task(&self, input: DetachChildTaskInput) -> AppResult<TaskRelation> {
        self.detach_child_task_from(RequestOrigin::Cli, input).await
    }

    pub async fn detach_child_task_from(
        &self,
        origin: RequestOrigin,
        mut input: DetachChildTaskInput,
    ) -> AppResult<TaskRelation> {
        if input
            .updated_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.updated_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input.child.clone(),
            format!(
                "Detach child task {} from parent {}",
                input.child.trim(),
                input.parent.trim()
            ),
            actor_or_default(input.updated_by.as_deref(), origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.detach_child_task_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn add_task_blocker(&self, input: AddTaskBlockerInput) -> AppResult<TaskRelation> {
        self.add_task_blocker_from(RequestOrigin::Cli, input).await
    }

    pub async fn add_task_blocker_from(
        &self,
        origin: RequestOrigin,
        mut input: AddTaskBlockerInput,
    ) -> AppResult<TaskRelation> {
        if input
            .updated_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.updated_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input.blocked.clone(),
            format!(
                "Block task {} with task {}",
                input.blocked.trim(),
                input.blocker.trim()
            ),
            actor_or_default(input.updated_by.as_deref(), origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.add_task_blocker_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn resolve_task_blocker(
        &self,
        input: ResolveTaskBlockerInput,
    ) -> AppResult<TaskRelation> {
        self.resolve_task_blocker_from(RequestOrigin::Cli, input)
            .await
    }

    pub async fn resolve_task_blocker_from(
        &self,
        origin: RequestOrigin,
        mut input: ResolveTaskBlockerInput,
    ) -> AppResult<TaskRelation> {
        if input
            .updated_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.updated_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input.task.clone(),
            format!("Resolve blocker for task {}", input.task.trim()),
            actor_or_default(input.updated_by.as_deref(), origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.resolve_task_blocker_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub(super) async fn create_child_task_internal(
        &self,
        input: CreateChildTaskInput,
        mode: ApprovalMode,
    ) -> AppResult<Task> {
        self.enforce("task.create_child", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let parent = self
            .store
            .get_task_by_ref_tx(&mut tx, &input.parent)
            .await?;
        let version_id = match input.version.as_deref() {
            Some(reference) => {
                self.resolve_version_for_project_tx(&mut tx, parent.project_id, Some(reference))
                    .await?
            }
            None => parent.version_id,
        };
        let now = OffsetDateTime::now_utc();
        let created_by = input
            .created_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        let mut task = Task {
            task_id: Uuid::new_v4(),
            project_id: parent.project_id,
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
            updated_by: created_by.clone(),
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

        let relation = TaskRelation {
            relation_id: Uuid::new_v4(),
            kind: TaskRelationKind::ParentChild,
            source_task_id: parent.task_id,
            target_task_id: task.task_id,
            status: TaskRelationStatus::Active,
            created_by: created_by.clone(),
            updated_by: created_by.clone(),
            created_at: now,
            updated_at: now,
            resolved_at: None,
        };
        self.store
            .insert_task_relation_tx(&mut tx, &relation)
            .await?;

        task.task_context_digest = self
            .refresh_task_context_digest_tx(&mut tx, task.task_id)
            .await?;
        self.touch_task_for_relation_change_tx(&mut tx, parent.task_id, &created_by, now, None)
            .await?;
        self.append_system_activity_tx(
            &mut tx,
            parent.task_id,
            &format!("Attached child task {}.", task.title),
            &created_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "child_task_id": task.task_id,
            }),
        )
        .await?;
        self.append_system_activity_tx(
            &mut tx,
            task.task_id,
            &format!("Attached to parent task {}.", parent.title),
            &created_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "parent_task_id": parent.task_id,
            }),
        )
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
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::TaskRelation,
            relation.relation_id,
            SyncOperation::Create,
            &relation,
            relation.updated_at,
        )
        .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(task)
    }

    pub(super) async fn attach_child_task_internal(
        &self,
        input: AttachChildTaskInput,
        mode: ApprovalMode,
    ) -> AppResult<TaskRelation> {
        self.enforce("task.attach_child", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let parent = self
            .store
            .get_task_by_ref_tx(&mut tx, &input.parent)
            .await?;
        let child = self.store.get_task_by_ref_tx(&mut tx, &input.child).await?;
        if parent.task_id == child.task_id {
            return Err(AppError::Conflict(
                "parent and child task must be different".to_string(),
            ));
        }
        if parent.project_id != child.project_id {
            return Err(AppError::Conflict(
                "parent and child task must belong to the same project".to_string(),
            ));
        }
        if self
            .store
            .find_active_relation_tx(
                &mut tx,
                TaskRelationKind::ParentChild,
                parent.task_id,
                child.task_id,
            )
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(
                "child task is already attached to this parent".to_string(),
            ));
        }
        if self
            .store
            .find_active_parent_relation_tx(&mut tx, child.task_id)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(
                "child task already has an active parent".to_string(),
            ));
        }
        if self
            .store
            .has_active_parent_path_tx(&mut tx, child.task_id, parent.task_id)
            .await?
        {
            return Err(AppError::Conflict(
                "attaching this child would create a parent cycle".to_string(),
            ));
        }
        let now = OffsetDateTime::now_utc();
        let updated_by = input
            .updated_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        let relation = TaskRelation {
            relation_id: Uuid::new_v4(),
            kind: TaskRelationKind::ParentChild,
            source_task_id: parent.task_id,
            target_task_id: child.task_id,
            status: TaskRelationStatus::Active,
            created_by: updated_by.clone(),
            updated_by: updated_by.clone(),
            created_at: now,
            updated_at: now,
            resolved_at: None,
        };
        self.store
            .insert_task_relation_tx(&mut tx, &relation)
            .await?;
        self.touch_task_for_relation_change_tx(&mut tx, parent.task_id, &updated_by, now, None)
            .await?;
        self.touch_task_for_relation_change_tx(&mut tx, child.task_id, &updated_by, now, None)
            .await?;
        self.append_system_activity_tx(
            &mut tx,
            parent.task_id,
            &format!("Attached existing child task {}.", child.title),
            &updated_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "child_task_id": child.task_id,
            }),
        )
        .await?;
        self.append_system_activity_tx(
            &mut tx,
            child.task_id,
            &format!("Attached to parent task {}.", parent.title),
            &updated_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "parent_task_id": parent.task_id,
            }),
        )
        .await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::TaskRelation,
            relation.relation_id,
            SyncOperation::Create,
            &relation,
            relation.updated_at,
        )
        .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(relation)
    }

    pub(super) async fn detach_child_task_internal(
        &self,
        input: DetachChildTaskInput,
        mode: ApprovalMode,
    ) -> AppResult<TaskRelation> {
        self.enforce("task.detach_child", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let parent = self
            .store
            .get_task_by_ref_tx(&mut tx, &input.parent)
            .await?;
        let child = self.store.get_task_by_ref_tx(&mut tx, &input.child).await?;
        let mut relation = self
            .store
            .find_active_relation_tx(
                &mut tx,
                TaskRelationKind::ParentChild,
                parent.task_id,
                child.task_id,
            )
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "task_relation".to_string(),
                reference: format!("parent_child:{}->{}", parent.task_id, child.task_id),
            })?;
        let now = OffsetDateTime::now_utc();
        let updated_by = input
            .updated_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        relation.status = TaskRelationStatus::Resolved;
        relation.updated_by = updated_by.clone();
        relation.updated_at = now;
        relation.resolved_at = Some(now);
        self.store
            .update_task_relation_tx(&mut tx, &relation)
            .await?;
        self.touch_task_for_relation_change_tx(&mut tx, parent.task_id, &updated_by, now, None)
            .await?;
        self.touch_task_for_relation_change_tx(&mut tx, child.task_id, &updated_by, now, None)
            .await?;
        self.append_system_activity_tx(
            &mut tx,
            parent.task_id,
            &format!("Detached child task {}.", child.title),
            &updated_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "child_task_id": child.task_id,
                "status": relation.status,
            }),
        )
        .await?;
        self.append_system_activity_tx(
            &mut tx,
            child.task_id,
            &format!("Detached from parent task {}.", parent.title),
            &updated_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "parent_task_id": parent.task_id,
                "status": relation.status,
            }),
        )
        .await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::TaskRelation,
            relation.relation_id,
            SyncOperation::Update,
            &relation,
            relation.updated_at,
        )
        .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(relation)
    }

    pub(super) async fn add_task_blocker_internal(
        &self,
        input: AddTaskBlockerInput,
        mode: ApprovalMode,
    ) -> AppResult<TaskRelation> {
        self.enforce("task.add_blocker", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let blocker = self
            .store
            .get_task_by_ref_tx(&mut tx, &input.blocker)
            .await?;
        let blocked = self
            .store
            .get_task_by_ref_tx(&mut tx, &input.blocked)
            .await?;
        if blocker.task_id == blocked.task_id {
            return Err(AppError::Conflict(
                "blocker and blocked task must be different".to_string(),
            ));
        }
        if blocker.project_id != blocked.project_id {
            return Err(AppError::Conflict(
                "blocker and blocked task must belong to the same project".to_string(),
            ));
        }
        if self
            .store
            .find_active_relation_tx(
                &mut tx,
                TaskRelationKind::Blocks,
                blocker.task_id,
                blocked.task_id,
            )
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(
                "this blocker relation already exists".to_string(),
            ));
        }
        let now = OffsetDateTime::now_utc();
        let updated_by = input
            .updated_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        let relation = TaskRelation {
            relation_id: Uuid::new_v4(),
            kind: TaskRelationKind::Blocks,
            source_task_id: blocker.task_id,
            target_task_id: blocked.task_id,
            status: TaskRelationStatus::Active,
            created_by: updated_by.clone(),
            updated_by: updated_by.clone(),
            created_at: now,
            updated_at: now,
            resolved_at: None,
        };
        self.store
            .insert_task_relation_tx(&mut tx, &relation)
            .await?;
        self.touch_task_for_relation_change_tx(&mut tx, blocker.task_id, &updated_by, now, None)
            .await?;
        let blocked_status = (!matches!(blocked.status, TaskStatus::Done | TaskStatus::Cancelled))
            .then_some(TaskStatus::Blocked);
        self.touch_task_for_relation_change_tx(
            &mut tx,
            blocked.task_id,
            &updated_by,
            now,
            blocked_status,
        )
        .await?;
        self.append_system_activity_tx(
            &mut tx,
            blocker.task_id,
            &format!("Task {} is now blocked by this task.", blocked.title),
            &updated_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "blocked_task_id": blocked.task_id,
            }),
        )
        .await?;
        self.append_system_activity_tx(
            &mut tx,
            blocked.task_id,
            &format!("Blocked by task {}.", blocker.title),
            &updated_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "blocker_task_id": blocker.task_id,
            }),
        )
        .await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::TaskRelation,
            relation.relation_id,
            SyncOperation::Create,
            &relation,
            relation.updated_at,
        )
        .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(relation)
    }

    pub(super) async fn resolve_task_blocker_internal(
        &self,
        input: ResolveTaskBlockerInput,
        mode: ApprovalMode,
    ) -> AppResult<TaskRelation> {
        self.enforce("task.resolve_blocker", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let blocked = self.store.get_task_by_ref_tx(&mut tx, &input.task).await?;
        let mut relation = if let Some(relation_id) = input.relation_id.as_deref() {
            let relation = self
                .store
                .get_task_relation_by_ref_tx(&mut tx, relation_id)
                .await?;
            if relation.kind != TaskRelationKind::Blocks
                || relation.target_task_id != blocked.task_id
            {
                return Err(AppError::Conflict(
                    "relation_id must point to an active blocker for the selected task".to_string(),
                ));
            }
            relation
        } else {
            let blocker_ref = input.blocker.as_deref().ok_or_else(|| {
                AppError::InvalidArguments(
                    "either blocker or relation_id must be provided".to_string(),
                )
            })?;
            let blocker = self.store.get_task_by_ref_tx(&mut tx, blocker_ref).await?;
            self.store
                .find_active_relation_tx(
                    &mut tx,
                    TaskRelationKind::Blocks,
                    blocker.task_id,
                    blocked.task_id,
                )
                .await?
                .ok_or_else(|| AppError::NotFound {
                    entity: "task_relation".to_string(),
                    reference: format!("blocks:{}->{}", blocker.task_id, blocked.task_id),
                })?
        };
        if relation.status != TaskRelationStatus::Active {
            return Err(AppError::Conflict(
                "only active blocker relations can be resolved".to_string(),
            ));
        }
        let blocker = self
            .store
            .get_task_by_ref_tx(&mut tx, &relation.source_task_id.to_string())
            .await?;
        let now = OffsetDateTime::now_utc();
        let updated_by = input
            .updated_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        relation.status = TaskRelationStatus::Resolved;
        relation.updated_by = updated_by.clone();
        relation.updated_at = now;
        relation.resolved_at = Some(now);
        self.store
            .update_task_relation_tx(&mut tx, &relation)
            .await?;
        self.touch_task_for_relation_change_tx(&mut tx, blocker.task_id, &updated_by, now, None)
            .await?;
        let (_, _, _, _, _, _, remaining_open_blockers, _) = self
            .store
            .get_task_with_stats_by_ref_tx(&mut tx, &blocked.task_id.to_string())
            .await?;
        let restore_status = (blocked.status == TaskStatus::Blocked
            && remaining_open_blockers == 0)
            .then_some(TaskStatus::Ready);
        self.touch_task_for_relation_change_tx(
            &mut tx,
            blocked.task_id,
            &updated_by,
            now,
            restore_status,
        )
        .await?;
        self.append_system_activity_tx(
            &mut tx,
            blocker.task_id,
            &format!("Resolved blocker for task {}.", blocked.title),
            &updated_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "blocked_task_id": blocked.task_id,
                "status": relation.status,
            }),
        )
        .await?;
        self.append_system_activity_tx(
            &mut tx,
            blocked.task_id,
            &format!("Unblocked from task {}.", blocker.title),
            &updated_by,
            now,
            json!({
                "relation_id": relation.relation_id,
                "kind": relation.kind,
                "blocker_task_id": blocker.task_id,
                "status": relation.status,
            }),
        )
        .await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::TaskRelation,
            relation.relation_id,
            SyncOperation::Update,
            &relation,
            relation.updated_at,
        )
        .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(relation)
    }

    pub(super) async fn touch_task_for_relation_change_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
        updated_by: &str,
        updated_at: OffsetDateTime,
        next_status: Option<TaskStatus>,
    ) -> AppResult<Task> {
        let mut task = self
            .store
            .get_task_by_ref_tx(tx, &task_id.to_string())
            .await?;
        let previous_status = task.status;
        if let Some(status) = next_status {
            task.status = status;
        }
        task.updated_by = updated_by.to_string();
        task.updated_at = updated_at;
        task.closed_at = closed_at_for_status(task.status, updated_at);
        task.task_context_digest = build_task_context_digest(&task);
        self.store.update_task_tx(tx, &task).await?;
        task.task_context_digest = self
            .refresh_task_context_digest_tx(tx, task.task_id)
            .await?;
        if previous_status != task.status {
            self.append_status_change_activity_tx(
                tx,
                task.task_id,
                previous_status,
                task.status,
                updated_by,
                updated_at,
            )
            .await?;
        }
        self.enqueue_sync_mutation_tx(
            tx,
            SyncEntityKind::Task,
            task.task_id,
            SyncOperation::Update,
            &task,
            updated_at,
        )
        .await?;
        Ok(task)
    }

    pub(super) async fn task_link_for_relation(
        &self,
        relation_id: Uuid,
        task_id: Uuid,
    ) -> AppResult<TaskLink> {
        let detail = self.get_task_detail(&task_id.to_string()).await?;
        Ok(TaskLink {
            relation_id,
            task_id: detail.task.task_id,
            title: detail.task.title.clone(),
            status: detail.task.status,
            priority: detail.task.priority,
            ready_to_start: detail.ready_to_start,
        })
    }
}
