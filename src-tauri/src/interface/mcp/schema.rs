use super::*;

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

/// Initialize or update a project context manifest in a workspace directory.
#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct ContextInitToolInput {
    /// Optional project reference. Supported values: project_id UUID or slug.
    pub project: Option<String>,
    /// Optional workspace root used to resolve configured context paths.
    pub workspace_root: Option<String>,
    /// Optional explicit context directory. Relative paths resolve against workspace_root when provided.
    pub context_dir: Option<String>,
    /// Optional instructions entrypoint written into the manifest. Defaults to `README.md`.
    pub instructions: Option<String>,
    /// Optional memory directory written into the manifest. Defaults to `memory`.
    pub memory_dir: Option<String>,
    /// When true, overwrite an existing manifest if its contents differ.
    pub force: Option<bool>,
    /// When true, do not write files and only report the resolved target and outcome.
    pub dry_run: Option<bool>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ContextInitRecord {
    pub project: String,
    pub context_dir: String,
    pub manifest_path: String,
    pub status: String,
    pub used_defaults: bool,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ContextInitToolOutput {
    pub context: ContextInitRecord,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct CursorPayload {
    pub(super) created_at: String,
    pub(super) id: String,
    pub(super) sort_by: Option<String>,
    pub(super) sort_order: Option<String>,
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

impl From<ContextInitResult> for ContextInitRecord {
    fn from(result: ContextInitResult) -> Self {
        Self {
            project: result.project,
            context_dir: result.context_dir.display().to_string(),
            manifest_path: result.manifest_path.display().to_string(),
            status: result.status.to_string(),
            used_defaults: result.used_defaults,
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
    /// When true, allow listing across every project instead of requiring a single current project scope.
    pub all_projects: Option<bool>,
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
    /// When true, allow searching across every project instead of requiring a single current project scope.
    pub all_projects: Option<bool>,
    /// Optional version filter. Supported values: version_id UUID only.
    pub version: Option<String>,
    /// Optional task lifecycle status filter.
    pub status: Option<TaskStatus>,
    /// Optional task priority filter.
    pub priority: Option<TaskPriority>,
    /// Optional knowledge rollup filter.
    pub knowledge_status: Option<KnowledgeStatus>,
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
    /// Retrieval lane that produced the hit.
    pub retrieval_source: String,
    /// Optional combined rank score used for ordering.
    pub score: Option<f64>,
    /// Indexed fields that matched the lexical query terms.
    pub matched_fields: Vec<String>,
    /// Primary evidence field used to explain the hit.
    pub evidence_source: Option<String>,
    /// Short evidence snippet aligned with the primary evidence field.
    pub evidence_snippet: Option<String>,
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
    /// Optional lexical score used for ordering.
    pub score: Option<f64>,
    /// Indexed fields that matched the lexical query terms.
    pub matched_fields: Vec<String>,
    /// Primary evidence field used to explain the hit.
    pub evidence_source: Option<String>,
    /// Short evidence snippet aligned with the primary evidence field.
    pub evidence_snippet: Option<String>,
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
    /// Effective retrieval mode for the task bucket.
    pub retrieval_mode: String,
    /// Active vector backend when semantic retrieval is enabled.
    pub vector_backend: Option<String>,
    /// Current vector runtime status.
    pub vector_status: String,
    /// Pending task vector index jobs still waiting to be processed.
    pub pending_index_jobs: usize,
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
    pub(super) fn from_response(response: SearchResponse, applied_limit: usize) -> Self {
        let crate::search::SearchResponse {
            query,
            tasks,
            activities,
            meta,
        } = response;
        Self {
            query,
            tasks: tasks
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
                    retrieval_source: task.retrieval_source,
                    score: task.score,
                    matched_fields: task.matched_fields,
                    evidence_source: task.evidence_source,
                    evidence_snippet: task.evidence_snippet,
                })
                .collect(),
            activities: activities
                .into_iter()
                .map(|activity| SearchActivityHitRecord {
                    activity_id: activity.activity_id,
                    task_id: activity.task_id,
                    kind: activity.kind,
                    summary: activity.summary,
                    score: activity.score,
                    matched_fields: activity.matched_fields,
                    evidence_source: activity.evidence_source,
                    evidence_snippet: activity.evidence_snippet,
                })
                .collect(),
            meta: SearchMetaRecord {
                indexed_fields: SearchIndexedFieldsRecord {
                    tasks: meta.indexed_fields.tasks,
                    activities: meta.indexed_fields.activities,
                },
                task_sort: meta.task_sort,
                activity_sort: meta.activity_sort,
                limit_applies_per_bucket: meta.limit_applies_per_bucket,
                task_limit_applied: applied_limit,
                activity_limit_applied: applied_limit,
                default_limit: meta.default_limit,
                max_limit: meta.max_limit,
                retrieval_mode: meta.retrieval_mode,
                vector_backend: meta.vector_backend,
                vector_status: meta.vector_status,
                pending_index_jobs: meta.pending_index_jobs,
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
