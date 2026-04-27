pub mod config;
pub mod error_log;
pub mod mcp;
pub mod runtime;

pub use config::{
    load_runtime_config, resolve_error_log_path, save_mcp_config_defaults, AppPaths, McpConfig,
    McpHostKind, McpLaunchOverrides, McpLogConfig, McpLogDestination, McpLogLevel,
    ProjectContextConfig, ResolvedMcpLogConfig, ResolvedMcpSessionConfig, RuntimeConfig,
    SearchConfig, SearchEmbeddingConfig, SearchEmbeddingProvider, SearchVectorBackend,
    SearchVectorConfig, SyncConfig, SyncRemoteConfig, SyncRemoteKind, SyncRemotePostgresConfig,
};
pub use error_log::{
    install_panic_hook, record_app_error, record_error_message,
    record_search_index_processing_error,
};
pub use mcp::{
    build_mcp_router, start_mcp_host, McpLifecycleState, McpLogEntry, McpLogSnapshot,
    McpRuntimeStatus, McpSessionLogger, McpSupervisor, RunningMcpHost, MCP_LOG_EVENT,
    MCP_STATUS_EVENT,
};
pub use runtime::{init_tracing, AgentaApp as AppRuntime, BootstrapOptions};
