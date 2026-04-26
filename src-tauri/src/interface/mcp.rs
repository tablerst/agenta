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
    AddTaskBlockerInput, AgentaService, AttachChildTaskInput, ContextInitInput, ContextInitResult,
    CreateAttachmentInput, CreateChildTaskInput, CreateNoteInput, CreateProjectInput,
    CreateTaskInput, CreateVersionInput, DetachChildTaskInput, PageCursor, PageRequest, PageResult,
    RequestOrigin, ResolveTaskBlockerInput, SearchEvidenceInput, SearchInput, SortOrder,
    TaskContextOptions, TaskDetail, TaskLink, TaskListPageResult, TaskQuery, TaskSortBy,
    UpdateProjectInput, UpdateTaskInput, UpdateVersionInput,
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

mod helpers;
mod schema;

use helpers::*;
pub use schema::*;

#[tool_router(router = tool_router)]
impl AgentaMcpServer {
    #[tool(
        name = "context_init",
        description = "Initialize or update a project context manifest in a workspace or explicit context directory.",
        annotations(
            title = "Context Init",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn context_init(
        &self,
        Parameters(params): Parameters<ContextInitToolInput>,
    ) -> Result<Json<ContextInitToolOutput>, ErrorData> {
        let action = "init";
        self.log_tool_call("context_init", action).await;
        let result = self
            .service
            .init_project_context(ContextInitInput {
                project: optional_trimmed(params.project),
                workspace_root: optional_trimmed(params.workspace_root).map(PathBuf::from),
                context_dir: optional_trimmed(params.context_dir).map(PathBuf::from),
                instructions: optional_trimmed(params.instructions),
                memory_dir: optional_trimmed(params.memory_dir),
                entry_task_id: optional_trimmed(params.entry_task_id),
                entry_task_code: optional_trimmed(params.entry_task_code),
                force: params.force.unwrap_or(false),
                dry_run: params.dry_run.unwrap_or(false),
            })
            .await
            .map(|result| ContextInitToolOutput {
                context: ContextInitRecord::from(result),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result(
            "context_init",
            action,
            "Initialized project context",
            &result,
        )
        .await;

        result.map(Json)
    }

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
            .get_task_context_with_options(
                &required_text(params.task, "task")?,
                TaskContextOptions {
                    recent_activity_limit: params.recent_activity_limit,
                    include_notes: params.include_notes.unwrap_or(true),
                    notes_limit: params.notes_limit,
                    include_attachments: params.include_attachments.unwrap_or(true),
                    attachments_limit: params.attachments_limit,
                },
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
                    all_projects: params.all_projects.unwrap_or(false),
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
                status: params.status,
                priority: params.priority,
                knowledge_status: params.knowledge_status,
                task_kind: params.task_kind,
                task_code_prefix: optional_trimmed(params.task_code_prefix),
                title_prefix: optional_trimmed(params.title_prefix),
                limit: params.limit,
                all_projects: params.all_projects.unwrap_or(false),
            })
            .await
            .map(|response| SearchQueryToolOutput::from_response(response, applied_limit))
            .map_err(error_to_rmcp);
        self.log_structured_tool_result("search_query", action, "Completed search", &result)
            .await;

        result.map(Json)
    }

    #[tool(
        name = "search_evidence_get",
        description = "Load text for a search evidence source returned by search_query.",
        annotations(
            title = "Search Evidence Get",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn search_evidence_get(
        &self,
        Parameters(params): Parameters<SearchEvidenceGetToolInput>,
    ) -> Result<Json<SearchEvidenceGetToolOutput>, ErrorData> {
        let action = "get_evidence";
        self.log_tool_call("search_evidence_get", action).await;
        let result = self
            .service
            .get_search_evidence(SearchEvidenceInput {
                chunk_id: optional_trimmed(params.chunk_id),
                attachment_id: optional_trimmed(params.attachment_id),
            })
            .await
            .map(|evidence| SearchEvidenceGetToolOutput {
                evidence: SearchEvidenceRecord::from(evidence),
            })
            .map_err(error_to_rmcp);
        self.log_structured_tool_result(
            "search_evidence_get",
            action,
            "Loaded search evidence",
            &result,
        )
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
            AgentaMcpServer::context_init_tool_attr(),
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
            AgentaMcpServer::search_evidence_get_tool_attr(),
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
        let context_tool =
            serde_json::to_value(AgentaMcpServer::context_init_tool_attr()).expect("tool json");
        let context_input =
            serde_json::to_string(&context_tool["inputSchema"]).expect("context input schema");
        let context_output =
            serde_json::to_string(&context_tool["outputSchema"]).expect("context output schema");
        assert!(context_input.contains("\"entry_task_id\""));
        assert!(context_input.contains("\"entry_task_code\""));
        assert!(context_output.contains("\"entry_task_id\""));
        assert!(context_output.contains("\"entry_task_code\""));
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
        assert!(search_output.contains("\"semantic_attempted\""));
        assert!(search_output.contains("\"semantic_error\""));
        assert!(search_output.contains("\"evidence_activity_id\""));
        assert!(search_output.contains("\"evidence_chunk_id\""));
        assert!(search_output.contains("\"evidence_attachment_id\""));

        let search_evidence =
            serde_json::to_value(AgentaMcpServer::search_evidence_get_tool_attr())
                .expect("tool json");
        assert_eq!(search_evidence["annotations"]["readOnlyHint"], true);
        let evidence_input =
            serde_json::to_string(&search_evidence["inputSchema"]).expect("evidence input schema");
        let evidence_output = serde_json::to_string(&search_evidence["outputSchema"])
            .expect("evidence output schema");
        assert!(evidence_input.contains("\"chunk_id\""));
        assert!(evidence_input.contains("\"attachment_id\""));
        assert!(evidence_output.contains("\"text\""));
        assert!(evidence_output.contains("\"source_kind\""));
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
        let task_context_input =
            serde_json::to_string(&task_context_get["inputSchema"]).expect("task context input");
        assert!(task_list_input.contains("\"limit\""));
        assert!(task_list_input.contains("\"cursor\""));
        assert!(task_list_output.contains("\"page\""));
        assert!(task_get_output.contains("\"task_search_summary\""));
        assert!(task_get_output.contains("\"task_context_digest\""));
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
        assert!(task_context_input.contains("\"include_notes\""));
        assert!(task_context_input.contains("\"notes_limit\""));
        assert!(task_context_input.contains("\"include_attachments\""));
        assert!(task_context_input.contains("\"attachments_limit\""));
        assert!(task_context_output.contains("\"notes\""));
        assert!(task_context_output.contains("\"attachments\""));
        assert!(task_context_output.contains("\"recent_activities\""));
        assert!(task_context_output.contains("\"blocked_by\""));
        assert!(task_context_output.contains("\"blocking\""));
        assert!(task_context_output.contains("\"children\""));
    }
}
