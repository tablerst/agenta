use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{routing::get, Router};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, tower::StreamableHttpService, StreamableHttpServerConfig,
};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::Emitter;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::app::{
    AppRuntime, McpConfig, McpHostKind, McpLaunchOverrides, McpLogDestination, McpLogLevel,
    ResolvedMcpSessionConfig,
};
use crate::error::{AppError, AppResult};
use crate::interface::mcp::AgentaMcpServer;

pub const MCP_STATUS_EVENT: &str = "desktop://mcp-status";
pub const MCP_LOG_EVENT: &str = "desktop://mcp-log";

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpLifecycleState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct McpLogEntry {
    pub session_id: String,
    pub timestamp: String,
    pub level: McpLogLevel,
    pub component: String,
    pub message: String,
    pub fields: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct McpLogSnapshot {
    pub session_id: Option<String>,
    pub entries: Vec<McpLogEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct McpRuntimeStatus {
    pub state: McpLifecycleState,
    pub session_id: Option<String>,
    pub bind: String,
    pub actual_bind: Option<String>,
    pub path: String,
    pub autostart: bool,
    pub log_level: McpLogLevel,
    pub log_destinations: Vec<McpLogDestination>,
    pub log_file_path: String,
    pub log_ui_buffer_lines: usize,
    pub last_error: Option<String>,
}

type UiLogSink = Arc<dyn Fn(McpLogEntry) + Send + Sync>;

#[derive(Clone)]
pub struct McpSessionLogger {
    session_id: String,
    config: ResolvedMcpSessionConfig,
    ui_sink: Option<UiLogSink>,
}

impl McpSessionLogger {
    pub fn new(
        session_id: String,
        config: ResolvedMcpSessionConfig,
        ui_sink: Option<UiLogSink>,
    ) -> Self {
        Self {
            session_id,
            config,
            ui_sink,
        }
    }

    pub async fn record(
        &self,
        level: McpLogLevel,
        component: impl Into<String>,
        message: impl Into<String>,
        fields: Value,
    ) -> AppResult<()> {
        if !self.config.log.level.allows(level) {
            return Ok(());
        }

        let entry = McpLogEntry {
            session_id: self.session_id.clone(),
            timestamp: now_rfc3339(),
            level,
            component: component.into(),
            message: message.into(),
            fields,
        };

        if self
            .config
            .log
            .destinations
            .contains(&McpLogDestination::Ui)
        {
            if let Some(ui_sink) = &self.ui_sink {
                ui_sink(entry.clone());
            }
        }

        if self
            .config
            .log
            .destinations
            .contains(&McpLogDestination::Stdout)
        {
            let line = serde_json::to_string(&entry).map_err(|error| {
                AppError::internal(format!("failed to encode MCP log entry: {error}"))
            })?;
            println!("{line}");
        }

        if self
            .config
            .log
            .destinations
            .contains(&McpLogDestination::File)
        {
            if let Some(parent) = self.config.log.file_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.config.log.file_path)
                .await?;
            let line = serde_json::to_string(&entry).map_err(|error| {
                AppError::internal(format!("failed to encode MCP log entry: {error}"))
            })?;
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }

        Ok(())
    }

    pub fn info(&self, component: impl Into<String>, message: impl Into<String>, fields: Value) {
        let logger = self.clone();
        let component = component.into();
        let message = message.into();
        tauri::async_runtime::spawn(async move {
            let _ = logger
                .record(McpLogLevel::Info, component, message, fields)
                .await;
        });
    }

    pub fn error(&self, component: impl Into<String>, message: impl Into<String>, fields: Value) {
        let logger = self.clone();
        let component = component.into();
        let message = message.into();
        tauri::async_runtime::spawn(async move {
            let _ = logger
                .record(McpLogLevel::Error, component, message, fields)
                .await;
        });
    }
}

pub struct RunningMcpHost {
    pub actual_bind: String,
    stop_tx: Option<oneshot::Sender<()>>,
    join_handle: JoinHandle<AppResult<()>>,
}

impl RunningMcpHost {
    pub async fn wait(self) -> AppResult<()> {
        self.join_handle
            .await
            .map_err(|error| AppError::internal(format!("failed to join MCP host task: {error}")))?
    }

    pub async fn stop(mut self) -> AppResult<()> {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }

        self.join_handle
            .await
            .map_err(|error| AppError::internal(format!("failed to join MCP host task: {error}")))?
    }
}

pub fn build_mcp_router(
    runtime: Arc<AppRuntime>,
    mount_path: &str,
    logger: McpSessionLogger,
) -> Router {
    let service_for_factory = runtime.service.clone();
    let logger_for_factory = logger.clone();
    let http_service: StreamableHttpService<AgentaMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || {
                Ok(AgentaMcpServer::new(
                    service_for_factory.clone(),
                    logger_for_factory.clone(),
                ))
            },
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default()
                .with_sse_keep_alive(Some(Duration::from_secs(20))),
        );

    Router::new()
        .route(
            "/health",
            get(|| async { axum::Json(json!({ "ok": true })) }),
        )
        .nest_service(mount_path, http_service)
}

pub async fn start_mcp_host(
    runtime: Arc<AppRuntime>,
    config: ResolvedMcpSessionConfig,
    logger: McpSessionLogger,
) -> AppResult<RunningMcpHost> {
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    let actual_bind = listener
        .local_addr()
        .map_err(|error| AppError::Io(error.to_string()))?
        .to_string();
    let router = build_mcp_router(runtime, &config.path, logger);
    let (stop_tx, stop_rx) = oneshot::channel::<()>();

    let join_handle = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = stop_rx.await;
            })
            .await
            .map_err(|error| AppError::Io(error.to_string()))
    });

    Ok(RunningMcpHost {
        actual_bind,
        stop_tx: Some(stop_tx),
        join_handle,
    })
}

pub async fn run_standalone_host(runtime: Arc<AppRuntime>) -> AppResult<()> {
    let resolved = runtime
        .config
        .resolve_mcp_session(McpHostKind::Standalone, &McpLaunchOverrides::default())?;
    let session_id = Uuid::new_v4().to_string();
    let logger = McpSessionLogger::new(session_id, resolved.clone(), None);
    logger
        .record(
            McpLogLevel::Info,
            "mcp_standalone",
            "Starting standalone MCP host",
            json!({
                "bind": resolved.bind,
                "path": resolved.path,
            }),
        )
        .await?;
    let host = start_mcp_host(runtime, resolved.clone(), logger.clone()).await?;
    logger
        .record(
            McpLogLevel::Info,
            "mcp_standalone",
            "Standalone MCP host is running",
            json!({
                "actual_bind": host.actual_bind,
                "path": resolved.path,
            }),
        )
        .await?;
    host.wait().await
}

pub struct McpSupervisor {
    runtime: Arc<AppRuntime>,
    default_config: Arc<Mutex<McpConfig>>,
    inner: Arc<Mutex<SupervisorState>>,
    emitter: Arc<Mutex<Option<tauri::AppHandle>>>,
}

impl McpSupervisor {
    pub fn new(runtime: Arc<AppRuntime>) -> Self {
        Self {
            default_config: Arc::new(Mutex::new(runtime.config.mcp.clone())),
            runtime,
            inner: Arc::new(Mutex::new(SupervisorState::default())),
            emitter: Arc::new(Mutex::new(None)),
        }
    }

    pub fn attach_emitter(&self, app_handle: tauri::AppHandle) {
        *self.emitter.lock().expect("lock MCP supervisor emitter") = Some(app_handle);
        self.emit_status();
    }

    pub fn default_config(&self) -> McpConfig {
        self.default_config
            .lock()
            .expect("lock MCP default config")
            .clone()
    }

    pub fn replace_default_config(&self, next_config: McpConfig) {
        *self.default_config.lock().expect("lock MCP default config") = next_config;
        self.emit_status();
    }

    pub fn resolve_default_config(&self, overrides: &McpLaunchOverrides) -> McpConfig {
        self.default_config().overlay(overrides)
    }

    pub fn status_snapshot(&self) -> McpRuntimeStatus {
        let inner = self.inner.lock().expect("lock MCP supervisor state");
        self.status_snapshot_from_state(&inner)
    }

    pub fn logs_snapshot(&self, limit: Option<usize>) -> McpLogSnapshot {
        let inner = self.inner.lock().expect("lock MCP supervisor state");
        let entries = match limit {
            Some(limit) if inner.logs.len() > limit => inner
                .logs
                .iter()
                .skip(inner.logs.len().saturating_sub(limit))
                .cloned()
                .collect(),
            _ => inner.logs.iter().cloned().collect(),
        };

        McpLogSnapshot {
            session_id: inner
                .session
                .as_ref()
                .map(|session| session.session_id.clone()),
            entries,
        }
    }

    pub async fn start(&self, overrides: McpLaunchOverrides) -> AppResult<McpRuntimeStatus> {
        let effective_config = self.resolve_default_config(&overrides);
        let resolved = effective_config.resolve_for_host(McpHostKind::Desktop);
        let session_id = Uuid::new_v4().to_string();
        let logger = McpSessionLogger::new(
            session_id.clone(),
            resolved.clone(),
            Some(self.make_ui_sink()),
        );

        {
            let mut inner = self.inner.lock().expect("lock MCP supervisor state");
            if matches!(
                inner.state,
                McpLifecycleState::Starting
                    | McpLifecycleState::Running
                    | McpLifecycleState::Stopping
            ) {
                return Err(AppError::Conflict(
                    "desktop-managed MCP host is already active".to_string(),
                ));
            }

            inner.state = McpLifecycleState::Starting;
            inner.last_error = None;
            inner.logs.clear();
            inner.session = Some(SupervisorSession {
                session_id: session_id.clone(),
                config: resolved.clone(),
                actual_bind: None,
            });
        }
        self.emit_status();

        logger
            .record(
                McpLogLevel::Info,
                "mcp_supervisor",
                "Starting desktop-managed MCP host",
                json!({
                    "bind": resolved.bind,
                    "path": resolved.path,
                    "destinations": resolved.log.destinations,
                }),
            )
            .await?;

        match start_mcp_host(self.runtime.clone(), resolved.clone(), logger.clone()).await {
            Ok(host) => {
                let actual_bind = host.actual_bind.clone();
                {
                    let mut inner = self.inner.lock().expect("lock MCP supervisor state");
                    inner.state = McpLifecycleState::Running;
                    if let Some(session) = inner.session.as_mut() {
                        session.actual_bind = Some(actual_bind.clone());
                    }
                    inner.host = Some(host);
                }
                logger
                    .record(
                        McpLogLevel::Info,
                        "mcp_supervisor",
                        "Desktop-managed MCP host is running",
                        json!({
                            "requested_bind": resolved.bind,
                            "actual_bind": actual_bind,
                            "path": resolved.path,
                        }),
                    )
                    .await?;
                self.emit_status();
                Ok(self.status_snapshot())
            }
            Err(error) => {
                let error_message = error.to_string();
                {
                    let mut inner = self.inner.lock().expect("lock MCP supervisor state");
                    inner.state = McpLifecycleState::Failed;
                    inner.last_error = Some(error_message.clone());
                }
                let _ = logger
                    .record(
                        McpLogLevel::Error,
                        "mcp_supervisor",
                        "Failed to start desktop-managed MCP host",
                        json!({
                            "error": error_message,
                        }),
                    )
                    .await;
                self.emit_status();
                Err(error)
            }
        }
    }

    pub async fn stop(&self) -> AppResult<McpRuntimeStatus> {
        let (host, logger) = {
            let mut inner = self.inner.lock().expect("lock MCP supervisor state");
            if !matches!(
                inner.state,
                McpLifecycleState::Starting
                    | McpLifecycleState::Running
                    | McpLifecycleState::Failed
            ) {
                return Ok(self.status_snapshot_from_state(&inner));
            }

            let session = match inner.session.clone() {
                Some(session) => session,
                None => return Ok(self.status_snapshot_from_state(&inner)),
            };

            inner.state = McpLifecycleState::Stopping;
            let logger = McpSessionLogger::new(
                session.session_id.clone(),
                session.config.clone(),
                Some(self.make_ui_sink()),
            );
            let host = inner.host.take();
            (host, logger)
        };
        self.emit_status();

        let _ = logger
            .record(
                McpLogLevel::Info,
                "mcp_supervisor",
                "Stopping desktop-managed MCP host",
                json!({}),
            )
            .await;

        let stop_result = match host {
            Some(host) => host.stop().await,
            None => Ok(()),
        };

        match stop_result {
            Ok(()) => {
                {
                    let mut inner = self.inner.lock().expect("lock MCP supervisor state");
                    inner.state = McpLifecycleState::Stopped;
                    inner.session = None;
                    inner.host = None;
                    inner.last_error = None;
                }
                let _ = logger
                    .record(
                        McpLogLevel::Info,
                        "mcp_supervisor",
                        "Desktop-managed MCP host stopped",
                        json!({}),
                    )
                    .await;
                self.emit_status();
                Ok(self.status_snapshot())
            }
            Err(error) => {
                let error_message = error.to_string();
                {
                    let mut inner = self.inner.lock().expect("lock MCP supervisor state");
                    inner.state = McpLifecycleState::Failed;
                    inner.last_error = Some(error_message.clone());
                    inner.host = None;
                }
                let _ = logger
                    .record(
                        McpLogLevel::Error,
                        "mcp_supervisor",
                        "Desktop-managed MCP host failed while stopping",
                        json!({ "error": error_message }),
                    )
                    .await;
                self.emit_status();
                Err(error)
            }
        }
    }

    pub async fn shutdown(&self) -> AppResult<()> {
        let _ = self.stop().await?;
        Ok(())
    }

    fn make_ui_sink(&self) -> UiLogSink {
        let inner = self.inner.clone();
        let emitter = self.emitter.clone();
        Arc::new(move |entry: McpLogEntry| {
            {
                let mut state = inner.lock().expect("lock MCP supervisor state");
                let capacity = state
                    .session
                    .as_ref()
                    .map(|session| session.config.log.ui_buffer_lines)
                    .unwrap_or(1000);
                while state.logs.len() >= capacity {
                    state.logs.pop_front();
                }
                state.logs.push_back(entry.clone());
            }

            if let Some(app_handle) = emitter.lock().expect("lock MCP emitter").clone() {
                let _ = app_handle.emit(MCP_LOG_EVENT, &entry);
            }
        })
    }

    fn emit_status(&self) {
        let status = self.status_snapshot();
        if let Some(app_handle) = self.emitter.lock().expect("lock MCP emitter").clone() {
            let _ = app_handle.emit(MCP_STATUS_EVENT, &status);
        }
    }

    fn status_snapshot_from_state(&self, state: &SupervisorState) -> McpRuntimeStatus {
        let defaults = self.default_config();
        let session_config = state
            .session
            .as_ref()
            .map(|session| session.config.clone())
            .unwrap_or_else(|| defaults.resolve_for_host(McpHostKind::Desktop));

        McpRuntimeStatus {
            state: state.state,
            session_id: state
                .session
                .as_ref()
                .map(|session| session.session_id.clone()),
            bind: session_config.bind.clone(),
            actual_bind: state
                .session
                .as_ref()
                .and_then(|session| session.actual_bind.clone()),
            path: session_config.path.clone(),
            autostart: session_config.autostart,
            log_level: session_config.log.level,
            log_destinations: session_config.log.destinations.clone(),
            log_file_path: session_config.log.file_path.display().to_string(),
            log_ui_buffer_lines: session_config.log.ui_buffer_lines,
            last_error: state.last_error.clone(),
        }
    }
}

#[derive(Default)]
struct SupervisorState {
    state: McpLifecycleState,
    session: Option<SupervisorSession>,
    last_error: Option<String>,
    logs: VecDeque<McpLogEntry>,
    host: Option<RunningMcpHost>,
}

#[derive(Clone)]
struct SupervisorSession {
    session_id: String,
    config: ResolvedMcpSessionConfig,
    actual_bind: Option<String>,
}

impl Default for McpLifecycleState {
    fn default() -> Self {
        Self::Stopped
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc().unix_timestamp().to_string())
}
