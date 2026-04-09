use std::path::PathBuf;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData, Json, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::app::McpSessionLogger;
use crate::domain::{
    Attachment, AttachmentKind, Project, ProjectStatus, Task, TaskActivity, TaskActivityKind,
    TaskPriority, TaskStatus, Version, VersionStatus,
};
use crate::interface::response::error_to_rmcp;
use crate::search::SearchResponse;
use crate::service::{
    AgentaService, CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
    CreateVersionInput, RequestOrigin, SearchInput, TaskQuery, UpdateProjectInput, UpdateTaskInput,
    UpdateVersionInput,
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
    /// Project UUID or slug.
    #[schemars(description = "Project UUID or slug.")]
    pub project: String,
}

/// Update a project in place.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct ProjectUpdateToolInput {
    /// Project UUID or slug to update.
    #[schemars(description = "Project UUID or slug to update.")]
    pub project: String,
    /// Replace the slug used to reference the project.
    pub slug: Option<String>,
    /// Replace the human-readable project name.
    pub name: Option<String>,
    /// Replace the long-form summary for the project.
    pub description: Option<String>,
    /// New lifecycle status for the project.
    pub status: Option<ProjectStatus>,
    /// Version UUID to mark as the project's default version.
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
    /// Project UUID or slug that owns the version.
    pub project: String,
    /// Human-readable version name.
    pub name: String,
    /// Optional long-form summary for the version.
    pub description: Option<String>,
    /// Initial lifecycle status for the version.
    pub status: Option<VersionStatus>,
}

/// Load a single version by UUID or reference.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct VersionGetToolInput {
    /// Version UUID or reference.
    pub version: String,
}

/// List versions, optionally filtered to a project.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct VersionListToolInput {
    /// Optional project UUID or slug to filter versions by owning project.
    pub project: Option<String>,
}

/// Update an existing version.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct VersionUpdateToolInput {
    /// Version UUID or reference to update.
    pub version: String,
    /// Replace the human-readable version name.
    pub name: Option<String>,
    /// Replace the long-form summary for the version.
    pub description: Option<String>,
    /// New lifecycle status for the version.
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
    /// Project UUID or slug that owns the task.
    pub project: String,
    /// Optional version UUID or reference linked to the task.
    pub version: Option<String>,
    /// Task title shown in task lists.
    pub title: String,
    /// Optional short summary used in overviews.
    pub summary: Option<String>,
    /// Optional long-form description for the task.
    pub description: Option<String>,
    /// Initial lifecycle status for the task.
    pub status: Option<TaskStatus>,
    /// Initial priority for the task.
    pub priority: Option<TaskPriority>,
    /// Actor name to record as the creator. Falls back to the MCP origin actor when omitted.
    pub created_by: Option<String>,
}

/// Load a single task by UUID or reference.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskGetToolInput {
    /// Task UUID or reference.
    pub task: String,
}

/// List tasks with optional project, version, and status filters.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskListToolInput {
    /// Optional project UUID or slug filter.
    pub project: Option<String>,
    /// Optional version UUID or reference filter.
    pub version: Option<String>,
    /// Optional lifecycle status filter.
    pub status: Option<TaskStatus>,
}

/// Update an existing task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskUpdateToolInput {
    /// Task UUID or reference to update.
    pub task: String,
    /// Replace the linked version UUID or reference.
    pub version: Option<String>,
    /// Replace the task title.
    pub title: Option<String>,
    /// Replace the short summary.
    pub summary: Option<String>,
    /// Replace the long-form description.
    pub description: Option<String>,
    /// New lifecycle status for the task.
    pub status: Option<TaskStatus>,
    /// New priority for the task.
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
        }
    }
}

/// Add a note to a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct NoteCreateToolInput {
    /// Task UUID or reference that will receive the note.
    pub task: String,
    /// Raw note content to append to the task activity stream.
    pub content: String,
    /// Actor name to record as the note author. Falls back to the MCP origin actor when omitted.
    pub created_by: Option<String>,
}

/// List notes for a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct NoteListToolInput {
    /// Task UUID or reference whose notes should be returned.
    pub task: String,
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

/// Add an attachment to a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct AttachmentCreateToolInput {
    /// Task UUID or reference that will own the attachment.
    pub task: String,
    /// Source file path on the local machine.
    pub path: String,
    /// Optional attachment category.
    pub kind: Option<AttachmentKind>,
    /// Actor name to record as the uploader. Falls back to the MCP origin actor when omitted.
    pub created_by: Option<String>,
    /// Optional user-facing summary for the attachment.
    pub summary: Option<String>,
}

/// Load a single attachment by UUID or reference.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct AttachmentGetToolInput {
    /// Attachment UUID or reference.
    pub attachment_id: String,
}

/// List attachments for a task.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct AttachmentListToolInput {
    /// Task UUID or reference whose attachments should be returned.
    pub task: String,
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
    /// Search text to run against the local task index.
    pub query: String,
    /// Optional maximum number of matches to return. Clamped to the server range.
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

/// Result returned by local search queries.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SearchQueryToolOutput {
    /// Original normalized search query.
    pub query: String,
    /// Task matches.
    pub tasks: Vec<SearchTaskHitRecord>,
    /// Activity matches.
    pub activities: Vec<SearchActivityHitRecord>,
}

impl From<SearchResponse> for SearchQueryToolOutput {
    fn from(response: SearchResponse) -> Self {
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
        description = "Load a project by UUID or slug.",
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
        description = "List all projects visible to the MCP caller.",
        annotations(
            title = "Project List",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn project_list(&self) -> Result<Json<ProjectListToolOutput>, ErrorData> {
        let action = "list";
        self.log_tool_call("project_list", action).await;
        let result = self
            .service
            .list_projects()
            .await
            .map(|projects| ProjectListToolOutput {
                projects: projects.into_iter().map(ProjectRecord::from).collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("project_list", action, "Listed projects", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "project_update",
        description = "Update an existing project by UUID or slug.",
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
        description = "Load a version by UUID or reference.",
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
        description = "List versions, optionally filtered to a project.",
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
        let result = self
            .service
            .list_versions(project.as_deref())
            .await
            .map(|versions| VersionListToolOutput {
                versions: versions.into_iter().map(VersionRecord::from).collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("version_list", action, "Listed versions", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "version_update",
        description = "Update an existing version by UUID or reference.",
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
        description = "Create a new task within a project.",
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
        let result = self
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
            .map(|task| TaskToolOutput {
                task: TaskRecord::from(task),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("task_create", action, "Created task", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_get",
        description = "Load a task by UUID or reference.",
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
            .get_task(&required_text(params.task, "task")?)
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
        name = "task_list",
        description = "List tasks with optional project, version, and status filters.",
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
        let result = self
            .service
            .list_tasks(TaskQuery {
                project: optional_trimmed(params.project),
                version: optional_trimmed(params.version),
                status: params.status,
            })
            .await
            .map(|tasks| TaskListToolOutput {
                tasks: tasks.into_iter().map(TaskRecord::from).collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("task_list", action, "Listed tasks", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "task_update",
        description = "Update an existing task by UUID or reference.",
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
        let result = self
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
            .map(|task| TaskToolOutput {
                task: TaskRecord::from(task),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("task_update", action, "Updated task", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "note_create",
        description = "Add a note to a task.",
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
        description = "List notes for a task.",
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
        let result = self
            .service
            .list_notes(&required_text(params.task, "task")?)
            .await
            .map(|notes| NoteListToolOutput {
                notes: notes.into_iter().map(NoteRecord::from).collect(),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("note_list", action, "Listed notes", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "attachment_create",
        description = "Add an attachment to a task from a local file path.",
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
        description = "Load an attachment by UUID or reference.",
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
        description = "List attachments for a task.",
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
        let result = self
            .service
            .list_attachments(&required_text(params.task, "task")?)
            .await
            .map(|attachments| AttachmentListToolOutput {
                attachments: attachments
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
        description = "Search local tasks and task activities using the built-in full-text index.",
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
        let result = self
            .service
            .search(SearchInput {
                text: required_text(params.query, "query")?,
                limit: params.limit,
            })
            .await
            .map(SearchQueryToolOutput::from)
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
            AgentaMcpServer::task_list_tool_attr(),
            AgentaMcpServer::task_update_tool_attr(),
            AgentaMcpServer::note_create_tool_attr(),
            AgentaMcpServer::note_list_tool_attr(),
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
    }
}
