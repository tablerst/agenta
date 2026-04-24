use super::*;

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

#[derive(Clone, Debug, Serialize)]
pub struct SearchBackfillSummary {
    pub run_id: Uuid,
    pub status: String,
    pub operation_kind: String,
    pub operation_description: String,
    pub scanned: usize,
    pub queued: usize,
    pub skipped: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub pending_after: usize,
    pub processing_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchIndexRunSummary {
    pub run_id: Uuid,
    pub status: String,
    pub trigger_kind: String,
    pub operation_kind: String,
    pub operation_description: String,
    pub scanned: usize,
    pub queued: usize,
    pub skipped: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub batch_size: usize,
    pub pending_count: usize,
    pub processing_count: usize,
    pub retrying_count: usize,
    pub remaining_count: usize,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchIndexJobSummary {
    pub task_id: Uuid,
    pub title: Option<String>,
    pub status: String,
    pub attempt_count: i64,
    pub last_error: Option<String>,
    pub next_attempt_at: Option<OffsetDateTime>,
    pub locked_at: Option<OffsetDateTime>,
    pub lease_until: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
    pub run_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchIndexStatusSummary {
    pub enabled: bool,
    pub vector_available: bool,
    pub sidecar: String,
    pub total_count: usize,
    pub pending_count: usize,
    pub processing_count: usize,
    pub failed_count: usize,
    pub due_count: usize,
    pub stale_processing_count: usize,
    pub next_retry_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub active_run: Option<SearchIndexRunSummary>,
    pub latest_run: Option<SearchIndexRunSummary>,
    pub failed_jobs: Vec<SearchIndexJobSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchQueueRecoverySummary {
    pub run_id: Uuid,
    pub status: String,
    pub trigger_kind: String,
    pub operation_kind: String,
    pub operation_description: String,
    pub queued: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub pending_after: usize,
    pub processing_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextInitStatus {
    Created,
    Updated,
    Unchanged,
    WouldCreate,
    WouldUpdate,
}

impl ContextInitStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Unchanged => "unchanged",
            Self::WouldCreate => "would_create",
            Self::WouldUpdate => "would_update",
        }
    }
}

impl std::fmt::Display for ContextInitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ContextInitInput {
    pub project: Option<String>,
    pub workspace_root: Option<PathBuf>,
    pub context_dir: Option<PathBuf>,
    pub instructions: Option<String>,
    pub memory_dir: Option<String>,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextInitResult {
    pub project: String,
    pub context_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub status: ContextInitStatus,
    pub used_defaults: bool,
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
    pub all_projects: bool,
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
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub knowledge_status: Option<KnowledgeStatus>,
    pub task_kind: Option<TaskKind>,
    pub task_code_prefix: Option<String>,
    pub title_prefix: Option<String>,
    pub limit: Option<usize>,
    pub all_projects: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ApprovalQuery {
    pub project: Option<String>,
    pub status: Option<ApprovalStatus>,
    pub all_projects: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ProjectContextManifest {
    pub(super) project: Option<String>,
    pub(super) instructions: Option<String>,
    pub(super) memory_dir: Option<String>,
}

#[derive(Debug)]
pub(super) struct ContextInitTarget {
    pub(super) context_dir: PathBuf,
    pub(super) manifest_path: PathBuf,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ReviewApprovalInput {
    pub reviewed_by: Option<String>,
    pub review_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ApprovalSeed {
    pub(super) requested_via: ApprovalRequestedVia,
    pub(super) resource_ref: String,
    pub(super) payload_json: Value,
    pub(super) request_summary: String,
    pub(super) requested_by: String,
}

#[derive(Default)]
pub(super) struct ApprovalContext {
    pub(super) project_ref: Option<String>,
    pub(super) project_name: Option<String>,
    pub(super) task_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ReferencedUpdatePayload<T> {
    pub(super) reference: String,
    pub(super) input: T,
}

#[derive(Clone, Debug)]
pub(super) enum ApprovalMode {
    Standard(ApprovalSeed),
    Replay,
}

impl RequestOrigin {
    pub(super) fn requested_via(self) -> ApprovalRequestedVia {
        match self {
            Self::Cli => ApprovalRequestedVia::Cli,
            Self::Mcp => ApprovalRequestedVia::Mcp,
            Self::Desktop => ApprovalRequestedVia::Desktop,
        }
    }

    pub(super) fn fallback_actor(self) -> &'static str {
        self.requested_via().as_str()
    }
}
