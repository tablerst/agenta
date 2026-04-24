use super::*;

impl AgentaService {
    pub async fn sync_status(&self) -> AppResult<SyncStatusSummary> {
        let Some(remote) = self.sync_remote() else {
            return Ok(SyncStatusSummary {
                enabled: self.sync.enabled,
                mode: self.sync.mode,
                remote: None,
                pending_outbox_count: 0,
                oldest_pending_at: None,
                checkpoints: SyncCheckpointStatus {
                    pull: None,
                    push_ack: None,
                },
            });
        };

        let pull_checkpoint = self
            .store
            .get_sync_checkpoint(&remote.id, SyncCheckpointKind::Pull)
            .await?;
        let push_ack_checkpoint = self
            .store
            .get_sync_checkpoint(&remote.id, SyncCheckpointKind::PushAck)
            .await?;

        Ok(SyncStatusSummary {
            enabled: self.sync.enabled,
            mode: self.sync.mode,
            remote: Some(self.sync_remote_status(remote)?),
            pending_outbox_count: self.store.pending_sync_outbox_count(&remote.id).await?,
            oldest_pending_at: self.store.oldest_pending_sync_outbox_at(&remote.id).await?,
            checkpoints: SyncCheckpointStatus {
                pull: pull_checkpoint.map(|checkpoint| checkpoint.checkpoint_value),
                push_ack: push_ack_checkpoint.map(|checkpoint| checkpoint.checkpoint_value),
            },
        })
    }

    pub async fn sync_postgres_smoke_check(&self) -> AppResult<()> {
        let remote = self.connect_remote_postgres().await?;
        remote.smoke_check().await?;
        remote.close().await;
        Ok(())
    }

    pub async fn sync_backfill(&self, limit: Option<usize>) -> AppResult<SyncBackfillSummary> {
        let remote = self
            .sync_remote()
            .ok_or_else(|| AppError::Conflict("sync is not enabled".to_string()))?;
        let _write_guard = self.write_queue.lock().await;
        let max_to_queue = limit.unwrap_or(1000).clamp(1, 10_000);
        let mut summary = SyncBackfillSummary::default();

        for project in self.list_projects().await? {
            let queued = self
                .backfill_entity_if_untracked(
                    &remote.id,
                    SyncEntityKind::Project,
                    project.project_id,
                    &project,
                    project.updated_at,
                )
                .await?;
            summary.scanned += 1;
            if queued {
                summary.queued += 1;
                summary.queued_projects += 1;
                if summary.queued >= max_to_queue {
                    return Ok(summary);
                }
            } else {
                summary.skipped += 1;
            }
        }

        for version in self.list_versions(None).await? {
            let queued = self
                .backfill_entity_if_untracked(
                    &remote.id,
                    SyncEntityKind::Version,
                    version.version_id,
                    &version,
                    version.updated_at,
                )
                .await?;
            summary.scanned += 1;
            if queued {
                summary.queued += 1;
                summary.queued_versions += 1;
                if summary.queued >= max_to_queue {
                    return Ok(summary);
                }
            } else {
                summary.skipped += 1;
            }
        }

        let tasks = self
            .list_tasks(TaskQuery {
                all_projects: true,
                ..TaskQuery::default()
            })
            .await?;
        for task in &tasks {
            let queued = self
                .backfill_entity_if_untracked(
                    &remote.id,
                    SyncEntityKind::Task,
                    task.task_id,
                    task,
                    task.updated_at,
                )
                .await?;
            summary.scanned += 1;
            if queued {
                summary.queued += 1;
                summary.queued_tasks += 1;
                if summary.queued >= max_to_queue {
                    return Ok(summary);
                }
            } else {
                summary.skipped += 1;
            }
        }

        for relation in self.store.list_task_relations().await? {
            let queued = self
                .backfill_entity_if_untracked(
                    &remote.id,
                    SyncEntityKind::TaskRelation,
                    relation.relation_id,
                    &relation,
                    relation.updated_at,
                )
                .await?;
            summary.scanned += 1;
            if queued {
                summary.queued += 1;
                if summary.queued >= max_to_queue {
                    return Ok(summary);
                }
            } else {
                summary.skipped += 1;
            }
        }

        for task in &tasks {
            for note in self.list_notes(&task.task_id.to_string()).await? {
                let queued = self
                    .backfill_entity_if_untracked(
                        &remote.id,
                        SyncEntityKind::Note,
                        note.activity_id,
                        &note,
                        note.created_at,
                    )
                    .await?;
                summary.scanned += 1;
                if queued {
                    summary.queued += 1;
                    summary.queued_notes += 1;
                    if summary.queued >= max_to_queue {
                        return Ok(summary);
                    }
                } else {
                    summary.skipped += 1;
                }
            }

            for attachment in self.list_attachments(&task.task_id.to_string()).await? {
                let queued = self
                    .backfill_entity_if_untracked(
                        &remote.id,
                        SyncEntityKind::Attachment,
                        attachment.attachment_id,
                        &attachment,
                        attachment.created_at,
                    )
                    .await?;
                summary.scanned += 1;
                if queued {
                    summary.queued += 1;
                    summary.queued_attachments += 1;
                    if summary.queued >= max_to_queue {
                        return Ok(summary);
                    }
                } else {
                    summary.skipped += 1;
                }
            }
        }

        Ok(summary)
    }

    pub async fn sync_push(&self, limit: Option<usize>) -> AppResult<SyncPushSummary> {
        let remote_config = self
            .sync_remote()
            .ok_or_else(|| AppError::Conflict("sync is not enabled".to_string()))?;
        let remote = self.connect_remote_postgres().await?;
        remote.ensure_schema().await?;

        let entries = self
            .store
            .list_sync_outbox_for_delivery(&remote_config.id, limit)
            .await?;
        let mut summary = SyncPushSummary {
            attempted: entries.len(),
            pushed: 0,
            failed: 0,
            last_remote_mutation_id: None,
        };

        for entry in entries {
            match remote
                .push_outbox_entry(&remote_config.id, &entry, &self.store.attachments_dir)
                .await
            {
                Ok(ack) => {
                    let _write_guard = self.write_queue.lock().await;
                    self.store
                        .mark_sync_outbox_acked(entry.mutation_id, ack.acked_at)
                        .await?;
                    self.store
                        .mark_sync_entity_acked(
                            entry.entity_kind,
                            entry.local_id,
                            &remote_config.id,
                            &ack.remote_entity_id,
                            entry.mutation_id,
                            ack.acked_at,
                        )
                        .await?;
                    self.store
                        .upsert_sync_checkpoint(
                            &remote_config.id,
                            SyncCheckpointKind::PushAck,
                            &ack.remote_mutation_id.to_string(),
                            ack.acked_at,
                        )
                        .await?;
                    summary.pushed += 1;
                    summary.last_remote_mutation_id = Some(ack.remote_mutation_id);
                }
                Err(error) => {
                    let failed_at = OffsetDateTime::now_utc();
                    let _write_guard = self.write_queue.lock().await;
                    self.store
                        .mark_sync_outbox_failed(entry.mutation_id, failed_at, &error.to_string())
                        .await?;
                    summary.failed += 1;
                }
            }
        }

        remote.close().await;
        Ok(summary)
    }

    pub async fn sync_pull(&self, limit: Option<usize>) -> AppResult<SyncPullSummary> {
        let remote_config = self
            .sync_remote()
            .ok_or_else(|| AppError::Conflict("sync is not enabled".to_string()))?;
        let remote = self.connect_remote_postgres().await?;
        remote.ensure_schema().await?;

        let after_remote_mutation_id = self
            .store
            .get_sync_checkpoint(&remote_config.id, SyncCheckpointKind::Pull)
            .await?
            .and_then(|checkpoint| checkpoint.checkpoint_value.parse::<i64>().ok());
        let limit = limit.unwrap_or(50).clamp(1, 200);
        let mut mutations = remote
            .pull_mutations(&remote_config.id, after_remote_mutation_id, limit)
            .await?;
        let checkpoint = mutations
            .iter()
            .max_by_key(|mutation| mutation.remote_mutation_id)
            .map(|mutation| (mutation.remote_mutation_id, mutation.created_at));
        mutations.sort_by(compare_remote_mutations_for_apply);
        let mut summary = SyncPullSummary {
            fetched: mutations.len(),
            applied: 0,
            skipped: 0,
            last_remote_mutation_id: None,
        };

        let _write_guard = self.write_queue.lock().await;
        for mutation in &mutations {
            let applied = self
                .apply_remote_mutation(&remote_config.id, mutation)
                .await?;
            if applied {
                summary.applied += 1;
            } else {
                summary.skipped += 1;
            }
            summary.last_remote_mutation_id = Some(mutation.remote_mutation_id);
        }
        if let Some((remote_mutation_id, created_at)) = checkpoint {
            self.store
                .upsert_sync_checkpoint(
                    &remote_config.id,
                    SyncCheckpointKind::Pull,
                    &remote_mutation_id.to_string(),
                    created_at,
                )
                .await?;
            summary.last_remote_mutation_id = Some(remote_mutation_id);
        }

        remote.close().await;
        if summary.applied > 0 {
            self.search.trigger_index_worker(self.store.clone());
        }
        Ok(summary)
    }

    pub async fn list_sync_outbox(
        &self,
        limit: Option<usize>,
    ) -> AppResult<Vec<SyncOutboxListItem>> {
        let entries = self.store.list_sync_outbox(limit).await?;
        Ok(entries
            .into_iter()
            .map(|entry| SyncOutboxListItem {
                mutation_id: entry.mutation_id,
                entity_kind: entry.entity_kind,
                local_id: entry.local_id,
                operation: entry.operation,
                local_version: entry.local_version,
                status: entry.status,
                created_at: entry.created_at,
                attempt_count: entry.attempt_count,
                last_error: entry.last_error,
            })
            .collect())
    }

    pub(super) fn sync_remote(&self) -> Option<&crate::app::SyncRemoteConfig> {
        self.sync.remote.as_ref().filter(|_| self.sync.enabled)
    }

    pub(super) async fn connect_remote_postgres(&self) -> AppResult<PostgresSyncRemote> {
        let remote = self
            .sync_remote()
            .ok_or_else(|| AppError::Conflict("sync is not enabled".to_string()))?;
        if remote.kind != SyncRemoteKind::Postgres {
            return Err(AppError::Conflict(
                "sync remote is not configured as postgres".to_string(),
            ));
        }
        PostgresSyncRemote::connect(&remote.postgres).await
    }

    pub(super) fn sync_remote_status(
        &self,
        remote: &SyncRemoteConfig,
    ) -> AppResult<SyncRemoteStatus> {
        Ok(match remote.kind {
            SyncRemoteKind::Postgres => {
                let url = Url::parse(&remote.postgres.dsn).map_err(|error| {
                    AppError::Config(format!("invalid sync postgres dsn: {error}"))
                })?;
                SyncRemoteStatus {
                    id: remote.id.clone(),
                    kind: remote.kind,
                    postgres: Some(SyncPostgresRemoteStatus {
                        host: url.host_str().map(ToOwned::to_owned),
                        port: url.port_or_known_default(),
                        database: {
                            let database = url.path().trim_start_matches('/');
                            (!database.is_empty()).then(|| database.to_string())
                        },
                        max_conns: remote.postgres.max_conns,
                        min_conns: remote.postgres.min_conns,
                        max_conn_lifetime: humantime::format_duration(
                            remote.postgres.max_conn_lifetime,
                        )
                        .to_string(),
                    }),
                }
            }
        })
    }

    pub(super) async fn backfill_entity_if_untracked<T: Serialize>(
        &self,
        remote_id: &str,
        entity_kind: SyncEntityKind,
        local_id: Uuid,
        payload: &T,
        updated_at: OffsetDateTime,
    ) -> AppResult<bool> {
        if self
            .store
            .get_sync_entity(entity_kind, local_id)
            .await?
            .is_some()
        {
            return Ok(false);
        }

        let payload_json = serde_json::to_value(payload).map_err(|error| {
            AppError::internal(format!("failed to serialize sync payload: {error}"))
        })?;
        let mut tx = self.store.pool.begin().await?;
        self.store
            .record_sync_mutation_tx(
                &mut tx,
                remote_id,
                entity_kind,
                local_id,
                SyncOperation::Create,
                &payload_json,
                updated_at,
            )
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub(super) async fn apply_remote_mutation(
        &self,
        remote_id: &str,
        mutation: &RemoteMutation,
    ) -> AppResult<bool> {
        if let Some(existing) = self
            .store
            .get_sync_entity(mutation.entity_kind, mutation.local_id)
            .await?
        {
            if existing.local_version >= mutation.local_version {
                return Ok(false);
            }
        }

        match mutation.entity_kind {
            SyncEntityKind::Project => {
                let project: Project = serde_json::from_value(mutation.payload_json.clone())
                    .map_err(|error| {
                        AppError::InvalidArguments(format!(
                            "invalid remote project payload: {error}"
                        ))
                    })?;
                let mut tx = self.store.pool.begin().await?;
                match self
                    .store
                    .get_project_by_ref_tx(&mut tx, &project.project_id.to_string())
                    .await
                {
                    Ok(_) => self.store.update_project_tx(&mut tx, &project).await?,
                    Err(AppError::NotFound { .. }) => {
                        self.store.insert_project_tx(&mut tx, &project).await?
                    }
                    Err(error) => return Err(error),
                }
                self.store
                    .upsert_synced_entity_state_tx(
                        &mut tx,
                        SyncEntityKind::Project,
                        project.project_id,
                        remote_id,
                        &mutation.remote_entity_id,
                        mutation.local_version,
                        mutation.created_at,
                    )
                    .await?;
                self.queue_project_task_search_jobs_tx(&mut tx, project.project_id)
                    .await?;
                tx.commit().await?;
                Ok(true)
            }
            SyncEntityKind::Version => {
                let version: Version = serde_json::from_value(mutation.payload_json.clone())
                    .map_err(|error| {
                        AppError::InvalidArguments(format!(
                            "invalid remote version payload: {error}"
                        ))
                    })?;
                let mut tx = self.store.pool.begin().await?;
                match self
                    .store
                    .get_version_by_ref_tx(&mut tx, &version.version_id.to_string())
                    .await
                {
                    Ok(_) => self.store.update_version_tx(&mut tx, &version).await?,
                    Err(AppError::NotFound { .. }) => {
                        self.store.insert_version_tx(&mut tx, &version).await?
                    }
                    Err(error) => return Err(error),
                }
                self.store
                    .upsert_synced_entity_state_tx(
                        &mut tx,
                        SyncEntityKind::Version,
                        version.version_id,
                        remote_id,
                        &mutation.remote_entity_id,
                        mutation.local_version,
                        mutation.created_at,
                    )
                    .await?;
                self.queue_version_task_search_jobs_tx(&mut tx, version.version_id)
                    .await?;
                tx.commit().await?;
                Ok(true)
            }
            SyncEntityKind::Task => {
                let task: Task =
                    serde_json::from_value(mutation.payload_json.clone()).map_err(|error| {
                        AppError::InvalidArguments(format!("invalid remote task payload: {error}"))
                    })?;
                let mut tx = self.store.pool.begin().await?;
                let previous = match self
                    .store
                    .get_task_by_ref_tx(&mut tx, &task.task_id.to_string())
                    .await
                {
                    Ok(existing) => {
                        self.store.update_task_tx(&mut tx, &task).await?;
                        Some(existing)
                    }
                    Err(AppError::NotFound { .. }) => {
                        self.store.insert_task_tx(&mut tx, &task).await?;
                        None
                    }
                    Err(error) => return Err(error),
                };

                if let Some(previous) = previous {
                    if previous.status != task.status {
                        let content = format!(
                            "Status changed from {} to {}.",
                            previous.status, task.status
                        );
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
                            created_at: mutation.created_at,
                            metadata_json: json!({
                                "from_status": previous.status,
                                "to_status": task.status,
                            }),
                        };
                        self.store.insert_activity_tx(&mut tx, &activity).await?;
                        self.store
                            .replace_activity_chunks_tx(&mut tx, &activity, &content)
                            .await?;
                    }
                }
                self.refresh_task_note_rollup_tx(&mut tx, task.task_id)
                    .await?;

                self.store
                    .upsert_synced_entity_state_tx(
                        &mut tx,
                        SyncEntityKind::Task,
                        task.task_id,
                        remote_id,
                        &mutation.remote_entity_id,
                        mutation.local_version,
                        mutation.created_at,
                    )
                    .await?;
                tx.commit().await?;
                Ok(true)
            }
            SyncEntityKind::TaskRelation => {
                let relation: TaskRelation = serde_json::from_value(mutation.payload_json.clone())
                    .map_err(|error| {
                        AppError::InvalidArguments(format!(
                            "invalid remote task relation payload: {error}"
                        ))
                    })?;
                let mut tx = self.store.pool.begin().await?;
                let existed = match self
                    .store
                    .get_task_relation_by_ref_tx(&mut tx, &relation.relation_id.to_string())
                    .await
                {
                    Ok(_) => {
                        self.store
                            .update_task_relation_tx(&mut tx, &relation)
                            .await?;
                        true
                    }
                    Err(AppError::NotFound { .. }) => {
                        self.store
                            .insert_task_relation_tx(&mut tx, &relation)
                            .await?;
                        false
                    }
                    Err(error) => return Err(error),
                };
                self.refresh_task_context_digest_tx(&mut tx, relation.source_task_id)
                    .await?;
                self.refresh_task_context_digest_tx(&mut tx, relation.target_task_id)
                    .await?;
                let source_message = if relation.status == TaskRelationStatus::Resolved {
                    format!(
                        "Resolved {} relation for task {}.",
                        relation.kind, relation.target_task_id
                    )
                } else {
                    format!(
                        "Applied {} relation for task {}.",
                        relation.kind, relation.target_task_id
                    )
                };
                let target_message = if relation.status == TaskRelationStatus::Resolved {
                    format!(
                        "Resolved {} relation from task {}.",
                        relation.kind, relation.source_task_id
                    )
                } else {
                    format!(
                        "Applied {} relation from task {}.",
                        relation.kind, relation.source_task_id
                    )
                };
                self.append_system_activity_tx(
                    &mut tx,
                    relation.source_task_id,
                    &source_message,
                    &relation.updated_by,
                    mutation.created_at,
                    json!({
                        "relation_id": relation.relation_id,
                        "kind": relation.kind,
                        "counterparty_task_id": relation.target_task_id,
                        "status": relation.status,
                    }),
                )
                .await?;
                self.append_system_activity_tx(
                    &mut tx,
                    relation.target_task_id,
                    &target_message,
                    &relation.updated_by,
                    mutation.created_at,
                    json!({
                        "relation_id": relation.relation_id,
                        "kind": relation.kind,
                        "counterparty_task_id": relation.source_task_id,
                        "status": relation.status,
                    }),
                )
                .await?;
                self.store
                    .upsert_synced_entity_state_tx(
                        &mut tx,
                        SyncEntityKind::TaskRelation,
                        relation.relation_id,
                        remote_id,
                        &mutation.remote_entity_id,
                        mutation.local_version,
                        mutation.created_at,
                    )
                    .await?;
                tx.commit().await?;
                Ok(!existed || relation.status == TaskRelationStatus::Resolved)
            }
            SyncEntityKind::Note => {
                let activity: TaskActivity = serde_json::from_value(mutation.payload_json.clone())
                    .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid remote note payload: {error}"))
                    })?;
                let mut tx = self.store.pool.begin().await?;
                let exists = sqlx::query("SELECT 1 FROM task_activities WHERE activity_id = ?")
                    .bind(activity.activity_id.to_string())
                    .fetch_optional(&mut *tx)
                    .await?
                    .is_some();
                if !exists {
                    self.store.insert_activity_tx(&mut tx, &activity).await?;
                    self.store
                        .replace_activity_chunks_tx(&mut tx, &activity, &activity.content)
                        .await?;
                    self.refresh_task_note_rollup_tx(&mut tx, activity.task_id)
                        .await?;
                }
                self.store
                    .upsert_synced_entity_state_tx(
                        &mut tx,
                        SyncEntityKind::Note,
                        activity.activity_id,
                        remote_id,
                        &mutation.remote_entity_id,
                        mutation.local_version,
                        mutation.created_at,
                    )
                    .await?;
                tx.commit().await?;
                Ok(!exists)
            }
            SyncEntityKind::Attachment => {
                let attachment: Attachment = serde_json::from_value(mutation.payload_json.clone())
                    .map_err(|error| {
                        AppError::InvalidArguments(format!(
                            "invalid remote attachment payload: {error}"
                        ))
                    })?;
                let blob = mutation.attachment_blob.clone().ok_or_else(|| {
                    AppError::Storage("remote attachment mutation missing blob content".to_string())
                })?;
                let destination = self.store.attachments_dir.join(&attachment.storage_path);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).await?;
                }
                fs::write(&destination, &blob).await?;
                let mut tx = self.store.pool.begin().await?;
                let exists = sqlx::query("SELECT 1 FROM attachments WHERE attachment_id = ?")
                    .bind(attachment.attachment_id.to_string())
                    .fetch_optional(&mut *tx)
                    .await?
                    .is_some();
                let result = async {
                    if !exists {
                        let attachment_search_body = self
                            .store
                            .extract_attachment_search_text(
                                &blob,
                                &attachment.mime,
                                &attachment.original_filename,
                            )
                            .map(|text| format!("{}\n{text}", attachment.summary))
                            .unwrap_or_else(|| attachment.summary.clone());
                        self.store
                            .insert_attachment_tx(&mut tx, &attachment)
                            .await?;
                        let activity = TaskActivity {
                            activity_id: Uuid::new_v4(),
                            task_id: attachment.task_id,
                            kind: TaskActivityKind::AttachmentRef,
                            content: attachment.summary.clone(),
                            activity_search_summary: build_activity_search_summary(
                                TaskActivityKind::AttachmentRef,
                                &attachment.summary,
                            ),
                            activity_search_text: build_activity_search_text(
                                TaskActivityKind::AttachmentRef,
                                &attachment_search_body,
                            ),
                            created_by: attachment.created_by.clone(),
                            created_at: mutation.created_at,
                            metadata_json: json!({
                                "attachment_id": attachment.attachment_id,
                                "storage_path": attachment.storage_path,
                            }),
                        };
                        self.store.insert_activity_tx(&mut tx, &activity).await?;
                        self.store
                            .replace_activity_chunks_tx(&mut tx, &activity, &attachment_search_body)
                            .await?;
                    }
                    self.store
                        .upsert_synced_entity_state_tx(
                            &mut tx,
                            SyncEntityKind::Attachment,
                            attachment.attachment_id,
                            remote_id,
                            &mutation.remote_entity_id,
                            mutation.local_version,
                            mutation.created_at,
                        )
                        .await?;
                    self.queue_task_search_jobs_tx(&mut tx, &[attachment.task_id])
                        .await?;
                    tx.commit().await?;
                    Ok::<(), AppError>(())
                }
                .await;

                if let Err(error) = result {
                    let _ = fs::remove_file(&destination).await;
                    return Err(error);
                }

                Ok(!exists)
            }
        }
    }

    pub(super) async fn enqueue_sync_mutation_tx<T: Serialize>(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        entity_kind: SyncEntityKind,
        local_id: Uuid,
        operation: SyncOperation,
        payload: &T,
        updated_at: OffsetDateTime,
    ) -> AppResult<()> {
        let Some(remote) = self.sync_remote() else {
            return Ok(());
        };
        let payload_json = serde_json::to_value(payload).map_err(|error| {
            AppError::internal(format!("failed to serialize sync payload: {error}"))
        })?;
        self.store
            .record_sync_mutation_tx(
                tx,
                &remote.id,
                entity_kind,
                local_id,
                operation,
                &payload_json,
                updated_at,
            )
            .await?;
        Ok(())
    }
}
