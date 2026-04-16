use std::path::PathBuf;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData, Json, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::McpSessionLogger;
use crate::domain::{
    Attachment, AttachmentKind, KnowledgeStatus, NoteKind, Project, ProjectStatus, Task,
    TaskActivity, TaskActivityKind, TaskKind, TaskPriority, TaskRelationKind, TaskRelationStatus,
    TaskStatus, Version, VersionStatus,
};
use crate::interface::response::error_to_rmcp;
use crate::search::SearchResponse;
use crate::service::{
    AddTaskBlockerInput, AgentaService, AttachChildTaskInput, CreateAttachmentInput,
    CreateChildTaskInput, CreateNoteInput, CreateProjectInput, CreateTaskInput, CreateVersionInput,
    DetachChildTaskInput, PageCursor, PageRequest, PageResult, RequestOrigin,
    ResolveTaskBlockerInput, SearchInput, SortOrder, TaskDetail, TaskLink, TaskListPageResult,
    TaskQuery, TaskSortBy, UpdateProjectInput, UpdateTaskInput, UpdateVersionInput,
};

#[derive(Clone)]
pub struct AgentaMcpServer {
    tool_router: ToolRouter<Self>,
    service: AgentaService,
    logger: McpSessionLogger,
}

impl AgentaMcpServer {
    pub fn new(service: AgentaService, logger: McpSessionLogger) -> Self {
        Self {
            tool_router: Self::tool_router(),
            service,
            logger,
        }
    }

    async fn log_tool_call(&self, tool: &str, action: &str) {
        let _ = self
            .logger
            .record(
                crate::app::McpLogLevel::Info,
                "mcp.tool",
                "Received MCP tool call",
                json!({
                    "tool": tool,
                    "action": action,
                }),
            )
            .await;
    }

    async fn log_structured_tool_result<T>(
        &self,
        tool: &str,
        action: &str,
        success_summary: &str,
        result: &Result<T, ErrorData>,
    ) {
        match result {
            Ok(_) => {
                let _ = self
                    .logger
                    .record(
                        crate::app::McpLogLevel::Info,
                        "mcp.tool",
                        "Completed MCP tool call",
                        json!({
                            "tool": tool,
                            "action": action,
                            "summary": success_summary,
                        }),
                    )
                    .await;
            }
            Err(error) => {
                let _ = self
                    .logger
                    .record(
                        crate::app::McpLogLevel::Error,
                        "mcp.tool",
                        "Failed MCP tool call",
                        json!({
                            "tool": tool,
                            "action": action,
                            "error": error.to_string(),
                        }),
                    )
                    .await;
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct PageInfo {
    /// Applied page size when limit-based pagination was requested. Null when the full list was returned.
    pub limit: Option<usize>,
    /// Opaque cursor for the next page. Null when the current page exhausted the result set.
    pub next_cursor: Option<String>,
    /// Whether additional results are available after this page.
    pub has_more: bool,
    /// Stable sort key used to produce the page.
    pub sort_by: String,
    /// Stable sort order used to produce the page.
    pub sort_order: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct CursorPayload {
    created_at: String,
    id: String,
    sort_by: Option<String>,
    sort_order: Option<String>,
}

/// Create a new project.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct ProjectCreateToolInput {
    /// Stable slug used to reference the project across CLI, desktop, and MCP.
    pub slug: String,
    /// Human-readable project name shown in user interfaces.
    pub name: String,
    /// Optional long-form summary explaining the purpose of the project.
    pub description: Option<String>,
}

/// Load a single project by ID or slug.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct ProjectGetToolInput {
    /// Stable project reference. Supported values: project_id UUID or slug.
    #[schemars(
        description = "Stable project reference. Supported values: project_id UUID or slug."
    )]
    pub project: String,
}

/// List projects in reverse chronological order.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct ProjectListToolInput {
    /// Optional maximum number of projects to return when paginating. Clamped to the server range when provided.
    pub limit: Option<usize>,
    /// Opaque cursor returned by a previous `project_list` call. Requires `limit` when provided.
    pub cursor: Option<String>,
}

/// Update a project in place.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct ProjectUpdateToolInput {
    /// Stable project reference to update. Supported values: project_id UUID or slug.
    #[schemars(
        description = "Stable project reference to update. Supported values: project_id UUID or slug."
    )]
    pub project: String,
    /// Replace the slug used to reference the project.
    pub slug: Option<String>,
    /// Replace the human-readable project name.
    pub name: Option<String>,
    /// Replace the long-form summary for the project.
    pub description: Option<String>,
    /// Project lifecycle status. Allowed values: `active` or `archived`. New projects default to `active`.
    #[schemars(
        description = "Project lifecycle status. Allowed values: `active` or `archived`. New projects default to `active`."
    )]
    pub status: Option<ProjectStatus>,
    /// Stable version_id UUID to mark as the project's default version.
    #[schemars(description = "Stable version_id UUID to mark as the project's default version.")]
    pub default_version: Option<String>,
}

/// Structured MCP representation of a project.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ProjectRecord {
    /// Stable project UUID.
    pub project_id: String,
    /// Stable slug used to reference the project.
    pub slug: String,
    /// Human-readable project name.
    pub name: String,
    /// Optional long-form summary for the project.
    pub description: Option<String>,
    /// Current lifecycle status.
    pub status: ProjectStatus,
    /// Default version UUID if one is configured.
    pub default_version_id: Option<String>,
    /// RFC 3339 timestamp for when the project was created.
    pub created_at: String,
    /// RFC 3339 timestamp for the most recent update.
    pub updated_at: String,
}

/// Result returned by project mutation and lookup tools.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ProjectToolOutput {
    /// The resolved project record.
    pub project: ProjectRecord,
}

/// Result returned when listing projects.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ProjectListToolOutput {
    /// Projects visible to the MCP caller.
    pub projects: Vec<ProjectRecord>,
    /// Pagination metadata for the list.
    pub page: PageInfo,
}

impl From<Project> for ProjectRecord {
    fn from(project: Project) -> Self {
        Self {
            project_id: project.project_id.to_string(),
            slug: project.slug,
            name: project.name,
            description: project.description,
            status: project.status,
            default_version_id: project.default_version_id.map(|value| value.to_string()),
            created_at: format_timestamp(project.created_at),
            updated_at: format_timestamp(project.updated_at),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct VersionCreateToolInput {
    /// Stable project reference. Supported values: project_id UUID or slug.
    #[schemars(
        description = "Stable project reference. Supported values: project_id UUID or slug."
    )]
    pub project: String,
    /// Human-readable version name.
    pub name: String,
    /// Optional long-form summary for the version.
    pub description: Option<String>,
    /// Version lifecycle status. Allowed values: `planning`, `active`, `closed`, `archived`. New versions default to `planning`.
    #[schemars(
        description = "Version lifecycle status. Allowed values: `planning`, `active`, `closed`, `archived`. New versions default to `planning`."
    )]
    pub status: Option<VersionStatus>,
}

/// Load a single version by version_id.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct VersionGetToolInput {
    /// Stable version reference. Supported values: version_id UUID only.
    #[schemars(description = "Stable version reference. Supported values: version_id UUID only.")]
    pub version: String,
}

/// List versions, optionally filtered to a project.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct VersionListToolInput {
    /// Optional project filter. Supported values: project_id UUID or slug.
    #[schemars(
        description = "Optional project filter. Supported values: project_id UUID or slug."
    )]
    pub project: Option<String>,
    /// Optional maximum number of versions to return when paginating. Clamped to the server range when provided.
    pub limit: Option<usize>,
    /// Opaque cursor returned by a previous `version_list` call. Requires `limit` when provided.
    pub cursor: Option<String>,
}

/// Update an existing version.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct VersionUpdateToolInput {
    /// Stable version reference to update. Supported values: version_id UUID only.
    #[schemars(
        description = "Stable version reference to update. Supported values: version_id UUID only."
    )]
    pub version: String,
    /// Replace the human-readable version name.
    pub name: Option<String>,
    /// Replace the long-form summary for the version.
    pub description: Option<String>,
    /// Version lifecycle status. Allowed values: `planning`, `active`, `closed`, `archived`. New versions default to `planning`.
    #[schemars(
        description = "Version lifecycle status. Allowed values: `planning`, `active`, `closed`, `archived`. New versions default to `planning`."
    )]
    pub status: Option<VersionStatus>,
}

/// Structured MCP representation of a version.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct VersionRecord {
    /// Stable version UUID.
    pub version_id: String,
    /// Stable project UUID that owns the version.
    pub project_id: String,
    /// Human-readable version name.
    pub name: String,
    /// Optional long-form summary.
    pub description: Option<String>,
    /// Current lifecycle status.
    pub status: VersionStatus,
    /// RFC 3339 timestamp for creation.
    pub created_at: String,
    /// RFC 3339 timestamp for the most recent update.
    pub updated_at: String,
}

/// Result returned by version mutation and lookup tools.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct VersionToolOutput {
    /// The resolved version record.
    pub version: VersionRecord,
}

/// Result returned when listing versions.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct VersionListToolOutput {
    /// Versions visible to the MCP caller.
    pub versions: Vec<VersionRecord>,
    /// Pagination metadata for the list.
    pub page: PageInfo,
}

impl From<Version> for VersionRecord {
    fn from(version: Version) -> Self {
        Self {
            version_id: version.version_id.to_string(),
            project_id: version.project_id.to_string(),
            name: version.name,
            description: version.description,
            status: version.status,
            created_at: format_timestamp(version.created_at),
            updated_at: format_timestamp(version.updated_at),
        }
    }
}

/// Create a new task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskCreateToolInput {
    /// Stable project reference. Supported values: project_id UUID or slug.
    #[schemars(
        description = "Stable project reference. Supported values: project_id UUID or slug."
    )]
    pub project: String,
    /// Optional linked version reference. Supported values: version_id UUID only.
    #[schemars(
        description = "Optional linked version reference. Supported values: version_id UUID only."
    )]
    pub version: Option<String>,
    /// Optional stable task code used for grouped task flows such as `InitCtx-01`.
    pub task_code: Option<String>,
    /// Optional task role used during context recovery. Allowed values: `standard`, `context`, `index`.
    pub task_kind: Option<TaskKind>,
    /// Task title shown in task lists.
    pub title: String,
    /// Optional short summary used in overviews.
    pub summary: Option<String>,
    /// Optional long-form description for the task.
    pub description: Option<String>,
    /// Task lifecycle status. Allowed values: `draft`, `ready`, `in_progress`, `blocked`, `done`, `cancelled`. New tasks default to `ready`. Setting `done` or `cancelled` records `closed_at`.
    #[schemars(
        description = "Task lifecycle status. Allowed values: `draft`, `ready`, `in_progress`, `blocked`, `done`, `cancelled`. New tasks default to `ready`. Setting `done` or `cancelled` records `closed_at`."
    )]
    pub status: Option<TaskStatus>,
    /// Task priority. Allowed values: `low`, `normal`, `high`, `critical`. New tasks default to `normal`.
    #[schemars(
        description = "Task priority. Allowed values: `low`, `normal`, `high`, `critical`. New tasks default to `normal`."
    )]
    pub priority: Option<TaskPriority>,
    /// Actor name to record as the creator. Falls back to the MCP origin actor when omitted.
    pub created_by: Option<String>,
}

/// Load a single task by task_id.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskGetToolInput {
    /// Stable task reference. Supported values: task_id UUID only.
    #[schemars(description = "Stable task reference. Supported values: task_id UUID only.")]
    pub task: String,
}

/// Load a task plus its notes, attachments, and recent activities.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskContextGetToolInput {
    /// Stable task reference. Supported values: task_id UUID only.
    #[schemars(description = "Stable task reference. Supported values: task_id UUID only.")]
    pub task: String,
    /// Optional maximum number of recent activities to include. Defaults to 20 and is clamped to the server range.
    pub recent_activity_limit: Option<usize>,
}

/// List tasks with optional project, version, and status filters.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskListToolInput {
    /// Optional project filter. Supported values: project_id UUID or slug.
    #[schemars(
        description = "Optional project filter. Supported values: project_id UUID or slug."
    )]
    pub project: Option<String>,
    /// Optional version filter. Supported values: version_id UUID only.
    #[schemars(description = "Optional version filter. Supported values: version_id UUID only.")]
    pub version: Option<String>,
    /// Optional task lifecycle status filter. Allowed values: `draft`, `ready`, `in_progress`, `blocked`, `done`, `cancelled`.
    #[schemars(
        description = "Optional task lifecycle status filter. Allowed values: `draft`, `ready`, `in_progress`, `blocked`, `done`, `cancelled`."
    )]
    pub status: Option<TaskStatus>,
    /// Optional task role filter. Allowed values: `standard`, `context`, `index`.
    pub kind: Option<TaskKind>,
    /// Optional task code prefix filter such as `InitCtx-`.
    pub task_code_prefix: Option<String>,
    /// Optional title prefix filter.
    pub title_prefix: Option<String>,
    /// Optional sort key. Allowed values: `created_at`, `updated_at`, `latest_activity_at`, `task_code`, `title`.
    pub sort_by: Option<String>,
    /// Optional sort order. Allowed values: `asc`, `desc`.
    pub sort_order: Option<String>,
    /// Optional maximum number of tasks to return when paginating. Clamped to the server range when provided.
    pub limit: Option<usize>,
    /// Opaque cursor returned by a previous `task_list` call. Requires `limit` when provided.
    pub cursor: Option<String>,
}

/// Update an existing task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskUpdateToolInput {
    /// Stable task reference to update. Supported values: task_id UUID only.
    #[schemars(
        description = "Stable task reference to update. Supported values: task_id UUID only."
    )]
    pub task: String,
    /// Replace the linked version reference. Supported values: version_id UUID only.
    #[schemars(
        description = "Replace the linked version reference. Supported values: version_id UUID only."
    )]
    pub version: Option<String>,
    /// Replace the stable task code used for grouped flows.
    pub task_code: Option<String>,
    /// Replace the task role. Allowed values: `standard`, `context`, `index`.
    pub task_kind: Option<TaskKind>,
    /// Replace the task title.
    pub title: Option<String>,
    /// Replace the short summary.
    pub summary: Option<String>,
    /// Replace the long-form description.
    pub description: Option<String>,
    /// Task lifecycle status. Allowed values: `draft`, `ready`, `in_progress`, `blocked`, `done`, `cancelled`. Setting `done` or `cancelled` records `closed_at`. When the value changes, Agenta appends a `status_change` activity.
    #[schemars(
        description = "Task lifecycle status. Allowed values: `draft`, `ready`, `in_progress`, `blocked`, `done`, `cancelled`. Setting `done` or `cancelled` records `closed_at`. When the value changes, Agenta appends a `status_change` activity."
    )]
    pub status: Option<TaskStatus>,
    /// Task priority. Allowed values: `low`, `normal`, `high`, `critical`. New tasks default to `normal`.
    #[schemars(
        description = "Task priority. Allowed values: `low`, `normal`, `high`, `critical`. New tasks default to `normal`."
    )]
    pub priority: Option<TaskPriority>,
    /// Actor name to record as the updater. Falls back to the MCP origin actor when omitted.
    pub updated_by: Option<String>,
}

/// Create a child task under an existing parent task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskCreateChildToolInput {
    /// Stable parent task reference. Supported values: task_id UUID only.
    #[schemars(description = "Stable parent task reference. Supported values: task_id UUID only.")]
    pub parent: String,
    /// Optional linked version reference. Defaults to the parent task version when omitted.
    #[schemars(
        description = "Optional linked version reference. Supported values: version_id UUID only. Defaults to the parent task version when omitted."
    )]
    pub version: Option<String>,
    /// Optional stable task code used for grouped task flows such as `InitCtx-01`.
    pub task_code: Option<String>,
    /// Optional task role used during context recovery. Allowed values: `standard`, `context`, `index`.
    pub task_kind: Option<TaskKind>,
    /// Child task title shown in task lists.
    pub title: String,
    /// Optional short summary used in overviews.
    pub summary: Option<String>,
    /// Optional long-form description for the child task.
    pub description: Option<String>,
    /// Task lifecycle status. Allowed values: `draft`, `ready`, `in_progress`, `blocked`, `done`, `cancelled`. New tasks default to `ready`.
    #[schemars(
        description = "Task lifecycle status. Allowed values: `draft`, `ready`, `in_progress`, `blocked`, `done`, `cancelled`. New child tasks default to `ready`."
    )]
    pub status: Option<TaskStatus>,
    /// Task priority. Allowed values: `low`, `normal`, `high`, `critical`. New tasks default to `normal`.
    #[schemars(
        description = "Task priority. Allowed values: `low`, `normal`, `high`, `critical`. New child tasks default to `normal`."
    )]
    pub priority: Option<TaskPriority>,
    /// Actor name to record as the creator. Falls back to the MCP origin actor when omitted.
    pub created_by: Option<String>,
}

/// Attach an existing child task to a parent task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskAttachChildToolInput {
    /// Stable parent task reference. Supported values: task_id UUID only.
    pub parent: String,
    /// Stable child task reference. Supported values: task_id UUID only.
    pub child: String,
    /// Actor name to record as the updater. Falls back to the MCP origin actor when omitted.
    pub updated_by: Option<String>,
}

/// Detach an active child task relation.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskDetachChildToolInput {
    /// Stable parent task reference. Supported values: task_id UUID only.
    pub parent: String,
    /// Stable child task reference. Supported values: task_id UUID only.
    pub child: String,
    /// Actor name to record as the updater. Falls back to the MCP origin actor when omitted.
    pub updated_by: Option<String>,
}

/// Add a blocker relation between two tasks.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskAddBlockerToolInput {
    /// Stable blocker task reference. Supported values: task_id UUID only.
    pub blocker: String,
    /// Stable blocked task reference. Supported values: task_id UUID only.
    pub blocked: String,
    /// Actor name to record as the updater. Falls back to the MCP origin actor when omitted.
    pub updated_by: Option<String>,
}

/// Resolve a blocker relation for a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskResolveBlockerToolInput {
    /// Stable blocked task reference. Supported values: task_id UUID only.
    pub task: String,
    /// Optional blocker task reference. Provide either `blocker` or `relation_id`.
    pub blocker: Option<String>,
    /// Optional relation UUID to resolve directly. Provide either `blocker` or `relation_id`.
    pub relation_id: Option<String>,
    /// Actor name to record as the updater. Falls back to the MCP origin actor when omitted.
    pub updated_by: Option<String>,
}

/// Structured MCP representation of a task.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskRecord {
    /// Stable task UUID.
    pub task_id: String,
    /// Stable project UUID that owns the task.
    pub project_id: String,
    /// Linked version UUID when one exists.
    pub version_id: Option<String>,
    /// Optional stable task code used for grouped flows such as `InitCtx-01`.
    pub task_code: Option<String>,
    /// Context recovery role for the task.
    pub task_kind: TaskKind,
    /// Task title shown in task lists.
    pub title: String,
    /// Optional short summary used in overviews.
    pub summary: Option<String>,
    /// Optional long-form description.
    pub description: Option<String>,
    /// Search-friendly summary of the latest note when one exists.
    pub latest_note_summary: Option<String>,
    /// Rollup showing whether the task has reusable knowledge.
    pub knowledge_status: KnowledgeStatus,
    /// Current lifecycle status.
    pub status: TaskStatus,
    /// Current task priority.
    pub priority: TaskPriority,
    /// Recorded creator.
    pub created_by: String,
    /// Recorded last updater.
    pub updated_by: String,
    /// RFC 3339 timestamp for creation.
    pub created_at: String,
    /// RFC 3339 timestamp for the most recent update.
    pub updated_at: String,
    /// RFC 3339 timestamp for closure when the task is closed.
    pub closed_at: Option<String>,
    /// Number of append-only note activities recorded for the task.
    pub note_count: i64,
    /// Number of attachments currently associated with the task.
    pub attachment_count: i64,
    /// RFC 3339 timestamp for the most recent task change or appended activity.
    pub latest_activity_at: String,
    /// Active parent task UUID when one exists.
    pub parent_task_id: Option<String>,
    /// Number of active child task relations.
    pub child_count: i64,
    /// Number of currently open blockers.
    pub open_blocker_count: i64,
    /// Number of tasks currently blocked by this task.
    pub blocking_count: i64,
    /// True when the task is not closed and has no open blockers.
    pub ready_to_start: bool,
}

/// Lightweight related task record returned inside task context payloads.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskLinkRecord {
    /// Active task relation UUID.
    pub relation_id: String,
    /// Related task UUID.
    pub task_id: String,
    /// Related task title.
    pub title: String,
    /// Related task lifecycle status.
    pub status: TaskStatus,
    /// Related task priority.
    pub priority: TaskPriority,
    /// Whether the related task is currently ready to start.
    pub ready_to_start: bool,
}

/// Structured MCP representation of a task relation helper result.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskRelationRecord {
    /// Stable task relation UUID.
    pub relation_id: String,
    /// Relation kind.
    pub kind: TaskRelationKind,
    /// Source task UUID. For `parent_child`, this is the parent; for `blocks`, this is the blocker.
    pub source_task_id: String,
    /// Target task UUID. For `parent_child`, this is the child; for `blocks`, this is the blocked task.
    pub target_task_id: String,
    /// Current relation lifecycle status.
    pub status: TaskRelationStatus,
    /// RFC 3339 timestamp for creation.
    pub created_at: String,
    /// RFC 3339 timestamp for the latest relation update.
    pub updated_at: String,
    /// RFC 3339 timestamp for resolution when the relation is resolved.
    pub resolved_at: Option<String>,
}

/// Result returned by task mutation and lookup tools.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskToolOutput {
    /// The resolved task record.
    pub task: TaskRecord,
}

/// Result returned when listing tasks.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskListToolOutput {
    /// Tasks visible to the MCP caller.
    pub tasks: Vec<TaskRecord>,
    /// Summary counts computed from the filtered task set before pagination.
    pub summary: TaskListSummaryRecord,
    /// Pagination metadata for the list.
    pub page: PageInfo,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskStatusCountsRecord {
    pub draft: usize,
    pub ready: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub done: usize,
    pub cancelled: usize,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskKnowledgeCountsRecord {
    pub empty: usize,
    pub working: usize,
    pub reusable: usize,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskKindCountsRecord {
    pub standard: usize,
    pub context: usize,
    pub index: usize,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskListSummaryRecord {
    pub total: usize,
    pub status_counts: TaskStatusCountsRecord,
    pub knowledge_counts: TaskKnowledgeCountsRecord,
    pub kind_counts: TaskKindCountsRecord,
    pub ready_to_start_count: usize,
}

/// Result returned by task_context_get.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskContextToolOutput {
    /// The resolved task record.
    pub task: TaskRecord,
    /// Full append-only note list for the task.
    pub notes: Vec<NoteRecord>,
    /// Full attachment list for the task.
    pub attachments: Vec<AttachmentRecord>,
    /// Recent task activities in reverse chronological order.
    pub recent_activities: Vec<ActivityRecord>,
    /// Active parent task link when one exists.
    pub parent: Option<TaskLinkRecord>,
    /// Active child task links.
    pub children: Vec<TaskLinkRecord>,
    /// Active blocker task links.
    pub blocked_by: Vec<TaskLinkRecord>,
    /// Active blocked task links.
    pub blocking: Vec<TaskLinkRecord>,
}

/// Result returned by relation helper tools.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskRelationToolOutput {
    /// The resolved relation record.
    pub relation: TaskRelationRecord,
}

impl From<Task> for TaskRecord {
    fn from(task: Task) -> Self {
        Self {
            task_id: task.task_id.to_string(),
            project_id: task.project_id.to_string(),
            version_id: task.version_id.map(|value| value.to_string()),
            task_code: task.task_code,
            task_kind: task.task_kind,
            title: task.title,
            summary: task.summary,
            description: task.description,
            latest_note_summary: task.latest_note_summary,
            knowledge_status: task.knowledge_status,
            status: task.status,
            priority: task.priority,
            created_by: task.created_by,
            updated_by: task.updated_by,
            created_at: format_timestamp(task.created_at),
            updated_at: format_timestamp(task.updated_at),
            closed_at: task.closed_at.map(format_timestamp),
            note_count: 0,
            attachment_count: 0,
            latest_activity_at: format_timestamp(task.updated_at),
            parent_task_id: None,
            child_count: 0,
            open_blocker_count: 0,
            blocking_count: 0,
            ready_to_start: !matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled),
        }
    }
}

impl From<TaskDetail> for TaskRecord {
    fn from(detail: TaskDetail) -> Self {
        let TaskDetail {
            task,
            note_count,
            attachment_count,
            latest_activity_at,
            parent_task_id,
            child_count,
            open_blocker_count,
            blocking_count,
            ready_to_start,
            ..
        } = detail;
        Self {
            task_id: task.task_id.to_string(),
            project_id: task.project_id.to_string(),
            version_id: task.version_id.map(|value| value.to_string()),
            task_code: task.task_code,
            task_kind: task.task_kind,
            title: task.title,
            summary: task.summary,
            description: task.description,
            latest_note_summary: task.latest_note_summary,
            knowledge_status: task.knowledge_status,
            status: task.status,
            priority: task.priority,
            created_by: task.created_by,
            updated_by: task.updated_by,
            created_at: format_timestamp(task.created_at),
            updated_at: format_timestamp(task.updated_at),
            closed_at: task.closed_at.map(format_timestamp),
            note_count,
            attachment_count,
            latest_activity_at: format_timestamp(latest_activity_at),
            parent_task_id: parent_task_id.map(|value| value.to_string()),
            child_count,
            open_blocker_count,
            blocking_count,
            ready_to_start,
        }
    }
}

impl From<TaskLink> for TaskLinkRecord {
    fn from(link: TaskLink) -> Self {
        Self {
            relation_id: link.relation_id.to_string(),
            task_id: link.task_id.to_string(),
            title: link.title,
            status: link.status,
            priority: link.priority,
            ready_to_start: link.ready_to_start,
        }
    }
}

impl From<crate::domain::TaskRelation> for TaskRelationRecord {
    fn from(relation: crate::domain::TaskRelation) -> Self {
        Self {
            relation_id: relation.relation_id.to_string(),
            kind: relation.kind,
            source_task_id: relation.source_task_id.to_string(),
            target_task_id: relation.target_task_id.to_string(),
            status: relation.status,
            created_at: format_timestamp(relation.created_at),
            updated_at: format_timestamp(relation.updated_at),
            resolved_at: relation.resolved_at.map(format_timestamp),
        }
    }
}

impl From<TaskRelationRecord> for TaskRelationToolOutput {
    fn from(relation: TaskRelationRecord) -> Self {
        Self { relation }
    }
}

/// Add a note to a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct NoteCreateToolInput {
    /// Stable task reference. Supported values: task_id UUID only.
    #[schemars(description = "Stable task reference. Supported values: task_id UUID only.")]
    pub task: String,
    /// Raw note content to append to the audit-friendly task activity stream.
    pub content: String,
    /// Optional note semantic role. Allowed values: `scratch`, `finding`, `conclusion`. Defaults to `finding`.
    pub note_kind: Option<NoteKind>,
    /// Actor name to record as the note author. Falls back to the MCP origin actor when omitted.
    pub created_by: Option<String>,
}

/// List append-only note activities for a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct NoteListToolInput {
    /// Stable task reference. Supported values: task_id UUID only.
    #[schemars(description = "Stable task reference. Supported values: task_id UUID only.")]
    pub task: String,
    /// Optional maximum number of notes to return when paginating. Clamped to the server range when provided.
    pub limit: Option<usize>,
    /// Opaque cursor returned by a previous `note_list` call. Requires `limit` when provided.
    pub cursor: Option<String>,
}

/// List append-only activities for a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct ActivityListToolInput {
    /// Stable task reference. Supported values: task_id UUID only.
    #[schemars(description = "Stable task reference. Supported values: task_id UUID only.")]
    pub task: String,
    /// Optional maximum number of activities to return when paginating. Clamped to the server range when provided.
    pub limit: Option<usize>,
    /// Opaque cursor returned by a previous `activity_list` call. Requires `limit` when provided.
    pub cursor: Option<String>,
}

/// Structured MCP representation of a task note.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct NoteRecord {
    /// Stable activity UUID.
    pub activity_id: String,
    /// Stable task UUID that owns the note.
    pub task_id: String,
    /// Activity kind for the record. Notes should always use `note`.
    pub kind: TaskActivityKind,
    /// Semantic role for the note.
    pub note_kind: NoteKind,
    /// Original note content.
    pub content: String,
    /// Search-oriented summary derived from the note content.
    pub summary: String,
    /// Recorded note author.
    pub created_by: String,
    /// RFC 3339 timestamp for creation.
    pub created_at: String,
}

/// Structured MCP representation of a task activity.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ActivityRecord {
    /// Stable activity UUID.
    pub activity_id: String,
    /// Stable task UUID that owns the activity.
    pub task_id: String,
    /// Activity kind.
    pub kind: TaskActivityKind,
    /// Human-readable activity content.
    pub content: String,
    /// Search-oriented summary derived from the activity content.
    pub summary: String,
    /// Recorded actor for the activity.
    pub created_by: String,
    /// RFC 3339 timestamp for creation.
    pub created_at: String,
    /// Structured metadata for the activity.
    pub metadata: Value,
}

/// Result returned by note mutation and listing tools.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct NoteToolOutput {
    /// The resolved note record.
    pub note: NoteRecord,
}

/// Result returned when listing notes.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct NoteListToolOutput {
    /// Notes visible for the selected task.
    pub notes: Vec<NoteRecord>,
    /// Pagination metadata for the list.
    pub page: PageInfo,
}

/// Result returned when listing activities.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ActivityListToolOutput {
    /// Activities visible for the selected task.
    pub activities: Vec<ActivityRecord>,
    /// Pagination metadata for the list.
    pub page: PageInfo,
}

impl From<TaskActivity> for NoteRecord {
    fn from(activity: TaskActivity) -> Self {
        Self {
            activity_id: activity.activity_id.to_string(),
            task_id: activity.task_id.to_string(),
            kind: activity.kind,
            note_kind: note_kind_for_activity(&activity),
            content: activity.content,
            summary: activity.activity_search_summary,
            created_by: activity.created_by,
            created_at: format_timestamp(activity.created_at),
        }
    }
}

impl From<TaskActivity> for ActivityRecord {
    fn from(activity: TaskActivity) -> Self {
        Self {
            activity_id: activity.activity_id.to_string(),
            task_id: activity.task_id.to_string(),
            kind: activity.kind,
            content: activity.content,
            summary: activity.activity_search_summary,
            created_by: activity.created_by,
            created_at: format_timestamp(activity.created_at),
            metadata: activity.metadata_json,
        }
    }
}

/// Add an attachment to a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct AttachmentCreateToolInput {
    /// Stable task reference. Supported values: task_id UUID only.
    #[schemars(description = "Stable task reference. Supported values: task_id UUID only.")]
    pub task: String,
    /// Absolute or relative local source file path. Agenta copies the file into managed storage and appends an attachment_ref activity.
    pub path: String,
    /// Optional attachment category.
    pub kind: Option<AttachmentKind>,
    /// Actor name to record as the uploader. Falls back to the MCP origin actor when omitted.
    pub created_by: Option<String>,
    /// Optional user-facing summary for the attachment.
    pub summary: Option<String>,
}

/// Load a single attachment by attachment_id.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct AttachmentGetToolInput {
    /// Stable attachment reference. Supported values: attachment_id UUID only.
    #[schemars(
        description = "Stable attachment reference. Supported values: attachment_id UUID only."
    )]
    pub attachment_id: String,
}

/// List attachments for a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct AttachmentListToolInput {
    /// Stable task reference. Supported values: task_id UUID only.
    #[schemars(description = "Stable task reference. Supported values: task_id UUID only.")]
    pub task: String,
    /// Optional maximum number of attachments to return when paginating. Clamped to the server range when provided.
    pub limit: Option<usize>,
    /// Opaque cursor returned by a previous `attachment_list` call. Requires `limit` when provided.
    pub cursor: Option<String>,
}

/// Structured MCP representation of an attachment.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct AttachmentRecord {
    /// Stable attachment UUID.
    pub attachment_id: String,
    /// Stable task UUID that owns the attachment.
    pub task_id: String,
    /// Attachment category.
    pub kind: AttachmentKind,
    /// MIME type detected for the attachment.
    pub mime: String,
    /// Original filename from the source path.
    pub original_filename: String,
    /// Original source path supplied during creation.
    pub original_path: String,
    /// Internal storage path managed by Agenta.
    pub storage_path: String,
    /// SHA-256 digest of the stored file.
    pub sha256: String,
    /// Stored file size in bytes.
    pub size_bytes: i64,
    /// User-facing summary for the attachment.
    pub summary: String,
    /// Recorded uploader.
    pub created_by: String,
    /// RFC 3339 timestamp for creation.
    pub created_at: String,
}

/// Result returned by attachment mutation and lookup tools.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct AttachmentToolOutput {
    /// The resolved attachment record.
    pub attachment: AttachmentRecord,
}

/// Result returned when listing attachments.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct AttachmentListToolOutput {
    /// Attachments visible for the selected task.
    pub attachments: Vec<AttachmentRecord>,
    /// Pagination metadata for the list.
    pub page: PageInfo,
}

impl From<Attachment> for AttachmentRecord {
    fn from(attachment: Attachment) -> Self {
        Self {
            attachment_id: attachment.attachment_id.to_string(),
            task_id: attachment.task_id.to_string(),
            kind: attachment.kind,
            mime: attachment.mime,
            original_filename: attachment.original_filename,
            original_path: attachment.original_path,
            storage_path: attachment.storage_path,
            sha256: attachment.sha256,
            size_bytes: attachment.size_bytes,
            summary: attachment.summary,
            created_by: attachment.created_by,
            created_at: format_timestamp(attachment.created_at),
        }
    }
}

/// Run a local full-text search across tasks and related activities.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct SearchQueryToolInput {
    /// Optional search text. When omitted, Agenta falls back to structured task filtering only.
    pub query: Option<String>,
    /// Optional project filter. Supported values: project_id UUID or slug.
    pub project: Option<String>,
    /// Optional version filter. Supported values: version_id UUID only.
    pub version: Option<String>,
    /// Optional task role filter. Allowed values: `standard`, `context`, `index`.
    pub task_kind: Option<TaskKind>,
    /// Optional task code prefix filter such as `InitCtx-`.
    pub task_code_prefix: Option<String>,
    /// Optional title prefix filter.
    pub title_prefix: Option<String>,
    /// Optional maximum number of matches to return per result bucket. Defaults to 10 and is clamped to the server range.
    pub limit: Option<usize>,
}

/// Structured MCP representation of a task search hit.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SearchTaskHitRecord {
    /// Stable task UUID for the hit.
    pub task_id: String,
    /// Optional stable task code for grouped flows.
    pub task_code: Option<String>,
    /// Task context role.
    pub task_kind: String,
    /// Task title.
    pub title: String,
    /// Task lifecycle status as stored by the search index.
    pub status: String,
    /// Task priority as stored by the search index.
    pub priority: String,
    /// Knowledge rollup as stored by the search index.
    pub knowledge_status: String,
    /// Search-oriented task summary.
    pub summary: String,
}

/// Structured MCP representation of an activity search hit.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SearchActivityHitRecord {
    /// Stable activity UUID for the hit.
    pub activity_id: String,
    /// Stable task UUID that owns the activity.
    pub task_id: String,
    /// Activity kind as stored by the search index.
    pub kind: String,
    /// Search-oriented activity summary.
    pub summary: String,
}

/// Structured MCP representation of indexed field coverage.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SearchIndexedFieldsRecord {
    /// Indexed task fields.
    pub tasks: Vec<String>,
    /// Indexed activity fields.
    pub activities: Vec<String>,
}

/// Search behavior metadata returned alongside results.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SearchMetaRecord {
    /// Indexed field groups consulted by the search query.
    pub indexed_fields: SearchIndexedFieldsRecord,
    /// Sort used for the task result bucket.
    pub task_sort: String,
    /// Sort used for the activity result bucket.
    pub activity_sort: String,
    /// Whether the requested limit is applied independently to the task and activity buckets.
    pub limit_applies_per_bucket: bool,
    /// Applied limit for the task result bucket.
    pub task_limit_applied: usize,
    /// Applied limit for the activity result bucket.
    pub activity_limit_applied: usize,
    /// Default limit used when the caller omits limit.
    pub default_limit: usize,
    /// Maximum supported limit.
    pub max_limit: usize,
}

/// Result returned by local search queries.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SearchQueryToolOutput {
    /// Original normalized search query.
    pub query: Option<String>,
    /// Task matches.
    pub tasks: Vec<SearchTaskHitRecord>,
    /// Activity matches.
    pub activities: Vec<SearchActivityHitRecord>,
    /// Search behavior metadata describing index scope and sorting.
    pub meta: SearchMetaRecord,
}

impl SearchQueryToolOutput {
    fn from_response(response: SearchResponse, applied_limit: usize) -> Self {
        Self {
            query: response.query,
            tasks: response
                .tasks
                .into_iter()
                .map(|task| SearchTaskHitRecord {
                    task_id: task.task_id,
                    task_code: task.task_code,
                    task_kind: task.task_kind,
                    title: task.title,
                    status: task.status,
                    priority: task.priority,
                    knowledge_status: task.knowledge_status,
                    summary: task.summary,
                })
                .collect(),
            activities: response
                .activities
                .into_iter()
                .map(|activity| SearchActivityHitRecord {
                    activity_id: activity.activity_id,
                    task_id: activity.task_id,
                    kind: activity.kind,
                    summary: activity.summary,
                })
                .collect(),
            meta: SearchMetaRecord {
                indexed_fields: SearchIndexedFieldsRecord {
                    tasks: vec![
                        "title".to_string(),
                        "task_code".to_string(),
                        "task_kind".to_string(),
                        "task_search_summary".to_string(),
                        "task_context_digest".to_string(),
                        "latest_note_summary".to_string(),
                    ],
                    activities: vec!["activity_search_summary".to_string()],
                },
                task_sort:
                    "prefix/exact matches first, then query score, then latest_activity_at desc"
                        .to_string(),
                activity_sort: "query match order with structured task filters applied".to_string(),
                limit_applies_per_bucket: true,
                task_limit_applied: applied_limit,
                activity_limit_applied: applied_limit,
                default_limit: 10,
                max_limit: 50,
            },
        }
    }
}

impl From<crate::service::TaskListSummary> for TaskListSummaryRecord {
    fn from(summary: crate::service::TaskListSummary) -> Self {
        Self {
            total: summary.total,
            status_counts: TaskStatusCountsRecord {
                draft: summary.status_counts.draft,
                ready: summary.status_counts.ready,
                in_progress: summary.status_counts.in_progress,
                blocked: summary.status_counts.blocked,
                done: summary.status_counts.done,
                cancelled: summary.status_counts.cancelled,
            },
            knowledge_counts: TaskKnowledgeCountsRecord {
                empty: summary.knowledge_counts.empty,
                working: summary.knowledge_counts.working,
                reusable: summary.knowledge_counts.reusable,
            },
            kind_counts: TaskKindCountsRecord {
                standard: summary.kind_counts.standard,
                context: summary.kind_counts.context,
                index: summary.kind_counts.index,
            },
            ready_to_start_count: summary.ready_to_start_count,
        }
    }
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
}

fn note_kind_for_activity(activity: &TaskActivity) -> NoteKind {
    activity
        .metadata_json
        .get("note_kind")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<NoteKind>().ok())
        .unwrap_or_default()
}

#[tool_router(router = tool_router)]
impl AgentaMcpServer {
    #[tool(
        name = "project_create",
        description = "Create a new project.",
        annotations(
            title = "Project Create",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn project_create(
        &self,
        Parameters(params): Parameters<ProjectCreateToolInput>,
    ) -> Result<Json<ProjectToolOutput>, ErrorData> {
        let action = "create";
        self.log_tool_call("project_create", action).await;
        let result = self
            .service
            .create_project_from(
                RequestOrigin::Mcp,
                CreateProjectInput {
                    slug: required_text(params.slug, "slug")?,
                    name: required_text(params.name, "name")?,
                    description: optional_trimmed(params.description),
                },
            )
            .await
            .map(|project| ProjectToolOutput {
                project: ProjectRecord::from(project),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("project_create", action, "Created project", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "project_get",
        description = "Load a project by project_id UUID or slug.",
        annotations(
            title = "Project Get",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn project_get(
        &self,
        Parameters(params): Parameters<ProjectGetToolInput>,
    ) -> Result<Json<ProjectToolOutput>, ErrorData> {
        let action = "get";
        self.log_tool_call("project_get", action).await;
        let result = self
            .service
            .get_project(&required_text(params.project, "project")?)
            .await
            .map(|project| ProjectToolOutput {
                project: ProjectRecord::from(project),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("project_get", action, "Loaded project", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "project_list",
        description = "List projects visible to the MCP caller in reverse chronological order. When limit is provided, results are paginated by created_at DESC then project_id DESC.",
        annotations(
            title = "Project List",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn project_list(
        &self,
        Parameters(params): Parameters<ProjectListToolInput>,
    ) -> Result<Json<ProjectListToolOutput>, ErrorData> {
        let action = "list";
        self.log_tool_call("project_list", action).await;
        let page_request = page_request(params.limit, params.cursor)?;
        let result = self
            .service
            .list_projects_page(page_request)
            .await
            .map(|projects| ProjectListToolOutput {
                page: page_info(&projects, "created_at,project_id"),
                projects: projects
                    .items
                    .into_iter()
                    .map(ProjectRecord::from)
                    .collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("project_list", action, "Listed projects", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "project_update",
        description = "Update an existing project by project_id UUID or slug.",
        annotations(
            title = "Project Update",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn project_update(
        &self,
        Parameters(params): Parameters<ProjectUpdateToolInput>,
    ) -> Result<Json<ProjectToolOutput>, ErrorData> {
        let action = "update";
        self.log_tool_call("project_update", action).await;
        let reference = required_text(params.project, "project")?;
        let result = self
            .service
            .update_project_from(
                RequestOrigin::Mcp,
                &reference,
                UpdateProjectInput {
                    slug: optional_trimmed(params.slug),
                    name: optional_trimmed(params.name),
                    description: optional_trimmed(params.description),
                    status: params.status,
                    default_version: optional_trimmed(params.default_version),
                },
            )
            .await
            .map(|project| ProjectToolOutput {
                project: ProjectRecord::from(project),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("project_update", action, "Updated project", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "version_create",
        description = "Create a new version for a project.",
        annotations(
            title = "Version Create",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn version_create(
        &self,
        Parameters(params): Parameters<VersionCreateToolInput>,
    ) -> Result<Json<VersionToolOutput>, ErrorData> {
        let action = "create";
        self.log_tool_call("version_create", action).await;
        let result = self
            .service
            .create_version_from(
                RequestOrigin::Mcp,
                CreateVersionInput {
                    project: required_text(params.project, "project")?,
                    name: required_text(params.name, "name")?,
                    description: optional_trimmed(params.description),
                    status: params.status,
                },
            )
            .await
            .map(|version| VersionToolOutput {
                version: VersionRecord::from(version),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("version_create", action, "Created version", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "version_get",
        description = "Load a version by version_id UUID.",
        annotations(
            title = "Version Get",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn version_get(
        &self,
        Parameters(params): Parameters<VersionGetToolInput>,
    ) -> Result<Json<VersionToolOutput>, ErrorData> {
        let action = "get";
        self.log_tool_call("version_get", action).await;
        let result = self
            .service
            .get_version(&required_text(params.version, "version")?)
            .await
            .map(|version| VersionToolOutput {
                version: VersionRecord::from(version),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("version_get", action, "Loaded version", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "version_list",
        description = "List versions, optionally filtered to a project, in reverse chronological order. When limit is provided, results are paginated by created_at DESC then version_id DESC.",
        annotations(
            title = "Version List",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn version_list(
        &self,
        Parameters(params): Parameters<VersionListToolInput>,
    ) -> Result<Json<VersionListToolOutput>, ErrorData> {
        let action = "list";
        self.log_tool_call("version_list", action).await;
        let project = optional_trimmed(params.project);
        let page_request = page_request(params.limit, params.cursor)?;
        let result = self
            .service
            .list_versions_page(project.as_deref(), page_request)
            .await
            .map(|versions| VersionListToolOutput {
                page: page_info(&versions, "created_at,version_id"),
                versions: versions
                    .items
                    .into_iter()
                    .map(VersionRecord::from)
                    .collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("version_list", action, "Listed versions", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "version_update",
        description = "Update an existing version by version_id UUID.",
        annotations(
            title = "Version Update",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn version_update(
        &self,
        Parameters(params): Parameters<VersionUpdateToolInput>,
    ) -> Result<Json<VersionToolOutput>, ErrorData> {
        let action = "update";
        self.log_tool_call("version_update", action).await;
        let reference = required_text(params.version, "version")?;
        let result = self
            .service
            .update_version_from(
                RequestOrigin::Mcp,
                &reference,
                UpdateVersionInput {
                    name: optional_trimmed(params.name),
                    description: optional_trimmed(params.description),
                    status: params.status,
                },
            )
            .await
            .map(|version| VersionToolOutput {
                version: VersionRecord::from(version),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("version_update", action, "Updated version", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_create",
        description = "Create a new task within a project. Tasks remain mutable, while notes, attachments, and activities form the append-only audit trail around them.",
        annotations(
            title = "Task Create",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn task_create(
        &self,
        Parameters(params): Parameters<TaskCreateToolInput>,
    ) -> Result<Json<TaskToolOutput>, ErrorData> {
        let action = "create";
        self.log_tool_call("task_create", action).await;
        let result: Result<TaskToolOutput, ErrorData> = async {
            let task = self
                .service
                .create_task_from(
                    RequestOrigin::Mcp,
                    CreateTaskInput {
                        project: required_text(params.project, "project")?,
                        version: optional_trimmed(params.version),
                        task_code: optional_trimmed(params.task_code),
                        task_kind: params.task_kind,
                        title: required_text(params.title, "title")?,
                        summary: optional_trimmed(params.summary),
                        description: optional_trimmed(params.description),
                        status: params.status,
                        priority: params.priority,
                        created_by: optional_trimmed(params.created_by),
                    },
                )
                .await
                .map_err(error_to_rmcp)?;
            let detail = self
                .service
                .get_task_detail(&task.task_id.to_string())
                .await
                .map_err(error_to_rmcp)?;
            Ok(TaskToolOutput {
                task: TaskRecord::from(detail),
            })
        }
        .await;
        self.log_structured_tool_result("task_create", action, "Created task", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_get",
        description = "Load a task by task_id UUID and include lightweight summary fields such as note_count, attachment_count, and latest_activity_at.",
        annotations(
            title = "Task Get",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn task_get(
        &self,
        Parameters(params): Parameters<TaskGetToolInput>,
    ) -> Result<Json<TaskToolOutput>, ErrorData> {
        let action = "get";
        self.log_tool_call("task_get", action).await;
        let result = self
            .service
            .get_task_detail(&required_text(params.task, "task")?)
            .await
            .map(|task| TaskToolOutput {
                task: TaskRecord::from(task),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("task_get", action, "Loaded task", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_context_get",
        description = "Load a task plus its full notes, full attachments, and recent activities so an agent can restore working context in one read.",
        annotations(
            title = "Task Context Get",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn task_context_get(
        &self,
        Parameters(params): Parameters<TaskContextGetToolInput>,
    ) -> Result<Json<TaskContextToolOutput>, ErrorData> {
        let action = "get_context";
        self.log_tool_call("task_context_get", action).await;
        let result = self
            .service
            .get_task_context(
                &required_text(params.task, "task")?,
                params.recent_activity_limit,
            )
            .await
            .map(|context| TaskContextToolOutput {
                task: TaskRecord::from(context.task),
                notes: context.notes.into_iter().map(NoteRecord::from).collect(),
                attachments: context
                    .attachments
                    .into_iter()
                    .map(AttachmentRecord::from)
                    .collect(),
                recent_activities: context
                    .recent_activities
                    .into_iter()
                    .map(ActivityRecord::from)
                    .collect(),
                parent: context.parent.map(TaskLinkRecord::from),
                children: context
                    .children
                    .into_iter()
                    .map(TaskLinkRecord::from)
                    .collect(),
                blocked_by: context
                    .blocked_by
                    .into_iter()
                    .map(TaskLinkRecord::from)
                    .collect(),
                blocking: context
                    .blocking
                    .into_iter()
                    .map(TaskLinkRecord::from)
                    .collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("task_context_get", action, "Loaded task context", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_list",
        description = "List tasks with optional project, version, and status filters. When limit is provided, results are paginated by created_at DESC then task_id DESC.",
        annotations(
            title = "Task List",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn task_list(
        &self,
        Parameters(params): Parameters<TaskListToolInput>,
    ) -> Result<Json<TaskListToolOutput>, ErrorData> {
        let action = "list";
        self.log_tool_call("task_list", action).await;
        let page_request = page_request(params.limit, params.cursor)?;
        let result = self
            .service
            .list_task_details_page(
                TaskQuery {
                    project: optional_trimmed(params.project),
                    version: optional_trimmed(params.version),
                    status: params.status,
                    task_kind: params.kind,
                    task_code_prefix: optional_trimmed(params.task_code_prefix),
                    title_prefix: optional_trimmed(params.title_prefix),
                    sort_by: parse_task_sort_by(params.sort_by)?,
                    sort_order: parse_sort_order(params.sort_order)?,
                },
                page_request,
            )
            .await
            .map(|tasks| TaskListToolOutput {
                page: task_page_info(&tasks),
                summary: TaskListSummaryRecord::from(tasks.summary),
                tasks: tasks.items.into_iter().map(TaskRecord::from).collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("task_list", action, "Listed tasks", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_update",
        description = "Update an existing task by task_id UUID. Status changes append a status_change activity to the task's audit trail.",
        annotations(
            title = "Task Update",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn task_update(
        &self,
        Parameters(params): Parameters<TaskUpdateToolInput>,
    ) -> Result<Json<TaskToolOutput>, ErrorData> {
        let action = "update";
        self.log_tool_call("task_update", action).await;
        let reference = required_text(params.task, "task")?;
        let result: Result<TaskToolOutput, ErrorData> = async {
            let task = self
                .service
                .update_task_from(
                    RequestOrigin::Mcp,
                    &reference,
                    UpdateTaskInput {
                        version: optional_trimmed(params.version),
                        task_code: optional_trimmed(params.task_code),
                        task_kind: params.task_kind,
                        title: optional_trimmed(params.title),
                        summary: optional_trimmed(params.summary),
                        description: optional_trimmed(params.description),
                        status: params.status,
                        priority: params.priority,
                        updated_by: optional_trimmed(params.updated_by),
                    },
                )
                .await
                .map_err(error_to_rmcp)?;
            let detail = self
                .service
                .get_task_detail(&task.task_id.to_string())
                .await
                .map_err(error_to_rmcp)?;
            Ok(TaskToolOutput {
                task: TaskRecord::from(detail),
            })
        }
        .await;
        self.log_structured_tool_result("task_update", action, "Updated task", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_create_child",
        description = "Create a child task under an existing parent task. The child inherits the parent project and defaults to the parent version when omitted.",
        annotations(
            title = "Task Create Child",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn task_create_child(
        &self,
        Parameters(params): Parameters<TaskCreateChildToolInput>,
    ) -> Result<Json<TaskToolOutput>, ErrorData> {
        let action = "create_child";
        self.log_tool_call("task_create_child", action).await;
        let result: Result<TaskToolOutput, ErrorData> = async {
            let task = self
                .service
                .create_child_task_from(
                    RequestOrigin::Mcp,
                    CreateChildTaskInput {
                        parent: required_text(params.parent, "parent")?,
                        version: optional_trimmed(params.version),
                        task_code: optional_trimmed(params.task_code),
                        task_kind: params.task_kind,
                        title: required_text(params.title, "title")?,
                        summary: optional_trimmed(params.summary),
                        description: optional_trimmed(params.description),
                        status: params.status,
                        priority: params.priority,
                        created_by: optional_trimmed(params.created_by),
                    },
                )
                .await
                .map_err(error_to_rmcp)?;
            let detail = self
                .service
                .get_task_detail(&task.task_id.to_string())
                .await
                .map_err(error_to_rmcp)?;
            Ok(TaskToolOutput {
                task: TaskRecord::from(detail),
            })
        }
        .await;
        self.log_structured_tool_result("task_create_child", action, "Created child task", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_attach_child",
        description = "Attach an existing child task to a parent task without exposing generic graph editing primitives.",
        annotations(
            title = "Task Attach Child",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn task_attach_child(
        &self,
        Parameters(params): Parameters<TaskAttachChildToolInput>,
    ) -> Result<Json<TaskRelationToolOutput>, ErrorData> {
        let action = "attach_child";
        self.log_tool_call("task_attach_child", action).await;
        let result = self
            .service
            .attach_child_task_from(
                RequestOrigin::Mcp,
                AttachChildTaskInput {
                    parent: required_text(params.parent, "parent")?,
                    child: required_text(params.child, "child")?,
                    updated_by: optional_trimmed(params.updated_by),
                },
            )
            .await
            .map(|relation| TaskRelationToolOutput {
                relation: TaskRelationRecord::from(relation),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result(
            "task_attach_child",
            action,
            "Attached child task",
            &result,
        )
        .await;

        result.map(Json)
    }

    #[tool(
        name = "task_detach_child",
        description = "Resolve an active parent-child relation between two tasks.",
        annotations(
            title = "Task Detach Child",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn task_detach_child(
        &self,
        Parameters(params): Parameters<TaskDetachChildToolInput>,
    ) -> Result<Json<TaskRelationToolOutput>, ErrorData> {
        let action = "detach_child";
        self.log_tool_call("task_detach_child", action).await;
        let result = self
            .service
            .detach_child_task_from(
                RequestOrigin::Mcp,
                DetachChildTaskInput {
                    parent: required_text(params.parent, "parent")?,
                    child: required_text(params.child, "child")?,
                    updated_by: optional_trimmed(params.updated_by),
                },
            )
            .await
            .map(|relation| TaskRelationToolOutput {
                relation: TaskRelationRecord::from(relation),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result(
            "task_detach_child",
            action,
            "Detached child task",
            &result,
        )
        .await;

        result.map(Json)
    }

    #[tool(
        name = "task_add_blocker",
        description = "Add a blocker relation between two tasks. The blocked task is moved to `blocked` when it is not already closed.",
        annotations(
            title = "Task Add Blocker",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn task_add_blocker(
        &self,
        Parameters(params): Parameters<TaskAddBlockerToolInput>,
    ) -> Result<Json<TaskRelationToolOutput>, ErrorData> {
        let action = "add_blocker";
        self.log_tool_call("task_add_blocker", action).await;
        let result = self
            .service
            .add_task_blocker_from(
                RequestOrigin::Mcp,
                AddTaskBlockerInput {
                    blocker: required_text(params.blocker, "blocker")?,
                    blocked: required_text(params.blocked, "blocked")?,
                    updated_by: optional_trimmed(params.updated_by),
                },
            )
            .await
            .map(|relation| TaskRelationToolOutput {
                relation: TaskRelationRecord::from(relation),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("task_add_blocker", action, "Added task blocker", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_resolve_blocker",
        description = "Resolve a blocker relation for a task by blocker task_id or relation_id.",
        annotations(
            title = "Task Resolve Blocker",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn task_resolve_blocker(
        &self,
        Parameters(params): Parameters<TaskResolveBlockerToolInput>,
    ) -> Result<Json<TaskRelationToolOutput>, ErrorData> {
        let action = "resolve_blocker";
        self.log_tool_call("task_resolve_blocker", action).await;
        let result = self
            .service
            .resolve_task_blocker_from(
                RequestOrigin::Mcp,
                ResolveTaskBlockerInput {
                    task: required_text(params.task, "task")?,
                    blocker: optional_trimmed(params.blocker),
                    relation_id: optional_trimmed(params.relation_id),
                    updated_by: optional_trimmed(params.updated_by),
                },
            )
            .await
            .map(|relation| TaskRelationToolOutput {
                relation: TaskRelationRecord::from(relation),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result(
            "task_resolve_blocker",
            action,
            "Resolved task blocker",
            &result,
        )
        .await;

        result.map(Json)
    }

    #[tool(
        name = "note_create",
        description = "Append a note to a task. Notes are append-only, audit-friendly activity records.",
        annotations(
            title = "Note Create",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn note_create(
        &self,
        Parameters(params): Parameters<NoteCreateToolInput>,
    ) -> Result<Json<NoteToolOutput>, ErrorData> {
        let action = "create";
        self.log_tool_call("note_create", action).await;
        let result = self
            .service
            .create_note_from(
                RequestOrigin::Mcp,
                CreateNoteInput {
                    task: required_text(params.task, "task")?,
                    content: required_text(params.content, "content")?,
                    note_kind: params.note_kind,
                    created_by: optional_trimmed(params.created_by),
                },
            )
            .await
            .map(|note| NoteToolOutput {
                note: NoteRecord::from(note),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("note_create", action, "Created note", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "note_list",
        description = "List append-only note activities for a task in reverse chronological order. When limit is provided, results are paginated by created_at DESC then activity_id DESC.",
        annotations(
            title = "Note List",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn note_list(
        &self,
        Parameters(params): Parameters<NoteListToolInput>,
    ) -> Result<Json<NoteListToolOutput>, ErrorData> {
        let action = "list";
        self.log_tool_call("note_list", action).await;
        let page_request = page_request(params.limit, params.cursor)?;
        let result = self
            .service
            .list_notes_page(&required_text(params.task, "task")?, page_request)
            .await
            .map(|notes| NoteListToolOutput {
                page: page_info(&notes, "created_at,activity_id"),
                notes: notes.items.into_iter().map(NoteRecord::from).collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("note_list", action, "Listed notes", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "activity_list",
        description = "List append-only task activities in reverse chronological order, including notes, attachment references, and status changes. When limit is provided, results are paginated by created_at DESC then activity_id DESC.",
        annotations(
            title = "Activity List",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn activity_list(
        &self,
        Parameters(params): Parameters<ActivityListToolInput>,
    ) -> Result<Json<ActivityListToolOutput>, ErrorData> {
        let action = "list";
        self.log_tool_call("activity_list", action).await;
        let page_request = page_request(params.limit, params.cursor)?;
        let result = self
            .service
            .list_task_activities_page(&required_text(params.task, "task")?, page_request)
            .await
            .map(|activities| ActivityListToolOutput {
                page: page_info(&activities, "created_at,activity_id"),
                activities: activities
                    .items
                    .into_iter()
                    .map(ActivityRecord::from)
                    .collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("activity_list", action, "Listed activities", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "attachment_create",
        description = "Add an attachment to a task from a local file path. Agenta copies the file into managed storage and appends an attachment_ref activity.",
        annotations(
            title = "Attachment Create",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn attachment_create(
        &self,
        Parameters(params): Parameters<AttachmentCreateToolInput>,
    ) -> Result<Json<AttachmentToolOutput>, ErrorData> {
        let action = "create";
        self.log_tool_call("attachment_create", action).await;
        let result = self
            .service
            .create_attachment_from(
                RequestOrigin::Mcp,
                CreateAttachmentInput {
                    task: required_text(params.task, "task")?,
                    path: PathBuf::from(required_text(params.path, "path")?),
                    kind: params.kind,
                    created_by: optional_trimmed(params.created_by),
                    summary: optional_trimmed(params.summary),
                },
            )
            .await
            .map(|attachment| AttachmentToolOutput {
                attachment: AttachmentRecord::from(attachment),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("attachment_create", action, "Created attachment", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "attachment_get",
        description = "Load an attachment by attachment_id UUID.",
        annotations(
            title = "Attachment Get",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn attachment_get(
        &self,
        Parameters(params): Parameters<AttachmentGetToolInput>,
    ) -> Result<Json<AttachmentToolOutput>, ErrorData> {
        let action = "get";
        self.log_tool_call("attachment_get", action).await;
        let result = self
            .service
            .get_attachment(&required_text(params.attachment_id, "attachment_id")?)
            .await
            .map(|attachment| AttachmentToolOutput {
                attachment: AttachmentRecord::from(attachment),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("attachment_get", action, "Loaded attachment", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "attachment_list",
        description = "List attachments for a task in reverse chronological order. When limit is provided, results are paginated by created_at DESC then attachment_id DESC.",
        annotations(
            title = "Attachment List",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn attachment_list(
        &self,
        Parameters(params): Parameters<AttachmentListToolInput>,
    ) -> Result<Json<AttachmentListToolOutput>, ErrorData> {
        let action = "list";
        self.log_tool_call("attachment_list", action).await;
        let page_request = page_request(params.limit, params.cursor)?;
        let result = self
            .service
            .list_attachments_page(&required_text(params.task, "task")?, page_request)
            .await
            .map(|attachments| AttachmentListToolOutput {
                page: page_info(&attachments, "created_at,attachment_id"),
                attachments: attachments
                    .items
                    .into_iter()
                    .map(AttachmentRecord::from)
                    .collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("attachment_list", action, "Listed attachments", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "search_query",
        description = "Search tasks and related activities using structured filters plus optional query text. Task matches favor grouped task flow prefixes and reusable note summaries.",
        annotations(
            title = "Search Query",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn search_query(
        &self,
        Parameters(params): Parameters<SearchQueryToolInput>,
    ) -> Result<Json<SearchQueryToolOutput>, ErrorData> {
        let action = "query";
        self.log_tool_call("search_query", action).await;
        let applied_limit = params.limit.unwrap_or(10).clamp(1, 50);
        let result = self
            .service
            .search(SearchInput {
                text: optional_trimmed(params.query),
                project: optional_trimmed(params.project),
                version: optional_trimmed(params.version),
                task_kind: params.task_kind,
                task_code_prefix: optional_trimmed(params.task_code_prefix),
                title_prefix: optional_trimmed(params.title_prefix),
                limit: params.limit,
            })
            .await
            .map(|response| SearchQueryToolOutput::from_response(response, applied_limit))
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("search_query", action, "Completed search", &result)
            .await;

        result.map(Json)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AgentaMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::LATEST)
            .with_instructions(
                "Agenta manages local projects, versions, tasks, notes, attachments, and search backed by SQLite.",
            )
    }
}

fn required(value: Option<String>, field: &str) -> Result<String, ErrorData> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(ErrorData::invalid_params(
            format!("missing required field: {field}"),
            None,
        )),
    }
}

fn required_text(value: String, field: &str) -> Result<String, ErrorData> {
    required(Some(value), field)
}

fn optional_trimmed(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn page_request(limit: Option<usize>, cursor: Option<String>) -> Result<PageRequest, ErrorData> {
    if cursor.is_some() && limit.is_none() {
        return Err(ErrorData::invalid_params(
            "cursor requires limit to be provided".to_string(),
            None,
        ));
    }

    Ok(PageRequest {
        limit,
        cursor: cursor.map(decode_cursor).transpose()?,
    })
}

fn page_info<T>(page: &PageResult<T>, sort_by: &str) -> PageInfo {
    PageInfo {
        limit: page.limit,
        next_cursor: page
            .next_cursor
            .as_ref()
            .map(|cursor| encode_cursor(cursor, None, None)),
        has_more: page.has_more,
        sort_by: sort_by.to_string(),
        sort_order: "desc".to_string(),
    }
}

fn task_page_info(page: &TaskListPageResult) -> PageInfo {
    PageInfo {
        limit: page.limit,
        next_cursor: page
            .next_cursor
            .as_ref()
            .map(|cursor| encode_cursor(cursor, Some(page.sort_by), Some(page.sort_order))),
        has_more: page.has_more,
        sort_by: page.sort_by.to_string(),
        sort_order: page.sort_order.to_string(),
    }
}

fn decode_cursor(cursor: String) -> Result<PageCursor, ErrorData> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor.as_bytes()).map_err(|error| {
        ErrorData::invalid_params(format!("invalid cursor encoding: {error}"), None)
    })?;
    let payload: CursorPayload = serde_json::from_slice(&bytes).map_err(|error| {
        ErrorData::invalid_params(format!("invalid cursor payload: {error}"), None)
    })?;
    let created_at = OffsetDateTime::parse(&payload.created_at, &Rfc3339).map_err(|error| {
        ErrorData::invalid_params(format!("invalid cursor timestamp: {error}"), None)
    })?;
    let id = Uuid::parse_str(&payload.id)
        .map_err(|error| ErrorData::invalid_params(format!("invalid cursor id: {error}"), None))?;
    Ok(PageCursor { created_at, id })
}

fn encode_cursor(
    cursor: &PageCursor,
    sort_by: Option<TaskSortBy>,
    sort_order: Option<SortOrder>,
) -> String {
    let payload = CursorPayload {
        created_at: format_timestamp(cursor.created_at),
        id: cursor.id.to_string(),
        sort_by: sort_by.map(|value| value.to_string()),
        sort_order: sort_order.map(|value| value.to_string()),
    };
    let bytes = serde_json::to_vec(&payload).expect("cursor payload json");
    URL_SAFE_NO_PAD.encode(bytes)
}

fn parse_task_sort_by(value: Option<String>) -> Result<Option<TaskSortBy>, ErrorData> {
    value
        .map(|value| {
            value
                .parse::<TaskSortBy>()
                .map_err(|error| ErrorData::invalid_params(error, None))
        })
        .transpose()
}

fn parse_sort_order(value: Option<String>) -> Result<Option<SortOrder>, ErrorData> {
    value
        .map(|value| {
            value
                .parse::<SortOrder>()
                .map_err(|error| ErrorData::invalid_params(error, None))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::AgentaMcpServer;

    fn is_min_compat_tool_name(name: &str) -> bool {
        let mut chars = name.chars();
        match chars.next() {
            Some(first) if first.is_ascii_alphabetic() => {}
            _ => return false,
        }

        name.len() <= 64 && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    }

    #[test]
    fn project_tools_follow_min_compat_contract() {
        let tools = [
            AgentaMcpServer::project_create_tool_attr(),
            AgentaMcpServer::project_get_tool_attr(),
            AgentaMcpServer::project_list_tool_attr(),
            AgentaMcpServer::project_update_tool_attr(),
            AgentaMcpServer::version_create_tool_attr(),
            AgentaMcpServer::version_get_tool_attr(),
            AgentaMcpServer::version_list_tool_attr(),
            AgentaMcpServer::version_update_tool_attr(),
            AgentaMcpServer::task_create_tool_attr(),
            AgentaMcpServer::task_get_tool_attr(),
            AgentaMcpServer::task_context_get_tool_attr(),
            AgentaMcpServer::task_list_tool_attr(),
            AgentaMcpServer::task_update_tool_attr(),
            AgentaMcpServer::task_create_child_tool_attr(),
            AgentaMcpServer::task_attach_child_tool_attr(),
            AgentaMcpServer::task_detach_child_tool_attr(),
            AgentaMcpServer::task_add_blocker_tool_attr(),
            AgentaMcpServer::task_resolve_blocker_tool_attr(),
            AgentaMcpServer::note_create_tool_attr(),
            AgentaMcpServer::note_list_tool_attr(),
            AgentaMcpServer::activity_list_tool_attr(),
            AgentaMcpServer::attachment_create_tool_attr(),
            AgentaMcpServer::attachment_get_tool_attr(),
            AgentaMcpServer::attachment_list_tool_attr(),
            AgentaMcpServer::search_query_tool_attr(),
        ];

        for tool in tools {
            assert!(
                is_min_compat_tool_name(tool.name.as_ref()),
                "tool name must satisfy cross-provider min-compat rule"
            );
            assert!(
                !tool.name.contains('.'),
                "tool name must not contain dots for min-compat"
            );
        }

        let create_tool =
            serde_json::to_value(AgentaMcpServer::project_create_tool_attr()).expect("tool json");
        assert_eq!(create_tool["description"], "Create a new project.");
        assert!(
            create_tool["inputSchema"]["properties"]
                .get("action")
                .is_none(),
            "explicit tools must not expose legacy action multiplexing"
        );
        assert_eq!(create_tool["annotations"]["readOnlyHint"], false);
    }

    #[test]
    fn project_update_schema_exposes_status_enum_values() {
        let update_tool =
            serde_json::to_value(AgentaMcpServer::project_update_tool_attr()).expect("tool json");
        let input_schema =
            serde_json::to_string(&update_tool["inputSchema"]).expect("input schema string");

        assert!(input_schema.contains("\"active\""));
        assert!(input_schema.contains("\"archived\""));
        assert!(input_schema.contains("default to `active`"));
    }

    #[test]
    fn version_and_task_schemas_expose_enum_values() {
        let version_create =
            serde_json::to_value(AgentaMcpServer::version_create_tool_attr()).expect("tool json");
        let version_schema =
            serde_json::to_string(&version_create["inputSchema"]).expect("version schema string");
        assert!(version_schema.contains("\"planning\""));
        assert!(version_schema.contains("\"active\""));
        assert!(version_schema.contains("\"closed\""));
        assert!(version_schema.contains("\"archived\""));

        let task_create =
            serde_json::to_value(AgentaMcpServer::task_create_tool_attr()).expect("tool json");
        let task_schema =
            serde_json::to_string(&task_create["inputSchema"]).expect("task schema string");
        assert!(task_schema.contains("\"draft\""));
        assert!(task_schema.contains("\"ready\""));
        assert!(task_schema.contains("\"in_progress\""));
        assert!(task_schema.contains("\"blocked\""));
        assert!(task_schema.contains("\"done\""));
        assert!(task_schema.contains("\"cancelled\""));
        assert!(task_schema.contains("\"low\""));
        assert!(task_schema.contains("\"normal\""));
        assert!(task_schema.contains("\"high\""));
        assert!(task_schema.contains("\"critical\""));
        assert!(task_schema.contains("default to `ready`"));
        assert!(task_schema.contains("default to `normal`"));
    }

    #[test]
    fn attachment_and_search_schemas_follow_explicit_contract() {
        let attachment_create =
            serde_json::to_value(AgentaMcpServer::attachment_create_tool_attr())
                .expect("tool json");
        let attachment_schema = serde_json::to_string(&attachment_create["inputSchema"])
            .expect("attachment schema string");
        assert!(attachment_schema.contains("\"screenshot\""));
        assert!(attachment_schema.contains("\"image\""));
        assert!(attachment_schema.contains("\"artifact\""));

        let search_query =
            serde_json::to_value(AgentaMcpServer::search_query_tool_attr()).expect("tool json");
        assert!(
            search_query["inputSchema"]["properties"]
                .get("action")
                .is_none(),
            "search_query must not expose legacy action multiplexing"
        );
        assert_eq!(
            search_query["annotations"]["readOnlyHint"], true,
            "search_query should be explicitly marked read-only"
        );
        let search_output =
            serde_json::to_string(&search_query["outputSchema"]).expect("search output schema");
        assert!(search_output.contains("\"meta\""));
    }

    #[test]
    fn list_and_context_tools_expose_page_and_summary_fields() {
        let project_list =
            serde_json::to_value(AgentaMcpServer::project_list_tool_attr()).expect("tool json");
        let task_list =
            serde_json::to_value(AgentaMcpServer::task_list_tool_attr()).expect("tool json");
        let task_get =
            serde_json::to_value(AgentaMcpServer::task_get_tool_attr()).expect("tool json");
        let activity_list =
            serde_json::to_value(AgentaMcpServer::activity_list_tool_attr()).expect("tool json");
        let task_context_get =
            serde_json::to_value(AgentaMcpServer::task_context_get_tool_attr()).expect("tool json");

        let project_list_input =
            serde_json::to_string(&project_list["inputSchema"]).expect("project list input");
        let project_list_output =
            serde_json::to_string(&project_list["outputSchema"]).expect("project list output");
        assert!(project_list_input.contains("\"limit\""));
        assert!(project_list_input.contains("\"cursor\""));
        assert!(project_list_output.contains("\"page\""));

        let task_list_input =
            serde_json::to_string(&task_list["inputSchema"]).expect("task list input");
        let task_list_output =
            serde_json::to_string(&task_list["outputSchema"]).expect("task list output");
        let task_get_output =
            serde_json::to_string(&task_get["outputSchema"]).expect("task get output");
        assert!(task_list_input.contains("\"limit\""));
        assert!(task_list_input.contains("\"cursor\""));
        assert!(task_list_output.contains("\"page\""));
        assert!(task_get_output.contains("\"note_count\""));
        assert!(task_get_output.contains("\"attachment_count\""));
        assert!(task_get_output.contains("\"latest_activity_at\""));
        assert!(task_get_output.contains("\"parent_task_id\""));
        assert!(task_get_output.contains("\"open_blocker_count\""));
        assert!(task_get_output.contains("\"ready_to_start\""));

        let activity_list_output =
            serde_json::to_string(&activity_list["outputSchema"]).expect("activity list output");
        assert!(activity_list_output.contains("\"metadata\""));
        assert!(activity_list_output.contains("\"page\""));

        let task_context_output =
            serde_json::to_string(&task_context_get["outputSchema"]).expect("task context output");
        assert!(task_context_output.contains("\"notes\""));
        assert!(task_context_output.contains("\"attachments\""));
        assert!(task_context_output.contains("\"recent_activities\""));
        assert!(task_context_output.contains("\"blocked_by\""));
        assert!(task_context_output.contains("\"blocking\""));
        assert!(task_context_output.contains("\"children\""));
    }
}
