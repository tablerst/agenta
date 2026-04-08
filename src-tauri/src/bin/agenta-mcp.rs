use std::sync::Arc;

use agenta_lib::app::{
    AppRuntime, BootstrapOptions, McpHostKind, McpSessionLogger, init_tracing, start_mcp_host,
};
use clap::Parser;
use serde_json::json;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "agenta-mcp", about = "Agenta MCP streamable HTTP server")]
struct Cli {
    #[arg(long)]
    config: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: cli.config,
        ..BootstrapOptions::default()
    })
    .await
    .map(Arc::new)
    .map_err(|error| std::io::Error::other(error.to_string()))?;

    let config = runtime.config.mcp.resolve_for_host(McpHostKind::Standalone);
    let logger = McpSessionLogger::new(Uuid::new_v4().to_string(), config.clone(), None);
    logger
        .record(
            config.log.level,
            "agenta_mcp",
            "Starting standalone MCP host",
            json!({
                "bind": config.bind,
                "path": config.path,
                "destinations": config.log.destinations,
            }),
        )
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let host = start_mcp_host(runtime, config.clone(), logger.clone())
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let actual_bind = host.actual_bind.clone();
    logger
        .record(
            config.log.level,
            "agenta_mcp",
            "Standalone MCP host is running",
            json!({
                "actual_bind": actual_bind,
                "path": config.path,
            }),
        )
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    host.wait()
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))
}
