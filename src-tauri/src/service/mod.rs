use std::sync::Arc;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, Sqlite, Transaction};
use time::OffsetDateTime;
use tokio::fs;
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

use crate::app::{SearchConfig, SyncConfig, SyncRemoteConfig, SyncRemoteKind};
use crate::domain::{
    ApprovalRequest, ApprovalRequestedVia, ApprovalStatus, Attachment, AttachmentKind,
    KnowledgeStatus, NoteKind, Project, ProjectStatus, SyncCheckpointKind, SyncEntityKind,
    SyncMode, SyncOperation, SyncOutboxStatus, Task, TaskActivity, TaskActivityKind, TaskKind,
    TaskPriority, TaskRelation, TaskRelationKind, TaskRelationStatus, TaskStatus, Version,
    VersionStatus,
};
use crate::error::{AppError, AppResult};
use crate::policy::{PolicyEngine, WriteDecision};
use crate::search::{
    build_activity_search_summary, build_task_context_digest, build_task_search_summary,
    matched_field_names, normalize_search_query, weighted_rrf_score, ActivitySearchHit,
    SearchIndexedFields, SearchMeta, SearchResponse, SearchRuntime, TaskSearchHit,
    DEFAULT_SEARCH_LIMIT, LEXICAL_RRF_WEIGHT, MAX_SEARCH_LIMIT, SEMANTIC_RRF_WEIGHT,
};
use crate::storage::{SqliteStore, TaskListFilter};
use crate::sync::{PostgresSyncRemote, RemoteMutation};

#[derive(Clone)]
pub struct AgentaService {
    store: SqliteStore,
    policy: PolicyEngine,
    sync: SyncConfig,
    search: SearchRuntime,
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
    pub task_code: Option<String>,
    pub task_kind: Option<TaskKind>,
    pub title: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub created_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateChildTaskInput {
    pub parent: String,
    pub version: Option<String>,
    pub task_code: Option<String>,
    pub task_kind: Option<TaskKind>,
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
    pub task_code: Option<String>,
    pub task_kind: Option<TaskKind>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub updated_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AttachChildTaskInput {
    pub parent: String,
    pub child: String,
    pub updated_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DetachChildTaskInput {
    pub parent: String,
    pub child: String,
    pub updated_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AddTaskBlockerInput {
    pub blocker: String,
    pub blocked: String,
    pub updated_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResolveTaskBlockerInput {
    pub task: String,
    pub blocker: Option<String>,
    pub relation_id: Option<String>,
    pub updated_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateNoteInput {
    pub task: String,
    pub content: String,
    pub note_kind: Option<NoteKind>,
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
    pub task_kind: Option<TaskKind>,
    pub task_code_prefix: Option<String>,
    pub title_prefix: Option<String>,
    pub sort_by: Option<TaskSortBy>,
    pub sort_order: Option<SortOrder>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskSortBy {
    CreatedAt,
    UpdatedAt,
    LatestActivityAt,
    TaskCode,
    Title,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl TaskSortBy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
            Self::LatestActivityAt => "latest_activity_at",
            Self::TaskCode => "task_code",
            Self::Title => "title",
        }
    }
}

impl std::fmt::Display for TaskSortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TaskSortBy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "created_at" => Ok(Self::CreatedAt),
            "updated_at" => Ok(Self::UpdatedAt),
            "latest_activity_at" => Ok(Self::LatestActivityAt),
            "task_code" => Ok(Self::TaskCode),
            "title" => Ok(Self::Title),
            other => Err(format!("invalid TaskSortBy value: {other}")),
        }
    }
}

impl SortOrder {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

impl std::fmt::Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SortOrder {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "asc" => Ok(Self::Asc),
            "desc" => Ok(Self::Desc),
            other => Err(format!("invalid SortOrder value: {other}")),
        }
    }
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
    pub parent_task_id: Option<Uuid>,
    pub child_count: i64,
    pub open_blocker_count: i64,
    pub blocking_count: i64,
    pub ready_to_start: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskStatusCounts {
    pub draft: usize,
    pub ready: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub done: usize,
    pub cancelled: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskKnowledgeCounts {
    pub empty: usize,
    pub working: usize,
    pub reusable: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskKindCounts {
    pub standard: usize,
    pub context: usize,
    pub index: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskListSummary {
    pub total: usize,
    pub status_counts: TaskStatusCounts,
    pub knowledge_counts: TaskKnowledgeCounts,
    pub kind_counts: TaskKindCounts,
    pub ready_to_start_count: usize,
}

#[derive(Clone, Debug)]
pub struct TaskListPageResult {
    pub items: Vec<TaskDetail>,
    pub summary: TaskListSummary,
    pub limit: Option<usize>,
    pub next_cursor: Option<PageCursor>,
    pub has_more: bool,
    pub sort_by: TaskSortBy,
    pub sort_order: SortOrder,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskLink {
    pub relation_id: Uuid,
    pub task_id: Uuid,
    pub title: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub ready_to_start: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskContext {
    pub task: TaskDetail,
    pub notes: Vec<TaskActivity>,
    pub attachments: Vec<Attachment>,
    pub recent_activities: Vec<TaskActivity>,
    pub parent: Option<TaskLink>,
    pub children: Vec<TaskLink>,
    pub blocked_by: Vec<TaskLink>,
    pub blocking: Vec<TaskLink>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchInput {
    pub text: Option<String>,
    pub project: Option<String>,
    pub version: Option<String>,
    pub task_kind: Option<TaskKind>,
    pub task_code_prefix: Option<String>,
    pub title_prefix: Option<String>,
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
    pub fn new(
        store: SqliteStore,
        policy: PolicyEngine,
        sync: SyncConfig,
        search_config: SearchConfig,
        _data_dir: PathBuf,
    ) -> AppResult<Self> {
        let search = SearchRuntime::new(search_config)?;
        search.trigger_index_worker(store.clone());
        Ok(Self {
            store,
            policy,
            sync,
            search,
            write_queue: Arc::new(Mutex::new(())),
        })
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

    pub fn search_sidecar_autostart_enabled(&self) -> bool {
        self.search.autostart_sidecar_enabled()
    }

    pub async fn start_search_sidecar(&self) -> AppResult<()> {
        self.search.start_sidecar().await.map(|_| ())
    }

    pub async fn stop_search_sidecar(&self) -> AppResult<()> {
        self.search.stop_sidecar().await
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

    async fn collect_sorted_task_details(
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

    pub async fn search(&self, input: SearchInput) -> AppResult<SearchResponse> {
        let query_text = input.text.and_then(|value| clean_optional(Some(value)));
        let normalized_query = query_text.as_deref().and_then(normalize_search_query);
        if normalized_query.is_none()
            && input.project.is_none()
            && input.version.is_none()
            && input.task_kind.is_none()
            && input.task_code_prefix.is_none()
            && input.title_prefix.is_none()
        {
            return Err(AppError::InvalidArguments(
                "search requires query text or at least one structured filter".to_string(),
            ));
        }

        let limit = input
            .limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .clamp(1, MAX_SEARCH_LIMIT);
        let task_query = TaskQuery {
            project: input.project,
            version: input.version,
            status: None,
            task_kind: input.task_kind,
            task_code_prefix: input.task_code_prefix,
            title_prefix: input.title_prefix,
            sort_by: None,
            sort_order: None,
        };
        let filter = self.resolve_task_filter(&task_query).await?;
        let pending_index_jobs = self.store.pending_search_index_job_count().await?;
        let retrieval_mode = if normalized_query.is_none() {
            "structured_only".to_string()
        } else {
            "lexical_only".to_string()
        };

        if normalized_query.is_none() {
            let (details, _, _) = self.collect_sorted_task_details(task_query).await?;
            return Ok(SearchResponse {
                query: query_text,
                tasks: details
                    .into_iter()
                    .take(limit)
                    .map(structured_task_hit_from_detail)
                    .collect(),
                activities: Vec::new(),
                meta: SearchMeta {
                    indexed_fields: default_indexed_fields(),
                    task_sort: "structured task filter order".to_string(),
                    activity_sort: "activities are only returned for text queries".to_string(),
                    limit_applies_per_bucket: true,
                    task_limit_applied: limit,
                    activity_limit_applied: limit,
                    default_limit: DEFAULT_SEARCH_LIMIT,
                    max_limit: MAX_SEARCH_LIMIT,
                    retrieval_mode,
                    vector_backend: self.search.vector_backend_name(),
                    vector_status: vector_status_label(
                        self.search.vector_enabled(),
                        false,
                        pending_index_jobs,
                    ),
                    pending_index_jobs,
                },
            });
        }

        let normalized_query = normalized_query.expect("normalized query");
        let lexical_limit = limit
            .max(self.search.config().vector.top_k)
            .saturating_mul(4);
        let mut lexical_tasks = self
            .store
            .search_tasks(&filter, &normalized_query.fts_query, lexical_limit)
            .await?;
        let vector_hits = match self
            .search
            .query_tasks(
                &normalized_query.raw_text,
                &filter,
                self.search.config().vector.top_k,
            )
            .await
        {
            Ok(vector_hits) => vector_hits,
            Err(_) => Vec::new(),
        };
        let lexical_task_ids = lexical_tasks
            .iter()
            .map(|row| row.task_id.clone())
            .collect::<HashSet<_>>();
        let semantic_only_ids = vector_hits
            .iter()
            .filter_map(|hit| {
                (!lexical_task_ids.contains(&hit.task_id)).then_some(hit.task_id.clone())
            })
            .collect::<Vec<_>>();
        if !semantic_only_ids.is_empty() {
            let extra_rows = self.store.search_tasks_by_ids(&semantic_only_ids).await?;
            lexical_tasks.extend(
                extra_rows
                    .into_iter()
                    .filter(|row| matches_prefix_filters(row, &filter)),
            );
        }
        let mut task_sources =
            combine_task_search_results(lexical_tasks, vector_hits, &normalized_query.terms, limit);
        let used_hybrid = task_sources
            .iter()
            .any(|hit| hit.retrieval_source == "hybrid" || hit.retrieval_source == "semantic");
        let activities = self
            .store
            .search_activities(&filter, &normalized_query.fts_query, limit)
            .await?
            .into_iter()
            .map(|activity| ActivitySearchHit {
                activity_id: activity.activity_id,
                task_id: activity.task_id,
                kind: activity.kind,
                summary: activity.summary,
                score: Some(activity.score),
            })
            .collect::<Vec<_>>();
        self.search.trigger_index_worker(self.store.clone());

        Ok(SearchResponse {
            query: Some(normalized_query.raw_text.clone()),
            tasks: std::mem::take(&mut task_sources),
            activities,
            meta: SearchMeta {
                indexed_fields: default_indexed_fields(),
                task_sort: if used_hybrid {
                    "weighted RRF over sqlite fts5 bm25 and chroma semantic rank".to_string()
                } else {
                    "sqlite fts5 bm25 with structured filters and recency tiebreaks".to_string()
                },
                activity_sort: "sqlite fts5 bm25 with structured task filters applied".to_string(),
                limit_applies_per_bucket: true,
                task_limit_applied: limit,
                activity_limit_applied: limit,
                default_limit: DEFAULT_SEARCH_LIMIT,
                max_limit: MAX_SEARCH_LIMIT,
                retrieval_mode: if used_hybrid {
                    "hybrid".to_string()
                } else {
                    "lexical_only".to_string()
                },
                vector_backend: self.search.vector_backend_name(),
                vector_status: vector_status_label(
                    self.search.vector_enabled(),
                    used_hybrid,
                    pending_index_jobs,
                ),
                pending_index_jobs,
            },
        })
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
        self.search.trigger_index_worker(self.store.clone());
        Ok(task)
    }

    async fn create_child_task_internal(
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

    async fn attach_child_task_internal(
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

    async fn detach_child_task_internal(
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

    async fn add_task_blocker_internal(
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

    async fn resolve_task_blocker_internal(
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

    async fn task_detail_tx(
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

    async fn refresh_task_context_digest_tx(
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
                .upsert_search_index_job_tx(tx, task_id, OffsetDateTime::now_utc())
                .await?;
        }
        Ok(digest)
    }

    async fn refresh_task_note_rollup_tx(
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

    async fn touch_task_for_relation_change_tx(
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

    async fn append_status_change_activity_tx(
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
            created_by: created_by.to_string(),
            created_at,
            metadata_json: json!({
                "from_status": previous_status,
                "to_status": next_status,
            }),
        };
        self.store.insert_activity_tx(tx, &activity).await?;
        Ok(())
    }

    async fn append_system_activity_tx(
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
            created_by: created_by.to_string(),
            created_at,
            metadata_json,
        };
        self.store.insert_activity_tx(tx, &activity).await?;
        Ok(())
    }

    async fn task_link_for_relation(
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
            "task.create_child" | "task.attach_child" | "task.detach_child" => {
                let task_ref = json_string(&request.payload_json, "parent")
                    .or_else(|| json_string(&request.payload_json, "child"))
                    .unwrap_or_else(|| request.resource_ref.clone());
                self.task_context_from_reference(&task_ref).await
            }
            "task.add_blocker" | "task.resolve_blocker" => {
                let task_ref = json_string(&request.payload_json, "blocked")
                    .or_else(|| json_string(&request.payload_json, "task"))
                    .unwrap_or_else(|| request.resource_ref.clone());
                self.task_context_from_reference(&task_ref).await
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
            "task.create_child" => {
                let input =
                    serde_json::from_value::<CreateChildTaskInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.create_child_task_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.attach_child" => {
                let input =
                    serde_json::from_value::<AttachChildTaskInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.attach_child_task_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.detach_child" => {
                let input =
                    serde_json::from_value::<DetachChildTaskInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.detach_child_task_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.add_blocker" => {
                let input =
                    serde_json::from_value::<AddTaskBlockerInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.add_task_blocker_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.resolve_blocker" => {
                let input =
                    serde_json::from_value::<ResolveTaskBlockerInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.resolve_task_blocker_internal(input, ApprovalMode::Replay)
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
            task_kind: query.task_kind,
            task_code_prefix: query.task_code_prefix.clone(),
            title_prefix: query.title_prefix.clone(),
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

fn task_ready_to_start(task: &Task, open_blocker_count: i64) -> bool {
    !matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled) && open_blocker_count == 0
}

fn task_detail_from_parts(
    task: Task,
    note_count: i64,
    attachment_count: i64,
    latest_activity_at: OffsetDateTime,
    parent_task_id: Option<Uuid>,
    child_count: i64,
    open_blocker_count: i64,
    blocking_count: i64,
) -> TaskDetail {
    let ready_to_start = task_ready_to_start(&task, open_blocker_count);
    TaskDetail {
        task,
        note_count,
        attachment_count,
        latest_activity_at,
        parent_task_id,
        child_count,
        open_blocker_count,
        blocking_count,
        ready_to_start,
    }
}

fn build_task_context_digest_from_detail(detail: &TaskDetail) -> String {
    let digest = format!(
        "status={} priority={} task_code={} task_kind={} knowledge_status={} latest_note_summary={} ready_to_start={} parent_task_id={} child_count={} open_blocker_count={} blocking_count={} title={} summary={} description={}",
        detail.task.status,
        detail.task.priority,
        detail.task.task_code.as_deref().unwrap_or(""),
        detail.task.task_kind,
        detail.task.knowledge_status,
        detail.task.latest_note_summary.as_deref().unwrap_or(""),
        detail.ready_to_start,
        detail
            .parent_task_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
        detail.child_count,
        detail.open_blocker_count,
        detail.blocking_count,
        detail.task.title,
        detail.task.summary.as_deref().unwrap_or(""),
        detail.task.description.as_deref().unwrap_or("")
    );
    if digest.chars().count() <= 320 {
        digest
    } else {
        let mut output = digest.chars().take(319).collect::<String>();
        output.push_str("...");
        output
    }
}

fn build_task_list_summary(details: &[TaskDetail]) -> TaskListSummary {
    let mut summary = TaskListSummary {
        total: details.len(),
        status_counts: TaskStatusCounts {
            draft: 0,
            ready: 0,
            in_progress: 0,
            blocked: 0,
            done: 0,
            cancelled: 0,
        },
        knowledge_counts: TaskKnowledgeCounts {
            empty: 0,
            working: 0,
            reusable: 0,
        },
        kind_counts: TaskKindCounts {
            standard: 0,
            context: 0,
            index: 0,
        },
        ready_to_start_count: 0,
    };

    for detail in details {
        match detail.task.status {
            TaskStatus::Draft => summary.status_counts.draft += 1,
            TaskStatus::Ready => summary.status_counts.ready += 1,
            TaskStatus::InProgress => summary.status_counts.in_progress += 1,
            TaskStatus::Blocked => summary.status_counts.blocked += 1,
            TaskStatus::Done => summary.status_counts.done += 1,
            TaskStatus::Cancelled => summary.status_counts.cancelled += 1,
        }
        match detail.task.knowledge_status {
            KnowledgeStatus::Empty => summary.knowledge_counts.empty += 1,
            KnowledgeStatus::Working => summary.knowledge_counts.working += 1,
            KnowledgeStatus::Reusable => summary.knowledge_counts.reusable += 1,
        }
        match detail.task.task_kind {
            TaskKind::Standard => summary.kind_counts.standard += 1,
            TaskKind::Context => summary.kind_counts.context += 1,
            TaskKind::Index => summary.kind_counts.index += 1,
        }
        if detail.ready_to_start {
            summary.ready_to_start_count += 1;
        }
    }

    summary
}

fn default_task_sort(version_ref: Option<&str>, details: &[TaskDetail]) -> TaskSortBy {
    if version_ref.is_some()
        && details.iter().any(|detail| {
            detail
                .task
                .task_code
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        })
    {
        TaskSortBy::TaskCode
    } else {
        TaskSortBy::CreatedAt
    }
}

fn sort_task_details(details: &mut [TaskDetail], sort_by: TaskSortBy, sort_order: SortOrder) {
    details.sort_by(|left, right| {
        let ordering = match sort_by {
            TaskSortBy::CreatedAt => left.task.created_at.cmp(&right.task.created_at),
            TaskSortBy::UpdatedAt => left.task.updated_at.cmp(&right.task.updated_at),
            TaskSortBy::LatestActivityAt => left.latest_activity_at.cmp(&right.latest_activity_at),
            TaskSortBy::TaskCode => compare_task_code_fields(left, right),
            TaskSortBy::Title => compare_text(
                left.task.title.as_str(),
                right.task.title.as_str(),
                left.task.task_id,
                right.task.task_id,
            ),
        };
        match sort_order {
            SortOrder::Asc => ordering,
            SortOrder::Desc => ordering.reverse(),
        }
    });
}

fn compare_task_code_fields(left: &TaskDetail, right: &TaskDetail) -> std::cmp::Ordering {
    let left_code = left.task.task_code.as_deref();
    let right_code = right.task.task_code.as_deref();
    match (left_code, right_code) {
        (Some(left_code), Some(right_code)) => compare_task_codes(
            left_code,
            right_code,
            left.task.title.as_str(),
            right.task.title.as_str(),
            left.task.task_id,
            right.task.task_id,
        ),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => compare_text(
            left.task.title.as_str(),
            right.task.title.as_str(),
            left.task.task_id,
            right.task.task_id,
        ),
    }
}

fn compare_task_codes(
    left: &str,
    right: &str,
    left_title: &str,
    right_title: &str,
    left_id: Uuid,
    right_id: Uuid,
) -> std::cmp::Ordering {
    let left_raw = left.trim().to_ascii_lowercase();
    let right_raw = right.trim().to_ascii_lowercase();
    let left_parts = task_code_parts(&left_raw);
    let right_parts = task_code_parts(&right_raw);
    left_parts
        .0
        .cmp(&right_parts.0)
        .then_with(|| left_parts.1.cmp(&right_parts.1))
        .then_with(|| left_raw.cmp(&right_raw))
        .then_with(|| compare_text(left_title, right_title, left_id, right_id))
}

fn task_code_parts(value: &str) -> (String, u64) {
    if let Some((prefix, suffix)) = value.rsplit_once('-') {
        let prefix = prefix.trim();
        let suffix = suffix.trim();
        if !prefix.is_empty() && !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
        {
            return (
                prefix.to_string(),
                suffix.parse::<u64>().unwrap_or(u64::MAX),
            );
        }
    }
    (value.trim().to_string(), u64::MAX)
}

fn compare_text(left: &str, right: &str, left_id: Uuid, right_id: Uuid) -> std::cmp::Ordering {
    left.trim()
        .to_ascii_lowercase()
        .cmp(&right.trim().to_ascii_lowercase())
        .then_with(|| left_id.cmp(&right_id))
}

fn structured_task_hit_from_detail(detail: TaskDetail) -> TaskSearchHit {
    TaskSearchHit {
        task_id: detail.task.task_id.to_string(),
        task_code: detail.task.task_code.clone(),
        task_kind: detail.task.task_kind.to_string(),
        title: detail.task.title.clone(),
        status: detail.task.status.to_string(),
        priority: detail.task.priority.to_string(),
        knowledge_status: detail.task.knowledge_status.to_string(),
        summary: task_summary(
            detail.task.latest_note_summary.as_deref(),
            detail.task.task_search_summary.as_str(),
        ),
        retrieval_source: "structured_filter".to_string(),
        score: None,
        matched_fields: Vec::new(),
    }
}

fn combine_task_search_results(
    lexical_rows: Vec<crate::storage::TaskLexicalSearchRow>,
    semantic_rows: Vec<crate::search::VectorQueryHit>,
    terms: &[String],
    limit: usize,
) -> Vec<TaskSearchHit> {
    #[derive(Default)]
    struct CombinedTaskRow {
        lexical: Option<crate::storage::TaskLexicalSearchRow>,
        semantic_distance: Option<f64>,
        combined_score: f64,
    }

    let mut combined = HashMap::<String, CombinedTaskRow>::new();
    for (index, row) in lexical_rows.into_iter().enumerate() {
        let entry = combined.entry(row.task_id.clone()).or_default();
        entry.combined_score += weighted_rrf_score(index, LEXICAL_RRF_WEIGHT);
        entry.lexical = Some(row);
    }

    for (index, row) in semantic_rows.into_iter().enumerate() {
        let entry = combined.entry(row.task_id.clone()).or_default();
        entry.combined_score += weighted_rrf_score(index, SEMANTIC_RRF_WEIGHT);
        entry.semantic_distance = row.distance;
    }

    let mut rows = combined
        .into_values()
        .filter_map(|row| {
            row.lexical
                .map(|lexical| (lexical, row.semantic_distance, row.combined_score))
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .2
            .partial_cmp(&left.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.lexical_rank.cmp(&right.0.lexical_rank))
            .then_with(|| right.0.latest_activity_at.cmp(&left.0.latest_activity_at))
            .then_with(|| left.0.task_id.cmp(&right.0.task_id))
    });

    rows.into_iter()
        .take(limit)
        .map(|(row, semantic_distance, combined_score)| {
            let matched_fields = matched_field_names(
                terms,
                [
                    ("task_code", row.task_code.as_deref()),
                    ("title", Some(row.title.as_str())),
                    ("latest_note_summary", row.latest_note_summary.as_deref()),
                    (
                        "task_search_summary",
                        Some(row.task_search_summary.as_str()),
                    ),
                    (
                        "task_context_digest",
                        Some(row.task_context_digest.as_str()),
                    ),
                ],
            );
            let retrieval_source = match (semantic_distance.is_some(), !matched_fields.is_empty()) {
                (true, true) => "hybrid",
                (true, false) => "semantic",
                _ => "lexical",
            };
            TaskSearchHit {
                task_id: row.task_id,
                task_code: row.task_code,
                task_kind: row.task_kind,
                title: row.title,
                status: row.status,
                priority: row.priority,
                knowledge_status: row.knowledge_status,
                summary: task_summary(
                    row.latest_note_summary.as_deref(),
                    row.task_search_summary.as_str(),
                ),
                retrieval_source: retrieval_source.to_string(),
                score: Some(combined_score),
                matched_fields,
            }
        })
        .collect()
}

fn task_summary(latest_note_summary: Option<&str>, task_search_summary: &str) -> String {
    latest_note_summary
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(task_search_summary)
        .to_string()
}

fn default_indexed_fields() -> SearchIndexedFields {
    SearchIndexedFields {
        tasks: vec![
            "title".to_string(),
            "task_code".to_string(),
            "task_kind".to_string(),
            "task_search_summary".to_string(),
            "task_context_digest".to_string(),
            "latest_note_summary".to_string(),
        ],
        activities: vec!["activity_search_summary".to_string()],
    }
}

fn vector_status_label(
    vector_enabled: bool,
    used_hybrid: bool,
    pending_index_jobs: usize,
) -> String {
    if !vector_enabled {
        "disabled".to_string()
    } else if pending_index_jobs > 0 {
        "indexing".to_string()
    } else if used_hybrid {
        "ready".to_string()
    } else {
        "lexical_fallback".to_string()
    }
}

fn matches_prefix_filters(
    row: &crate::storage::TaskLexicalSearchRow,
    filter: &TaskListFilter,
) -> bool {
    if let Some(task_code_prefix) = filter.task_code_prefix.as_deref() {
        if !row
            .task_code
            .as_deref()
            .is_some_and(|value| value.starts_with(task_code_prefix))
        {
            return false;
        }
    }
    if let Some(title_prefix) = filter.title_prefix.as_deref() {
        if !row.title.starts_with(title_prefix) {
            return false;
        }
    }
    true
}

fn paginate_presorted_by_cursor<T, FCreatedAt, FId>(
    items: Vec<T>,
    page: PageRequest,
    created_at: FCreatedAt,
    id: FId,
) -> PageResult<T>
where
    FCreatedAt: Fn(&T) -> OffsetDateTime,
    FId: Fn(&T) -> Uuid,
{
    let start_index = page.cursor.and_then(|cursor| {
        items
            .iter()
            .position(|item| created_at(item) == cursor.created_at && id(item) == cursor.id)
            .map(|index| index + 1)
    });
    let mut items = if let Some(start_index) = start_index {
        items.into_iter().skip(start_index).collect::<Vec<_>>()
    } else {
        items
    };

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
