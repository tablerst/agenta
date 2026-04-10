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
    Attachment, AttachmentKind, Project, ProjectStatus, Task, TaskActivity, TaskActivityKind,
    TaskPriority, TaskStatus, Version, VersionStatus,
};
use crate::interface::response::error_to_rmcp;
use crate::search::SearchResponse;
use crate::service::{
    AgentaService, CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
    CreateVersionInput, PageCursor, PageRequest, PageResult, RequestOrigin, SearchInput,
    TaskDetail, TaskQuery, UpdateProjectInput, UpdateTaskInput, UpdateVersionInput,
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
    /// Stable sort order used to produce the page. Always `desc` for list tools.
    pub sort_order: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct CursorPayload {
    created_at: String,
    id: String,
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

/// Structured MCP representation of a task.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct TaskRecord {
    /// Stable task UUID.
    pub task_id: String,
    /// Stable project UUID that owns the task.
    pub project_id: String,
    /// Linked version UUID when one exists.
    pub version_id: Option<String>,
    /// Task title shown in task lists.
    pub title: String,
    /// Optional short summary used in overviews.
    pub summary: Option<String>,
    /// Optional long-form description.
    pub description: Option<String>,
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
    /// Pagination metadata for the list.
    pub page: PageInfo,
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
}

impl From<Task> for TaskRecord {
    fn from(task: Task) -> Self {
        Self {
            task_id: task.task_id.to_string(),
            project_id: task.project_id.to_string(),
            version_id: task.version_id.map(|value| value.to_string()),
            title: task.title,
            summary: task.summary,
            description: task.description,
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
        } = detail;
        Self {
            task_id: task.task_id.to_string(),
            project_id: task.project_id.to_string(),
            version_id: task.version_id.map(|value| value.to_string()),
            title: task.title,
            summary: task.summary,
            description: task.description,
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
        }
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
    /// Search text to run against the local task FTS index. Agenta indexes task `title`, `task_search_summary`, and activity `activity_search_summary`.
    pub query: String,
    /// Optional maximum number of matches to return per result bucket. Defaults to 10 and is clamped to the server range.
    pub limit: Option<usize>,
}

/// Structured MCP representation of a task search hit.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SearchTaskHitRecord {
    /// Stable task UUID for the hit.
    pub task_id: String,
    /// Task title.
    pub title: String,
    /// Task lifecycle status as stored by the search index.
    pub status: String,
    /// Task priority as stored by the search index.
    pub priority: String,
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
    pub query: String,
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
                    title: task.title,
                    status: task.status,
                    priority: task.priority,
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
                    tasks: vec!["title".to_string(), "task_search_summary".to_string()],
                    activities: vec!["activity_search_summary".to_string()],
                },
                task_sort: "bm25(tasks_fts) asc".to_string(),
                activity_sort: "bm25(task_activities_fts) asc".to_string(),
                limit_applies_per_bucket: true,
                task_limit_applied: applied_limit,
                activity_limit_applied: applied_limit,
                default_limit: 10,
                max_limit: 50,
            },
        }
    }
}

fn format_timestamp(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.unix_timestamp().to_string())
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
                },
                page_request,
            )
            .await
            .map(|tasks| TaskListToolOutput {
                page: page_info(&tasks, "created_at,task_id"),
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
        description = "Search the local FTS index over task titles, task_search_summary, and activity_search_summary. Returns separate task and activity buckets sorted independently by bm25 ascending; limit applies per bucket.",
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
                text: required_text(params.query, "query")?,
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
        next_cursor: page.next_cursor.as_ref().map(encode_cursor),
        has_more: page.has_more,
        sort_by: sort_by.to_string(),
        sort_order: "desc".to_string(),
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

fn encode_cursor(cursor: &PageCursor) -> String {
    let payload = CursorPayload {
        created_at: format_timestamp(cursor.created_at),
        id: cursor.id.to_string(),
    };
    let bytes = serde_json::to_vec(&payload).expect("cursor payload json");
    URL_SAFE_NO_PAD.encode(bytes)
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

        let activity_list_output =
            serde_json::to_string(&activity_list["outputSchema"]).expect("activity list output");
        assert!(activity_list_output.contains("\"metadata\""));
        assert!(activity_list_output.contains("\"page\""));

        let task_context_output =
            serde_json::to_string(&task_context_get["outputSchema"]).expect("task context output");
        assert!(task_context_output.contains("\"notes\""));
        assert!(task_context_output.contains("\"attachments\""));
        assert!(task_context_output.contains("\"recent_activities\""));
    }
}
