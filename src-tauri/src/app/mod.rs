pub mod config;
pub mod mcp;
pub mod runtime;

pub use config::{
    load_runtime_config, save_mcp_config_defaults, AppPaths, McpConfig, McpHostKind,
    McpLaunchOverrides, McpLogConfig, McpLogDestination, McpLogLevel, ResolvedMcpLogConfig,
    ResolvedMcpSessionConfig, RuntimeConfig, SearchConfig, SearchEmbeddingConfig,
    SearchEmbeddingProvider, SearchVectorBackend, SearchVectorConfig, SyncConfig, SyncRemoteConfig,
    SyncRemoteKind, SyncRemotePostgresConfig,
};
pub use mcp::{
    build_mcp_router, start_mcp_host, McpLifecycleState, McpLogEntry, McpLogSnapshot,
    McpRuntimeStatus, McpSessionLogger, McpSupervisor, RunningMcpHost, MCP_LOG_EVENT,
    MCP_STATUS_EVENT,
};
pub use runtime::{init_tracing, AgentaApp as AppRuntime, BootstrapOptions};
