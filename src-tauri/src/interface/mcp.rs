use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, Json, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::app::AppRuntime;
use crate::interface::response::{SuccessEnvelope, error_to_rmcp, success};
use crate::service::{
    CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput, CreateVersionInput,
    SearchInput, TaskQuery, UpdateProjectInput, UpdateTaskInput, UpdateVersionInput,
};

#[derive(Clone)]
pub struct AgentaMcpServer {
    tool_router: ToolRouter<Self>,
    runtime: Arc<AppRuntime>,
}

impl AgentaMcpServer {
    pub fn new(runtime: Arc<AppRuntime>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            runtime,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct ProjectToolInput {
    pub action: String,
    pub project: Option<String>,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub default_version: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct VersionToolInput {
    pub action: String,
    pub version: Option<String>,
    pub project: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct TaskToolInput {
    pub action: String,
    pub task: Option<String>,
    pub project: Option<String>,
    pub version: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct NoteToolInput {
    pub action: String,
    pub task: Option<String>,
    pub content: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct AttachmentToolInput {
    pub action: String,
    pub task: Option<String>,
    pub path: Option<String>,
    pub kind: Option<String>,
    pub created_by: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct SearchToolInput {
    pub action: String,
    pub text: Option<String>,
    pub limit: Option<usize>,
}

#[tool_router(router = tool_router)]
impl AgentaMcpServer {
    #[tool(description = "Manage project entities")]
    pub async fn project(
        &self,
        Parameters(params): Parameters<ProjectToolInput>,
    ) -> Result<Json<SuccessEnvelope>, ErrorData> {
        let service = &self.runtime.service;
        let envelope = match params.action.as_str() {
            "create" => success(
                "project.create",
                service
                    .create_project(CreateProjectInput {
                        slug: required(params.slug, "slug")?,
                        name: required(params.name, "name")?,
                        description: params.description,
                    })
                    .await
                    .map_err(error_to_rmcp)?,
                "Created project",
            ),
            "get" => success(
                "project.get",
                service
                    .get_project(&required(params.project, "project")?)
                    .await
                    .map_err(error_to_rmcp)?,
                "Loaded project",
            ),
            "list" => {
                let items = service.list_projects().await.map_err(error_to_rmcp)?;
                success("project.list", &items, format!("Listed {} project(s)", items.len()))
            }
            "update" => success(
                "project.update",
                service
                    .update_project(
                        &required(params.project, "project")?,
                        UpdateProjectInput {
                            slug: params.slug,
                            name: params.name,
                            description: params.description,
                            status: parse_optional_enum(params.status)?,
                            default_version: params.default_version,
                        },
                    )
                    .await
                    .map_err(error_to_rmcp)?,
                "Updated project",
            ),
            other => Err(crate::error::AppError::InvalidAction(format!(
                "unsupported project action: {other}"
            ))),
        }
        .map_err(error_to_rmcp)?;

        Ok(Json(envelope))
    }

    #[tool(description = "Manage version entities")]
    pub async fn version(
        &self,
        Parameters(params): Parameters<VersionToolInput>,
    ) -> Result<Json<SuccessEnvelope>, ErrorData> {
        let service = &self.runtime.service;
        let envelope = match params.action.as_str() {
            "create" => success(
                "version.create",
                service
                    .create_version(CreateVersionInput {
                        project: required(params.project, "project")?,
                        name: required(params.name, "name")?,
                        description: params.description,
                        status: parse_optional_enum(params.status)?,
                    })
                    .await
                    .map_err(error_to_rmcp)?,
                "Created version",
            ),
            "get" => success(
                "version.get",
                service
                    .get_version(&required(params.version, "version")?)
                    .await
                    .map_err(error_to_rmcp)?,
                "Loaded version",
            ),
            "list" => {
                let items = service
                    .list_versions(params.project.as_deref())
                    .await
                    .map_err(error_to_rmcp)?;
                success("version.list", &items, format!("Listed {} version(s)", items.len()))
            }
            "update" => success(
                "version.update",
                service
                    .update_version(
                        &required(params.version, "version")?,
                        UpdateVersionInput {
                            name: params.name,
                            description: params.description,
                            status: parse_optional_enum(params.status)?,
                        },
                    )
                    .await
                    .map_err(error_to_rmcp)?,
                "Updated version",
            ),
            other => Err(crate::error::AppError::InvalidAction(format!(
                "unsupported version action: {other}"
            ))),
        }
        .map_err(error_to_rmcp)?;

        Ok(Json(envelope))
    }

    #[tool(description = "Manage task entities")]
    pub async fn task(
        &self,
        Parameters(params): Parameters<TaskToolInput>,
    ) -> Result<Json<SuccessEnvelope>, ErrorData> {
        let service = &self.runtime.service;
        let envelope = match params.action.as_str() {
            "create" => success(
                "task.create",
                service
                    .create_task(CreateTaskInput {
                        project: required(params.project, "project")?,
                        version: params.version,
                        title: required(params.title, "title")?,
                        summary: params.summary,
                        description: params.description,
                        status: parse_optional_enum(params.status)?,
                        priority: parse_optional_enum(params.priority)?,
                        created_by: params.created_by,
                    })
                    .await
                    .map_err(error_to_rmcp)?,
                "Created task",
            ),
            "get" => success(
                "task.get",
                service
                    .get_task(&required(params.task, "task")?)
                    .await
                    .map_err(error_to_rmcp)?,
                "Loaded task",
            ),
            "list" => {
                let items = service
                    .list_tasks(TaskQuery {
                        project: params.project,
                        version: params.version,
                        status: parse_optional_enum(params.status)?,
                    })
                    .await
                    .map_err(error_to_rmcp)?;
                success("task.list", &items, format!("Listed {} task(s)", items.len()))
            }
            "update" => success(
                "task.update",
                service
                    .update_task(
                        &required(params.task, "task")?,
                        UpdateTaskInput {
                            version: params.version,
                            title: params.title,
                            summary: params.summary,
                            description: params.description,
                            status: parse_optional_enum(params.status)?,
                            priority: parse_optional_enum(params.priority)?,
                            updated_by: params.updated_by,
                        },
                    )
                    .await
                    .map_err(error_to_rmcp)?,
                "Updated task",
            ),
            other => Err(crate::error::AppError::InvalidAction(format!(
                "unsupported task action: {other}"
            ))),
        }
        .map_err(error_to_rmcp)?;

        Ok(Json(envelope))
    }

    #[tool(description = "Manage task notes and activity timeline items")]
    pub async fn note(
        &self,
        Parameters(params): Parameters<NoteToolInput>,
    ) -> Result<Json<SuccessEnvelope>, ErrorData> {
        let service = &self.runtime.service;
        let envelope = match params.action.as_str() {
            "create" => success(
                "note.create",
                service
                    .create_note(CreateNoteInput {
                        task: required(params.task, "task")?,
                        content: required(params.content, "content")?,
                        created_by: params.created_by,
                    })
                    .await
                    .map_err(error_to_rmcp)?,
                "Created note",
            ),
            "list" => {
                let items = service
                    .list_task_activities(&required(params.task, "task")?)
                    .await
                    .map_err(error_to_rmcp)?;
                success("note.list", &items, format!("Listed {} activity item(s)", items.len()))
            }
            other => Err(crate::error::AppError::InvalidAction(format!(
                "unsupported note action: {other}"
            ))),
        }
        .map_err(error_to_rmcp)?;

        Ok(Json(envelope))
    }

    #[tool(description = "Manage task attachments")]
    pub async fn attachment(
        &self,
        Parameters(params): Parameters<AttachmentToolInput>,
    ) -> Result<Json<SuccessEnvelope>, ErrorData> {
        let service = &self.runtime.service;
        let envelope = match params.action.as_str() {
            "create" => success(
                "attachment.create",
                service
                    .create_attachment(CreateAttachmentInput {
                        task: required(params.task, "task")?,
                        path: PathBuf::from(required(params.path, "path")?),
                        kind: parse_optional_enum(params.kind)?,
                        created_by: params.created_by,
                        summary: params.summary,
                    })
                    .await
                    .map_err(error_to_rmcp)?,
                "Created attachment",
            ),
            "list" => {
                let items = service
                    .list_attachments(&required(params.task, "task")?)
                    .await
                    .map_err(error_to_rmcp)?;
                success(
                    "attachment.list",
                    &items,
                    format!("Listed {} attachment(s)", items.len()),
                )
            }
            other => Err(crate::error::AppError::InvalidAction(format!(
                "unsupported attachment action: {other}"
            ))),
        }
        .map_err(error_to_rmcp)?;

        Ok(Json(envelope))
    }

    #[tool(description = "Search across tasks and task activities")]
    pub async fn search(
        &self,
        Parameters(params): Parameters<SearchToolInput>,
    ) -> Result<Json<SuccessEnvelope>, ErrorData> {
        let service = &self.runtime.service;
        let envelope = match params.action.as_str() {
            "query" => success(
                "search.query",
                service
                    .search(SearchInput {
                        text: required(params.text, "text")?,
                        limit: params.limit,
                    })
                    .await
                    .map_err(error_to_rmcp)?,
                "Completed search",
            ),
            other => Err(crate::error::AppError::InvalidAction(format!(
                "unsupported search action: {other}"
            ))),
        }
        .map_err(error_to_rmcp)?;

        Ok(Json(envelope))
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

fn parse_optional_enum<T>(value: Option<String>) -> Result<Option<T>, ErrorData>
where
    T: FromStr<Err = String>,
{
    value
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|error| ErrorData::invalid_params(error, None))
        })
        .transpose()
}
