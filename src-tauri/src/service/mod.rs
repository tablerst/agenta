use std::sync::Arc;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Sqlite, Transaction};
use time::OffsetDateTime;
use tokio::fs;
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

use crate::app::{SyncConfig, SyncRemoteConfig, SyncRemoteKind};
use crate::domain::{
    ApprovalRequest, ApprovalRequestedVia, ApprovalStatus, Attachment, AttachmentKind, Project,
    ProjectStatus, SyncCheckpointKind, SyncEntityKind, SyncMode, SyncOperation, SyncOutboxStatus,
    Task, TaskActivity, TaskActivityKind, TaskPriority, TaskStatus, Version, VersionStatus,
};
use crate::error::{AppError, AppResult};
use crate::policy::{PolicyEngine, WriteDecision};
use crate::search::{
    build_activity_search_summary, build_task_context_digest, build_task_search_summary,
    SearchResponse,
};
use crate::storage::{SqliteStore, TaskListFilter};
use crate::sync::{PostgresSyncRemote, RemoteMutation};

#[derive(Clone)]
pub struct AgentaService {
    store: SqliteStore,
    policy: PolicyEngine,
    sync: SyncConfig,
    write_queue: Arc<Mutex<()>>,
}

#[derive(Clone, Copy, Debug)]
pub enum RequestOrigin {
    Cli,
    Mcp,
    Desktop,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServiceOverview {
    pub project_count: i64,
    pub task_count: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncCheckpointStatus {
    pub pull: Option<String>,
    pub push_ack: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncStatusSummary {
    pub enabled: bool,
    pub mode: SyncMode,
    pub remote: Option<SyncRemoteStatus>,
    pub pending_outbox_count: i64,
    pub oldest_pending_at: Option<OffsetDateTime>,
    pub checkpoints: SyncCheckpointStatus,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncRemoteStatus {
    pub id: String,
    pub kind: SyncRemoteKind,
    pub postgres: Option<SyncPostgresRemoteStatus>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncPostgresRemoteStatus {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database: Option<String>,
    pub max_conns: u32,
    pub min_conns: u32,
    pub max_conn_lifetime: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncOutboxListItem {
    pub mutation_id: Uuid,
    pub entity_kind: SyncEntityKind,
    pub local_id: Uuid,
    pub operation: SyncOperation,
    pub local_version: i64,
    pub status: SyncOutboxStatus,
    pub created_at: OffsetDateTime,
    pub attempt_count: i64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncPushSummary {
    pub attempted: usize,
    pub pushed: usize,
    pub failed: usize,
    pub last_remote_mutation_id: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SyncBackfillSummary {
    pub scanned: usize,
    pub queued: usize,
    pub skipped: usize,
    pub queued_projects: usize,
    pub queued_versions: usize,
    pub queued_tasks: usize,
    pub queued_notes: usize,
    pub queued_attachments: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncPullSummary {
    pub fetched: usize,
    pub applied: usize,
    pub skipped: usize,
    pub last_remote_mutation_id: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateProjectInput {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateProjectInput {
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<ProjectStatus>,
    pub default_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateVersionInput {
    pub project: String,
    pub name: String,
    pub description: Option<String>,
    pub status: Option<VersionStatus>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateVersionInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<VersionStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateTaskInput {
    pub project: String,
    pub version: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub created_by: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateTaskInput {
    pub version: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub updated_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateNoteInput {
    pub task: String,
    pub content: String,
    pub created_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateAttachmentInput {
    pub task: String,
    pub path: PathBuf,
    pub kind: Option<AttachmentKind>,
    pub created_by: Option<String>,
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TaskQuery {
    pub project: Option<String>,
    pub version: Option<String>,
    pub status: Option<TaskStatus>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageCursor {
    pub created_at: OffsetDateTime,
    pub id: Uuid,
}

#[derive(Clone, Debug, Default)]
pub struct PageRequest {
    pub limit: Option<usize>,
    pub cursor: Option<PageCursor>,
}

#[derive(Clone, Debug)]
pub struct PageResult<T> {
    pub items: Vec<T>,
    pub limit: Option<usize>,
    pub next_cursor: Option<PageCursor>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskDetail {
    pub task: Task,
    pub note_count: i64,
    pub attachment_count: i64,
    pub latest_activity_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskContext {
    pub task: TaskDetail,
    pub notes: Vec<TaskActivity>,
    pub attachments: Vec<Attachment>,
    pub recent_activities: Vec<TaskActivity>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchInput {
    pub text: String,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ApprovalQuery {
    pub project: Option<String>,
    pub status: Option<ApprovalStatus>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ReviewApprovalInput {
    pub reviewed_by: Option<String>,
    pub review_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ApprovalSeed {
    requested_via: ApprovalRequestedVia,
    resource_ref: String,
    payload_json: Value,
    request_summary: String,
    requested_by: String,
}

#[derive(Default)]
struct ApprovalContext {
    project_ref: Option<String>,
    project_name: Option<String>,
    task_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ReferencedUpdatePayload<T> {
    reference: String,
    input: T,
}

#[derive(Clone, Debug)]
enum ApprovalMode {
    Standard(ApprovalSeed),
    Replay,
}

impl RequestOrigin {
    fn requested_via(self) -> ApprovalRequestedVia {
        match self {
            Self::Cli => ApprovalRequestedVia::Cli,
            Self::Mcp => ApprovalRequestedVia::Mcp,
            Self::Desktop => ApprovalRequestedVia::Desktop,
        }
    }

    fn fallback_actor(self) -> &'static str {
        self.requested_via().as_str()
    }
}

impl AgentaService {
    pub fn new(store: SqliteStore, policy: PolicyEngine, sync: SyncConfig) -> Self {
        Self {
            store,
            policy,
            sync,
            write_queue: Arc::new(Mutex::new(())),
        }
    }

    pub async fn service_overview(&self) -> AppResult<ServiceOverview> {
        Ok(ServiceOverview {
            project_count: self.store.project_count().await?,
            task_count: self.store.task_count().await?,
        })
    }

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

        let tasks = self.list_tasks(TaskQuery::default()).await?;
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
        let mutations = remote
            .pull_mutations(&remote_config.id, after_remote_mutation_id, limit)
            .await?;
        let mut summary = SyncPullSummary {
            fetched: mutations.len(),
            applied: 0,
            skipped: 0,
            last_remote_mutation_id: None,
        };

        for mutation in mutations {
            let applied = {
                let _write_guard = self.write_queue.lock().await;
                let applied = self
                    .apply_remote_mutation(&remote_config.id, &mutation)
                    .await?;
                self.store
                    .upsert_sync_checkpoint(
                        &remote_config.id,
                        SyncCheckpointKind::Pull,
                        &mutation.remote_mutation_id.to_string(),
                        mutation.created_at,
                    )
                    .await?;
                applied
            };
            if applied {
                summary.applied += 1;
            } else {
                summary.skipped += 1;
            }
            summary.last_remote_mutation_id = Some(mutation.remote_mutation_id);
        }

        remote.close().await;
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

    pub async fn create_project(&self, input: CreateProjectInput) -> AppResult<Project> {
        self.create_project_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_project_from(
        &self,
        origin: RequestOrigin,
        input: CreateProjectInput,
    ) -> AppResult<Project> {
        let slug = normalize_slug(&input.slug);
        if slug.is_empty() {
            return Err(AppError::InvalidArguments(
                "project slug must not be empty".to_string(),
            ));
        }
        let approval = self.approval_seed(
            origin,
            slug.clone(),
            format!("Create project {slug}"),
            actor_or_default(None, origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.create_project_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn get_project(&self, reference: &str) -> AppResult<Project> {
        self.store.get_project_by_ref(reference).await
    }

    pub async fn list_projects(&self) -> AppResult<Vec<Project>> {
        self.store.list_projects().await
    }

    pub async fn list_projects_page(&self, page: PageRequest) -> AppResult<PageResult<Project>> {
        let projects = self.store.list_projects().await?;
        Ok(paginate_by_created_at(
            projects,
            page,
            |project| project.created_at,
            |project| project.project_id,
        ))
    }

    pub async fn update_project(
        &self,
        reference: &str,
        input: UpdateProjectInput,
    ) -> AppResult<Project> {
        self.update_project_from(RequestOrigin::Cli, reference, input)
            .await
    }

    pub async fn update_project_from(
        &self,
        origin: RequestOrigin,
        reference: &str,
        input: UpdateProjectInput,
    ) -> AppResult<Project> {
        let approval = self.approval_seed(
            origin,
            reference.to_string(),
            format!("Update project {reference}"),
            actor_or_default(None, origin),
            &ReferencedUpdatePayload {
                reference: reference.to_string(),
                input: input.clone(),
            },
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.update_project_internal(reference, input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn create_version(&self, input: CreateVersionInput) -> AppResult<Version> {
        self.create_version_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_version_from(
        &self,
        origin: RequestOrigin,
        input: CreateVersionInput,
    ) -> AppResult<Version> {
        let approval = self.approval_seed(
            origin,
            input.project.clone(),
            format!(
                "Create version {} in {}",
                input.name.trim(),
                input.project.trim()
            ),
            actor_or_default(None, origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.create_version_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn get_version(&self, reference: &str) -> AppResult<Version> {
        self.store.get_version_by_ref(reference).await
    }

    pub async fn list_versions(&self, project_ref: Option<&str>) -> AppResult<Vec<Version>> {
        let project_id = match project_ref {
            Some(reference) => Some(self.store.get_project_by_ref(reference).await?.project_id),
            None => None,
        };
        self.store.list_versions(project_id).await
    }

    pub async fn list_versions_page(
        &self,
        project_ref: Option<&str>,
        page: PageRequest,
    ) -> AppResult<PageResult<Version>> {
        let versions = self.list_versions(project_ref).await?;
        Ok(paginate_by_created_at(
            versions,
            page,
            |version| version.created_at,
            |version| version.version_id,
        ))
    }

    pub async fn update_version(
        &self,
        reference: &str,
        input: UpdateVersionInput,
    ) -> AppResult<Version> {
        self.update_version_from(RequestOrigin::Cli, reference, input)
            .await
    }

    pub async fn update_version_from(
        &self,
        origin: RequestOrigin,
        reference: &str,
        input: UpdateVersionInput,
    ) -> AppResult<Version> {
        let approval = self.approval_seed(
            origin,
            reference.to_string(),
            format!("Update version {reference}"),
            actor_or_default(None, origin),
            &ReferencedUpdatePayload {
                reference: reference.to_string(),
                input: input.clone(),
            },
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.update_version_internal(reference, input, ApprovalMode::Standard(approval))
            .await
    }

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

    pub async fn get_task(&self, reference: &str) -> AppResult<Task> {
        self.store.get_task_by_ref(reference).await
    }

    pub async fn get_task_detail(&self, reference: &str) -> AppResult<TaskDetail> {
        let (task, note_count, attachment_count, latest_activity_at) =
            self.store.get_task_with_stats_by_ref(reference).await?;
        Ok(TaskDetail {
            task,
            note_count,
            attachment_count,
            latest_activity_at,
        })
    }

    pub async fn list_tasks(&self, query: TaskQuery) -> AppResult<Vec<Task>> {
        let filter = self.resolve_task_filter(&query).await?;
        self.store.list_tasks(filter).await
    }

    pub async fn list_task_details_page(
        &self,
        query: TaskQuery,
        page: PageRequest,
    ) -> AppResult<PageResult<TaskDetail>> {
        let filter = self.resolve_task_filter(&query).await?;
        let details = self
            .store
            .list_tasks_with_stats(filter)
            .await?
            .into_iter()
            .map(
                |(task, note_count, attachment_count, latest_activity_at)| TaskDetail {
                    task,
                    note_count,
                    attachment_count,
                    latest_activity_at,
                },
            )
            .collect();
        Ok(paginate_by_created_at(
            details,
            page,
            |detail| detail.task.created_at,
            |detail| detail.task.task_id,
        ))
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

    pub async fn get_task_context(
        &self,
        task_ref: &str,
        recent_activity_limit: Option<usize>,
    ) -> AppResult<TaskContext> {
        let task = self.get_task_detail(task_ref).await?;
        let notes = self.list_notes(task_ref).await?;
        let attachments = self.list_attachments(task_ref).await?;
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
        })
    }

    pub async fn search(&self, input: SearchInput) -> AppResult<SearchResponse> {
        let text = require_non_empty(input.text, "search text")?;
        let limit = input.limit.unwrap_or(10).clamp(1, 50);
        self.store.search(&text, limit).await
    }

    pub async fn list_approval_requests(
        &self,
        query: ApprovalQuery,
    ) -> AppResult<Vec<ApprovalRequest>> {
        let (project_slug_filter, project_id_filter) = match query.project.as_deref() {
            Some(reference) => match self.store.get_project_by_ref(reference).await {
                Ok(project) => (Some(project.slug), Some(project.project_id.to_string())),
                Err(AppError::NotFound { .. }) => (Some(reference.to_string()), None),
                Err(error) => return Err(error),
            },
            None => (None, None),
        };
        let items = self.store.list_approval_requests(query.status).await?;
        let mut approvals = Vec::with_capacity(items.len());

        for item in items {
            let approval = self.enrich_approval_request(item).await;
            if let Some(project_slug) = project_slug_filter.as_deref() {
                if !matches_project_filter(&approval, project_slug, project_id_filter.as_deref()) {
                    continue;
                }
            }
            approvals.push(approval);
        }

        Ok(approvals)
    }

    pub async fn get_approval_request(&self, request_id: &str) -> AppResult<ApprovalRequest> {
        let request = self
            .store
            .get_approval_request(parse_uuid(request_id, "request_id")?)
            .await?;
        Ok(self.enrich_approval_request(request).await)
    }

    pub async fn approve_approval_request(
        &self,
        request_id: &str,
        input: ReviewApprovalInput,
    ) -> AppResult<ApprovalRequest> {
        let _write_guard = self.write_queue.lock().await;
        let request_id = parse_uuid(request_id, "request_id")?;
        let mut request = self.store.get_approval_request(request_id).await?;
        ensure_pending(&request)?;

        let reviewer = actor_or_default(input.reviewed_by.as_deref(), RequestOrigin::Desktop);
        let review_note = clean_optional(input.review_note);
        let reviewed_at = OffsetDateTime::now_utc();

        match self.replay_approval_request(&request).await {
            Ok(result_json) => {
                request.reviewed_at = Some(reviewed_at);
                request.reviewed_by = Some(reviewer);
                request.review_note = review_note;
                request.result_json = Some(result_json);
                request.error_json = None;
                request.status = ApprovalStatus::Approved;
            }
            Err(app_error) => {
                request.reviewed_at = Some(reviewed_at);
                request.reviewed_by = Some(reviewer);
                request.review_note = review_note;
                request.result_json = None;
                request.error_json = Some(error_value(&app_error));
                request.status = ApprovalStatus::Failed;
            }
        }
        self.store.update_approval_request(&request).await?;
        Ok(self.enrich_approval_request(request).await)
    }

    pub async fn deny_approval_request(
        &self,
        request_id: &str,
        input: ReviewApprovalInput,
    ) -> AppResult<ApprovalRequest> {
        let _write_guard = self.write_queue.lock().await;
        let request_id = parse_uuid(request_id, "request_id")?;
        let mut request = self.store.get_approval_request(request_id).await?;
        ensure_pending(&request)?;
        request.reviewed_at = Some(OffsetDateTime::now_utc());
        request.reviewed_by = Some(actor_or_default(
            input.reviewed_by.as_deref(),
            RequestOrigin::Desktop,
        ));
        request.review_note = clean_optional(input.review_note);
        request.result_json = None;
        request.error_json = None;
        request.status = ApprovalStatus::Denied;
        self.store.update_approval_request(&request).await?;
        Ok(self.enrich_approval_request(request).await)
    }

    async fn create_project_internal(
        &self,
        input: CreateProjectInput,
        mode: ApprovalMode,
    ) -> AppResult<Project> {
        let slug = normalize_slug(&input.slug);
        if slug.is_empty() {
            return Err(AppError::InvalidArguments(
                "project slug must not be empty".to_string(),
            ));
        }

        self.enforce("project.create", mode).await?;

        let now = OffsetDateTime::now_utc();
        let project = Project {
            project_id: Uuid::new_v4(),
            slug,
            name: require_non_empty(input.name, "project name")?,
            description: clean_optional(input.description),
            status: ProjectStatus::Active,
            default_version_id: None,
            created_at: now,
            updated_at: now,
        };
        let mut tx = self.store.pool.begin().await?;
        self.store.insert_project_tx(&mut tx, &project).await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Project,
            project.project_id,
            SyncOperation::Create,
            &project,
            project.updated_at,
        )
        .await?;
        tx.commit().await?;
        Ok(project)
    }

    async fn update_project_internal(
        &self,
        reference: &str,
        input: UpdateProjectInput,
        mode: ApprovalMode,
    ) -> AppResult<Project> {
        self.enforce("project.update", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let mut project = self.store.get_project_by_ref_tx(&mut tx, reference).await?;
        if let Some(slug) = input.slug {
            project.slug = normalize_slug(&slug);
        }
        if let Some(name) = input.name {
            project.name = require_non_empty(name, "project name")?;
        }
        if let Some(description) = input.description {
            project.description = clean_optional(Some(description));
        }
        if let Some(status) = input.status {
            project.status = status;
        }
        if let Some(default_version) = input.default_version {
            let version = self
                .store
                .get_version_by_ref_tx(&mut tx, &default_version)
                .await?;
            if version.project_id != project.project_id {
                return Err(AppError::Conflict(
                    "default version must belong to the target project".to_string(),
                ));
            }
            project.default_version_id = Some(version.version_id);
        }
        project.updated_at = OffsetDateTime::now_utc();
        self.store.update_project_tx(&mut tx, &project).await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Project,
            project.project_id,
            SyncOperation::Update,
            &project,
            project.updated_at,
        )
        .await?;
        tx.commit().await?;
        Ok(project)
    }

    async fn create_version_internal(
        &self,
        input: CreateVersionInput,
        mode: ApprovalMode,
    ) -> AppResult<Version> {
        self.enforce("version.create", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let mut project = self
            .store
            .get_project_by_ref_tx(&mut tx, &input.project)
            .await?;
        let now = OffsetDateTime::now_utc();
        let version = Version {
            version_id: Uuid::new_v4(),
            project_id: project.project_id,
            name: require_non_empty(input.name, "version name")?,
            description: clean_optional(input.description),
            status: input.status.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        };
        self.store.insert_version_tx(&mut tx, &version).await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Version,
            version.version_id,
            SyncOperation::Create,
            &version,
            version.updated_at,
        )
        .await?;
        if project.default_version_id.is_none() {
            project.default_version_id = Some(version.version_id);
            project.updated_at = now;
            self.store
                .set_project_default_version_tx(
                    &mut tx,
                    project.project_id,
                    Some(version.version_id),
                    now,
                )
                .await?;
            self.enqueue_sync_mutation_tx(
                &mut tx,
                SyncEntityKind::Project,
                project.project_id,
                SyncOperation::Update,
                &project,
                project.updated_at,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(version)
    }

    async fn update_version_internal(
        &self,
        reference: &str,
        input: UpdateVersionInput,
        mode: ApprovalMode,
    ) -> AppResult<Version> {
        self.enforce("version.update", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let mut version = self.store.get_version_by_ref_tx(&mut tx, reference).await?;
        if let Some(name) = input.name {
            version.name = require_non_empty(name, "version name")?;
        }
        if let Some(description) = input.description {
            version.description = clean_optional(Some(description));
        }
        if let Some(status) = input.status {
            version.status = status;
        }
        version.updated_at = OffsetDateTime::now_utc();
        self.store.update_version_tx(&mut tx, &version).await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Version,
            version.version_id,
            SyncOperation::Update,
            &version,
            version.updated_at,
        )
        .await?;
        tx.commit().await?;
        Ok(version)
    }

    async fn create_task_internal(
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
            title: require_non_empty(input.title, "task title")?,
            summary: clean_optional(input.summary),
            description: clean_optional(input.description),
            task_search_summary: String::new(),
            task_context_digest: String::new(),
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
            &task.title,
            task.summary.as_deref(),
            task.description.as_deref(),
        );
        task.task_context_digest = build_task_context_digest(&task);
        self.store.insert_task_tx(&mut tx, &task).await?;
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
        Ok(task)
    }

    async fn update_task_internal(
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
            &task.title,
            task.summary.as_deref(),
            task.description.as_deref(),
        );
        task.task_context_digest = build_task_context_digest(&task);
        self.store.update_task_tx(&mut tx, &task).await?;
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
                created_by: task.updated_by.clone(),
                created_at: task.updated_at,
                metadata_json: json!({
                    "from_status": previous_status,
                    "to_status": task.status,
                }),
            };
            self.store.insert_activity_tx(&mut tx, &activity).await?;
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
        Ok(task)
    }

    async fn create_note_internal(
        &self,
        input: CreateNoteInput,
        mode: ApprovalMode,
    ) -> AppResult<TaskActivity> {
        self.enforce("note.create", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let task = self.store.get_task_by_ref_tx(&mut tx, &input.task).await?;
        let now = OffsetDateTime::now_utc();
        let content = require_non_empty(input.content, "note content")?;
        let activity = TaskActivity {
            activity_id: Uuid::new_v4(),
            task_id: task.task_id,
            kind: TaskActivityKind::Note,
            content: content.clone(),
            activity_search_summary: build_activity_search_summary(
                TaskActivityKind::Note,
                &content,
            ),
            created_by: input
                .created_by
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "cli".to_string()),
            created_at: now,
            metadata_json: json!({}),
        };
        self.store.insert_activity_tx(&mut tx, &activity).await?;
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
        Ok(activity)
    }

    async fn create_attachment_internal(
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
        let activity = TaskActivity {
            activity_id: Uuid::new_v4(),
            task_id: task.task_id,
            kind: TaskActivityKind::AttachmentRef,
            content: summary.clone(),
            activity_search_summary: build_activity_search_summary(
                TaskActivityKind::AttachmentRef,
                &summary,
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
            self.enqueue_sync_mutation_tx(
                &mut tx,
                SyncEntityKind::Attachment,
                attachment.attachment_id,
                SyncOperation::Create,
                &attachment,
                attachment.created_at,
            )
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

        Ok(attachment)
    }

    async fn enforce(&self, action: &str, mode: ApprovalMode) -> AppResult<()> {
        match self.policy.decision_for(action) {
            WriteDecision::Auto => Ok(()),
            WriteDecision::RequireHuman => match mode {
                ApprovalMode::Replay => Ok(()),
                ApprovalMode::Standard(seed) => {
                    let approval_request = ApprovalRequest {
                        request_id: Uuid::new_v4(),
                        action: action.to_string(),
                        requested_via: seed.requested_via,
                        resource_ref: seed.resource_ref,
                        project_ref: None,
                        project_name: None,
                        task_ref: None,
                        payload_json: seed.payload_json,
                        request_summary: seed.request_summary,
                        requested_at: OffsetDateTime::now_utc(),
                        requested_by: seed.requested_by,
                        reviewed_at: None,
                        reviewed_by: None,
                        review_note: None,
                        result_json: None,
                        error_json: None,
                        status: ApprovalStatus::Pending,
                    };
                    self.store
                        .insert_approval_request(&approval_request)
                        .await?;
                    Err(AppError::PolicyBlocked {
                        action: action.to_string(),
                        decision: WriteDecision::RequireHuman,
                        approval_request_id: Some(approval_request.request_id),
                        request_summary: Some(approval_request.request_summary.clone()),
                        payload_snapshot: Some(approval_request.payload_json.clone()),
                    })
                }
            },
            WriteDecision::Deny => Err(AppError::PolicyBlocked {
                action: action.to_string(),
                decision: WriteDecision::Deny,
                approval_request_id: None,
                request_summary: None,
                payload_snapshot: None,
            }),
        }
    }

    fn approval_seed(
        &self,
        origin: RequestOrigin,
        resource_ref: String,
        request_summary: String,
        requested_by: String,
        payload: &impl Serialize,
    ) -> AppResult<ApprovalSeed> {
        Ok(ApprovalSeed {
            requested_via: origin.requested_via(),
            resource_ref,
            payload_json: serde_json::to_value(payload).map_err(|error| {
                AppError::internal(format!("failed to serialize payload: {error}"))
            })?,
            request_summary,
            requested_by,
        })
    }

    async fn enrich_approval_request(&self, mut request: ApprovalRequest) -> ApprovalRequest {
        let context = self.resolve_approval_context(&request).await;
        request.project_ref = context.project_ref;
        request.project_name = context.project_name;
        request.task_ref = context.task_ref;
        request
    }

    async fn resolve_approval_context(&self, request: &ApprovalRequest) -> ApprovalContext {
        match request.action.as_str() {
            "project.create" => {
                let project_ref = json_string(&request.payload_json, "slug")
                    .or_else(|| Some(request.resource_ref.clone()));
                let project_name = json_string(&request.payload_json, "name");
                self.project_context_from_reference(project_ref, project_name)
                    .await
            }
            "project.update" => {
                let project_name = json_string(&request.payload_json, "name");
                self.project_context_from_reference(
                    Some(request.resource_ref.clone()),
                    project_name,
                )
                .await
            }
            "version.create" => {
                self.project_context_from_reference(
                    json_string(&request.payload_json, "project"),
                    None,
                )
                .await
            }
            "version.update" => {
                self.version_context_from_reference(&request.resource_ref)
                    .await
            }
            "task.create" => {
                let mut context = self
                    .project_context_from_reference(
                        json_string(&request.payload_json, "project"),
                        None,
                    )
                    .await;
                context.task_ref = request
                    .result_json
                    .as_ref()
                    .and_then(|value| json_string(value, "task_id"));
                context
            }
            "task.update" => {
                self.task_context_from_reference(&request.resource_ref)
                    .await
            }
            "note.create" | "attachment.create" => {
                let task_ref = json_string(&request.payload_json, "task")
                    .unwrap_or_else(|| request.resource_ref.clone());
                self.task_context_from_reference(&task_ref).await
            }
            _ => ApprovalContext::default(),
        }
    }

    async fn project_context_from_reference(
        &self,
        project_ref: Option<String>,
        project_name: Option<String>,
    ) -> ApprovalContext {
        let Some(reference) = project_ref else {
            return ApprovalContext {
                project_name,
                ..ApprovalContext::default()
            };
        };

        if let Ok(project) = self.store.get_project_by_ref(&reference).await {
            return ApprovalContext {
                project_ref: Some(project.slug),
                project_name: Some(project.name),
                task_ref: None,
            };
        }

        ApprovalContext {
            project_ref: Some(reference),
            project_name,
            task_ref: None,
        }
    }

    async fn version_context_from_reference(&self, reference: &str) -> ApprovalContext {
        let Ok(version) = self.store.get_version_by_ref(reference).await else {
            return ApprovalContext::default();
        };

        self.project_context_from_reference(Some(version.project_id.to_string()), None)
            .await
    }

    async fn task_context_from_reference(&self, reference: &str) -> ApprovalContext {
        let Ok(task) = self.store.get_task_by_ref(reference).await else {
            return ApprovalContext {
                task_ref: Some(reference.to_string()),
                ..ApprovalContext::default()
            };
        };

        let mut context = self
            .project_context_from_reference(Some(task.project_id.to_string()), None)
            .await;
        context.task_ref = Some(task.task_id.to_string());
        context
    }

    async fn replay_approval_request(&self, request: &ApprovalRequest) -> AppResult<Value> {
        match request.action.as_str() {
            "project.create" => {
                let input =
                    serde_json::from_value::<CreateProjectInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.create_project_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "project.update" => {
                let payload =
                    serde_json::from_value::<ReferencedUpdatePayload<UpdateProjectInput>>(
                        request.payload_json.clone(),
                    )
                    .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.update_project_internal(
                        &payload.reference,
                        payload.input,
                        ApprovalMode::Replay,
                    )
                    .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "version.create" => {
                let input =
                    serde_json::from_value::<CreateVersionInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.create_version_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "version.update" => {
                let payload =
                    serde_json::from_value::<ReferencedUpdatePayload<UpdateVersionInput>>(
                        request.payload_json.clone(),
                    )
                    .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.update_version_internal(
                        &payload.reference,
                        payload.input,
                        ApprovalMode::Replay,
                    )
                    .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.create" => {
                let input = serde_json::from_value::<CreateTaskInput>(request.payload_json.clone())
                    .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.create_task_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.update" => {
                let payload = serde_json::from_value::<ReferencedUpdatePayload<UpdateTaskInput>>(
                    request.payload_json.clone(),
                )
                .map_err(|error| {
                    AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                })?;
                serde_json::to_value(
                    self.update_task_internal(
                        &payload.reference,
                        payload.input,
                        ApprovalMode::Replay,
                    )
                    .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "note.create" => {
                let input = serde_json::from_value::<CreateNoteInput>(request.payload_json.clone())
                    .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.create_note_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "attachment.create" => {
                let input =
                    serde_json::from_value::<CreateAttachmentInput>(request.payload_json.clone())
                        .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.create_attachment_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            other => Err(AppError::InvalidAction(format!(
                "unsupported approval replay action: {other}"
            ))),
        }
    }

    fn sync_remote(&self) -> Option<&crate::app::SyncRemoteConfig> {
        self.sync.remote.as_ref().filter(|_| self.sync.enabled)
    }

    async fn connect_remote_postgres(&self) -> AppResult<PostgresSyncRemote> {
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

    fn sync_remote_status(&self, remote: &SyncRemoteConfig) -> AppResult<SyncRemoteStatus> {
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

    async fn backfill_entity_if_untracked<T: Serialize>(
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

    async fn apply_remote_mutation(
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
                            created_by: task.updated_by.clone(),
                            created_at: mutation.created_at,
                            metadata_json: json!({
                                "from_status": previous.status,
                                "to_status": task.status,
                            }),
                        };
                        self.store.insert_activity_tx(&mut tx, &activity).await?;
                    }
                }

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
                            created_by: attachment.created_by.clone(),
                            created_at: mutation.created_at,
                            metadata_json: json!({
                                "attachment_id": attachment.attachment_id,
                                "storage_path": attachment.storage_path,
                            }),
                        };
                        self.store.insert_activity_tx(&mut tx, &activity).await?;
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

    async fn enqueue_sync_mutation_tx<T: Serialize>(
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

    async fn resolve_version_for_project_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        project_id: Uuid,
        version_ref: Option<&str>,
    ) -> AppResult<Option<Uuid>> {
        match version_ref {
            Some(reference) => {
                let version = self.store.get_version_by_ref_tx(tx, reference).await?;
                if version.project_id != project_id {
                    return Err(AppError::Conflict(
                        "version must belong to the selected project".to_string(),
                    ));
                }
                Ok(Some(version.version_id))
            }
            None => Ok(None),
        }
    }

    async fn resolve_task_filter(&self, query: &TaskQuery) -> AppResult<TaskListFilter> {
        Ok(TaskListFilter {
            project_id: match query.project.as_deref() {
                Some(reference) => Some(self.store.get_project_by_ref(reference).await?.project_id),
                None => None,
            },
            version_id: match query.version.as_deref() {
                Some(reference) => Some(self.store.get_version_by_ref(reference).await?.version_id),
                None => None,
            },
            status: query.status,
        })
    }
}

fn normalize_slug(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            '-' | '_' => '-',
            ' ' => '-',
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn require_non_empty(value: String, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::InvalidArguments(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(trimmed.to_string())
    }
}

fn closed_at_for_status(status: TaskStatus, now: OffsetDateTime) -> Option<OffsetDateTime> {
    match status {
        TaskStatus::Done | TaskStatus::Cancelled => Some(now),
        _ => None,
    }
}

fn paginate_by_created_at<T, FCreatedAt, FId>(
    mut items: Vec<T>,
    page: PageRequest,
    created_at: FCreatedAt,
    id: FId,
) -> PageResult<T>
where
    FCreatedAt: Fn(&T) -> OffsetDateTime,
    FId: Fn(&T) -> Uuid,
{
    items.sort_by(|left, right| {
        created_at(right)
            .cmp(&created_at(left))
            .then_with(|| id(right).cmp(&id(left)))
    });

    if let Some(cursor) = page.cursor {
        items.retain(|item| {
            let item_created_at = created_at(item);
            let item_id = id(item);
            item_created_at < cursor.created_at
                || (item_created_at == cursor.created_at && item_id < cursor.id)
        });
    }

    let Some(limit) = page.limit.map(|value| value.clamp(1, 50)) else {
        return PageResult {
            items,
            limit: None,
            next_cursor: None,
            has_more: false,
        };
    };

    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit + 1);
    }

    let next_cursor = if has_more {
        let last_visible = &items[limit - 1];
        Some(PageCursor {
            created_at: created_at(last_visible),
            id: id(last_visible),
        })
    } else {
        None
    };

    if has_more {
        items.truncate(limit);
    }

    PageResult {
        items,
        limit: Some(limit),
        next_cursor,
        has_more,
    }
}

fn actor_or_default(value: Option<&str>, origin: RequestOrigin) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(origin.fallback_actor())
        .to_string()
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
}

fn matches_project_filter(
    request: &ApprovalRequest,
    project_slug: &str,
    project_id: Option<&str>,
) -> bool {
    request.project_ref.as_deref() == Some(project_slug)
        || project_id.is_some_and(|project_id| request.project_ref.as_deref() == Some(project_id))
}

fn parse_uuid(value: &str, field: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value)
        .map_err(|error| AppError::InvalidArguments(format!("invalid {field}: {error}")))
}

fn ensure_pending(request: &ApprovalRequest) -> AppResult<()> {
    if request.status == ApprovalStatus::Pending {
        Ok(())
    } else {
        Err(AppError::Conflict(format!(
            "approval request {} is already {}",
            request.request_id, request.status
        )))
    }
}

fn error_value(app_error: &AppError) -> Value {
    json!({
        "code": app_error.code(),
        "message": app_error.message(),
        "details": app_error.details(),
    })
}
