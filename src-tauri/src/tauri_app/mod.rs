use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::app::{
    save_mcp_config_defaults, AppRuntime, McpLaunchOverrides, McpLogDestination, McpLogLevel,
    McpLogSnapshot, McpRuntimeStatus, McpSupervisor, MCP_LOG_EVENT, MCP_STATUS_EVENT,
};
use crate::domain::ApprovalStatus;
use crate::error::AppError;
use crate::interface::response::{error, success, ErrorEnvelope, SuccessEnvelope};
use crate::service::{
    ApprovalQuery, CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
    CreateVersionInput, RequestOrigin, ReviewApprovalInput, SearchInput, TaskQuery,
    UpdateProjectInput, UpdateTaskInput, UpdateVersionInput,
};

#[derive(Debug, Serialize)]
struct DesktopRuntimeStatus {
    data_dir: String,
    database_path: String,
    attachments_dir: String,
    loaded_config_path: Option<String>,
    mcp_bind: String,
    mcp_path: String,
    project_count: i64,
    task_count: i64,
    pending_approval_count: usize,
}

struct DesktopAppState {
    runtime: Arc<AppRuntime>,
    mcp_supervisor: Arc<McpSupervisor>,
}

impl DesktopAppState {
    fn new(runtime: Arc<AppRuntime>) -> Self {
        let mcp_supervisor = Arc::new(McpSupervisor::new(runtime.clone()));
        Self {
            runtime,
            mcp_supervisor,
        }
    }

    async fn autostart_mcp_if_configured(&self) -> Result<Option<McpRuntimeStatus>, AppError> {
        if !self.mcp_supervisor.default_config().autostart {
            return Ok(None);
        }

        let status = self
            .mcp_supervisor
            .start(McpLaunchOverrides::default())
            .await?;
        Ok(Some(status))
    }

    fn spawn_mcp_autostart(self: Arc<Self>) {
        if !self.mcp_supervisor.default_config().autostart {
            return;
        }

        tauri::async_runtime::spawn(async move {
            let _ = self.autostart_mcp_if_configured().await;
        });
    }
}

impl Deref for DesktopAppState {
    type Target = AppRuntime;

    fn deref(&self) -> &Self::Target {
        &self.runtime
    }
}

#[derive(Debug, Deserialize, Default)]
struct DesktopProjectInput {
    action: String,
    project: Option<String>,
    slug: Option<String>,
    name: Option<String>,
    description: Option<String>,
    status: Option<String>,
    default_version: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DesktopVersionInput {
    action: String,
    version: Option<String>,
    project: Option<String>,
    name: Option<String>,
    description: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DesktopTaskInput {
    action: String,
    task: Option<String>,
    project: Option<String>,
    version: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    status: Option<String>,
    priority: Option<String>,
    created_by: Option<String>,
    updated_by: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DesktopNoteInput {
    action: String,
    task: Option<String>,
    content: Option<String>,
    created_by: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DesktopAttachmentInput {
    action: String,
    task: Option<String>,
    attachment_id: Option<String>,
    path: Option<String>,
    kind: Option<String>,
    created_by: Option<String>,
    summary: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DesktopSearchInput {
    action: String,
    text: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
struct DesktopApprovalInput {
    action: String,
    project: Option<String>,
    request_id: Option<String>,
    status: Option<String>,
    reviewed_by: Option<String>,
    review_note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DesktopMcpStartInput {
    bind: Option<String>,
    path: Option<String>,
    autostart: Option<bool>,
    log_level: Option<McpLogLevel>,
    log_destinations: Option<Vec<McpLogDestination>>,
    log_file_path: Option<PathBuf>,
    log_ui_buffer_lines: Option<usize>,
    save_as_default: Option<bool>,
}

impl DesktopMcpStartInput {
    fn into_overrides(self) -> McpLaunchOverrides {
        McpLaunchOverrides {
            bind: self.bind,
            path: self.path,
            autostart: self.autostart,
            log_level: self.log_level,
            log_destinations: self.log_destinations,
            log_file_path: self.log_file_path,
            log_ui_buffer_lines: self.log_ui_buffer_lines,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct DesktopMcpLogsSnapshotInput {
    limit: Option<usize>,
}

#[tauri::command]
async fn desktop_status(
    state: State<'_, Arc<DesktopAppState>>,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let counts = state
        .service
        .service_overview()
        .await
        .map_err(|app_error| error(&app_error))?;
    let pending = state
        .service
        .list_approval_requests(ApprovalQuery {
            project: None,
            status: Some(ApprovalStatus::Pending),
        })
        .await
        .map_err(|app_error| error(&app_error))?;
    let default_mcp_config = state.mcp_supervisor.default_config();

    success(
        "desktop.status",
        DesktopRuntimeStatus {
            data_dir: state.config.paths.data_dir.display().to_string(),
            database_path: state.config.paths.database_path.display().to_string(),
            attachments_dir: state.config.paths.attachments_dir.display().to_string(),
            loaded_config_path: state
                .config
                .paths
                .loaded_config_path
                .as_ref()
                .map(|path| path.display().to_string()),
            mcp_bind: default_mcp_config.bind,
            mcp_path: default_mcp_config.path,
            project_count: counts.project_count,
            task_count: counts.task_count,
            pending_approval_count: pending.len(),
        },
        "Loaded desktop runtime status",
    )
    .map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_project(
    state: State<'_, Arc<DesktopAppState>>,
    input: DesktopProjectInput,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let result: Result<SuccessEnvelope, AppError> = async {
        let service = &state.service;
        match input.action.as_str() {
            "create" => success(
                "project.create",
                service
                    .create_project_from(
                        RequestOrigin::Desktop,
                        CreateProjectInput {
                            slug: required(input.slug, "slug")?,
                            name: required(input.name, "name")?,
                            description: input.description,
                        },
                    )
                    .await?,
                "Created project",
            ),
            "get" => success(
                "project.get",
                service
                    .get_project(&project_reference(input.project, input.slug)?)
                    .await?,
                "Loaded project",
            ),
            "list" => {
                let items = service.list_projects().await?;
                success(
                    "project.list",
                    &items,
                    format!("Listed {} project(s)", items.len()),
                )
            }
            "update" => success(
                "project.update",
                service
                    .update_project_from(
                        RequestOrigin::Desktop,
                        &required(input.project, "project")?,
                        UpdateProjectInput {
                            slug: input.slug,
                            name: input.name,
                            description: input.description,
                            status: parse_optional_enum(input.status)?,
                            default_version: input.default_version,
                        },
                    )
                    .await?,
                "Updated project",
            ),
            other => Err(AppError::InvalidAction(format!(
                "unsupported project action: {other}"
            ))),
        }
    }
    .await;

    result.map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_version(
    state: State<'_, Arc<DesktopAppState>>,
    input: DesktopVersionInput,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let result: Result<SuccessEnvelope, AppError> = async {
        let service = &state.service;
        match input.action.as_str() {
            "create" => success(
                "version.create",
                service
                    .create_version_from(
                        RequestOrigin::Desktop,
                        CreateVersionInput {
                            project: required(input.project, "project")?,
                            name: required(input.name, "name")?,
                            description: input.description,
                            status: parse_optional_enum(input.status)?,
                        },
                    )
                    .await?,
                "Created version",
            ),
            "get" => success(
                "version.get",
                service
                    .get_version(&required(input.version, "version")?)
                    .await?,
                "Loaded version",
            ),
            "list" => {
                let items = service.list_versions(input.project.as_deref()).await?;
                success(
                    "version.list",
                    &items,
                    format!("Listed {} version(s)", items.len()),
                )
            }
            "update" => success(
                "version.update",
                service
                    .update_version_from(
                        RequestOrigin::Desktop,
                        &required(input.version, "version")?,
                        UpdateVersionInput {
                            name: input.name,
                            description: input.description,
                            status: parse_optional_enum(input.status)?,
                        },
                    )
                    .await?,
                "Updated version",
            ),
            other => Err(AppError::InvalidAction(format!(
                "unsupported version action: {other}"
            ))),
        }
    }
    .await;

    result.map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_task(
    state: State<'_, Arc<DesktopAppState>>,
    input: DesktopTaskInput,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let result: Result<SuccessEnvelope, AppError> = async {
        let service = &state.service;
        match input.action.as_str() {
            "create" => success(
                "task.create",
                service
                    .create_task_from(
                        RequestOrigin::Desktop,
                        CreateTaskInput {
                            project: required(input.project, "project")?,
                            version: input.version,
                            title: required(input.title, "title")?,
                            summary: input.summary,
                            description: input.description,
                            status: parse_optional_enum(input.status)?,
                            priority: parse_optional_enum(input.priority)?,
                            created_by: input.created_by,
                        },
                    )
                    .await?,
                "Created task",
            ),
            "get" => success(
                "task.get",
                service.get_task(&required(input.task, "task")?).await?,
                "Loaded task",
            ),
            "list" => {
                let items = service
                    .list_tasks(TaskQuery {
                        project: input.project,
                        version: input.version,
                        status: parse_optional_enum(input.status)?,
                    })
                    .await?;
                success(
                    "task.list",
                    &items,
                    format!("Listed {} task(s)", items.len()),
                )
            }
            "update" => success(
                "task.update",
                service
                    .update_task_from(
                        RequestOrigin::Desktop,
                        &required(input.task, "task")?,
                        UpdateTaskInput {
                            version: input.version,
                            title: input.title,
                            summary: input.summary,
                            description: input.description,
                            status: parse_optional_enum(input.status)?,
                            priority: parse_optional_enum(input.priority)?,
                            updated_by: input.updated_by,
                        },
                    )
                    .await?,
                "Updated task",
            ),
            "activity_list" => {
                let items = service
                    .list_task_activities(&required(input.task, "task")?)
                    .await?;
                success(
                    "task.activity_list",
                    &items,
                    format!("Listed {} activity item(s)", items.len()),
                )
            }
            other => Err(AppError::InvalidAction(format!(
                "unsupported task action: {other}"
            ))),
        }
    }
    .await;

    result.map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_note(
    state: State<'_, Arc<DesktopAppState>>,
    input: DesktopNoteInput,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let result: Result<SuccessEnvelope, AppError> = async {
        let service = &state.service;
        match input.action.as_str() {
            "create" => success(
                "note.create",
                service
                    .create_note_from(
                        RequestOrigin::Desktop,
                        CreateNoteInput {
                            task: required(input.task, "task")?,
                            content: required(input.content, "content")?,
                            created_by: input.created_by,
                        },
                    )
                    .await?,
                "Created note",
            ),
            "list" => {
                let items = service.list_notes(&required(input.task, "task")?).await?;
                success(
                    "note.list",
                    &items,
                    format!("Listed {} note(s)", items.len()),
                )
            }
            other => Err(AppError::InvalidAction(format!(
                "unsupported note action: {other}"
            ))),
        }
    }
    .await;

    result.map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_attachment(
    state: State<'_, Arc<DesktopAppState>>,
    input: DesktopAttachmentInput,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let result: Result<SuccessEnvelope, AppError> = async {
        let service = &state.service;
        match input.action.as_str() {
            "create" => success(
                "attachment.create",
                service
                    .create_attachment_from(
                        RequestOrigin::Desktop,
                        CreateAttachmentInput {
                            task: required(input.task, "task")?,
                            path: PathBuf::from(required(input.path, "path")?),
                            kind: parse_optional_enum(input.kind)?,
                            created_by: input.created_by,
                            summary: input.summary,
                        },
                    )
                    .await?,
                "Created attachment",
            ),
            "get" => success(
                "attachment.get",
                service
                    .get_attachment(&required(input.attachment_id, "attachment_id")?)
                    .await?,
                "Loaded attachment",
            ),
            "list" => {
                let items = service
                    .list_attachments(&required(input.task, "task")?)
                    .await?;
                success(
                    "attachment.list",
                    &items,
                    format!("Listed {} attachment(s)", items.len()),
                )
            }
            other => Err(AppError::InvalidAction(format!(
                "unsupported attachment action: {other}"
            ))),
        }
    }
    .await;

    result.map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_search(
    state: State<'_, Arc<DesktopAppState>>,
    input: DesktopSearchInput,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let result: Result<SuccessEnvelope, AppError> = async {
        match input.action.as_str() {
            "query" => success(
                "search.query",
                state
                    .service
                    .search(SearchInput {
                        text: search_text(input.text, input.query)?,
                        limit: input.limit,
                    })
                    .await?,
                "Completed search",
            ),
            other => Err(AppError::InvalidAction(format!(
                "unsupported search action: {other}"
            ))),
        }
    }
    .await;

    result.map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_approval(
    state: State<'_, Arc<DesktopAppState>>,
    input: DesktopApprovalInput,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let result: Result<SuccessEnvelope, AppError> = async {
        let service = &state.service;
        match input.action.as_str() {
            "list" => {
                let items = service
                    .list_approval_requests(ApprovalQuery {
                        project: input.project,
                        status: parse_optional_enum(input.status)?,
                    })
                    .await?;
                success(
                    "approval.list",
                    &items,
                    format!("Listed {} approval request(s)", items.len()),
                )
            }
            "get" => success(
                "approval.get",
                service
                    .get_approval_request(&required(input.request_id, "request_id")?)
                    .await?,
                "Loaded approval request",
            ),
            "approve" => {
                let request = service
                    .approve_approval_request(
                        &required(input.request_id, "request_id")?,
                        ReviewApprovalInput {
                            reviewed_by: input.reviewed_by,
                            review_note: input.review_note,
                        },
                    )
                    .await?;
                let summary = if request.status == ApprovalStatus::Approved {
                    "Approved request"
                } else {
                    "Approval replay failed"
                };
                success("approval.approve", request, summary)
            }
            "deny" => success(
                "approval.deny",
                service
                    .deny_approval_request(
                        &required(input.request_id, "request_id")?,
                        ReviewApprovalInput {
                            reviewed_by: input.reviewed_by,
                            review_note: input.review_note,
                        },
                    )
                    .await?,
                "Denied request",
            ),
            other => Err(AppError::InvalidAction(format!(
                "unsupported approval action: {other}"
            ))),
        }
    }
    .await;

    result.map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_mcp_status(
    state: State<'_, Arc<DesktopAppState>>,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    success(
        "desktop.mcp_status",
        state.mcp_supervisor.status_snapshot(),
        "Loaded MCP runtime status",
    )
    .map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_mcp_start(
    state: State<'_, Arc<DesktopAppState>>,
    input: DesktopMcpStartInput,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let save_as_default = input.save_as_default.unwrap_or(false);
    let overrides = input.into_overrides();
    if save_as_default {
        let loaded_config_path =
            state
                .config
                .paths
                .loaded_config_path
                .clone()
                .ok_or_else(|| {
                    error(&AppError::InvalidArguments(
                        "cannot save MCP defaults without a loaded config file".to_string(),
                    ))
                })?;
        let next_defaults = state.mcp_supervisor.resolve_default_config(&overrides);
        save_mcp_config_defaults(&loaded_config_path, &next_defaults)
            .map_err(|app_error| error(&app_error))?;
        state.mcp_supervisor.replace_default_config(next_defaults);
    }

    let status = state
        .mcp_supervisor
        .start(overrides)
        .await
        .map_err(|app_error| error(&app_error))?;
    success(
        "desktop.mcp_start",
        status,
        "Started desktop-managed MCP host",
    )
    .map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_mcp_stop(
    state: State<'_, Arc<DesktopAppState>>,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let status = state
        .mcp_supervisor
        .stop()
        .await
        .map_err(|app_error| error(&app_error))?;
    success(
        "desktop.mcp_stop",
        status,
        "Stopped desktop-managed MCP host",
    )
    .map_err(|app_error| error(&app_error))
}

#[tauri::command]
async fn desktop_mcp_logs_snapshot(
    state: State<'_, Arc<DesktopAppState>>,
    input: DesktopMcpLogsSnapshotInput,
) -> Result<SuccessEnvelope, ErrorEnvelope> {
    let snapshot: McpLogSnapshot = state.mcp_supervisor.logs_snapshot(input.limit);
    success(
        "desktop.mcp_logs_snapshot",
        snapshot,
        "Loaded MCP log snapshot",
    )
    .map_err(|app_error| error(&app_error))
}

pub fn run(runtime: Arc<AppRuntime>) {
    let state = Arc::new(DesktopAppState::new(runtime));
    let state_for_setup = state.clone();
    let state_for_run = state.clone();

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            let status_handle = app.handle().clone();
            let log_handle = app.handle().clone();
            state_for_setup.mcp_supervisor.attach_event_sinks(
                Arc::new(move |status| {
                    let _ = status_handle.emit(MCP_STATUS_EVENT, &status);
                }),
                Arc::new(move |entry| {
                    let _ = log_handle.emit(MCP_LOG_EVENT, &entry);
                }),
            );
            state_for_setup.clone().spawn_mcp_autostart();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_status,
            desktop_mcp_status,
            desktop_mcp_start,
            desktop_mcp_stop,
            desktop_mcp_logs_snapshot,
            desktop_project,
            desktop_version,
            desktop_task,
            desktop_note,
            desktop_attachment,
            desktop_search,
            desktop_approval
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let _ = tauri::async_runtime::block_on(state_for_run.mcp_supervisor.shutdown());
            }
        });
}

fn required(value: Option<String>, field: &str) -> Result<String, AppError> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        _ => Err(AppError::InvalidArguments(format!(
            "missing required field: {field}"
        ))),
    }
}

fn parse_optional_enum<T>(value: Option<String>) -> Result<Option<T>, AppError>
where
    T: FromStr<Err = String>,
{
    value
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|error| AppError::InvalidArguments(error.to_string()))
        })
        .transpose()
}

fn project_reference(project: Option<String>, slug: Option<String>) -> Result<String, AppError> {
    match (project, slug) {
        (Some(project), _) if !project.trim().is_empty() => Ok(project.trim().to_string()),
        (None, Some(slug)) if !slug.trim().is_empty() => Ok(slug.trim().to_string()),
        _ => Err(AppError::InvalidArguments(
            "missing required field: project or slug".to_string(),
        )),
    }
}

fn search_text(text: Option<String>, query: Option<String>) -> Result<String, AppError> {
    match (text, query) {
        (Some(text), _) if !text.trim().is_empty() => Ok(text.trim().to_string()),
        (None, Some(query)) if !query.trim().is_empty() => Ok(query.trim().to_string()),
        _ => Err(AppError::InvalidArguments(
            "missing required field: text or query".to_string(),
        )),
    }
}
