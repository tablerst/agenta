use agenta_lib::{
    app::{AppRuntime, BootstrapOptions, init_tracing},
    interface::mcp::AgentaMcpServer,
};
use axum::{Router, routing::get};
use clap::Parser;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig,
    session::local::LocalSessionManager,
    tower::StreamableHttpService,
};
use std::{sync::Arc, time::Duration};

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

    let bind_address = runtime.config.mcp.bind.clone();
    let mount_path = runtime.config.mcp.path.clone();
    let runtime_for_factory = runtime.clone();

    let http_service: StreamableHttpService<AgentaMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(AgentaMcpServer::new(runtime_for_factory.clone())),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default().with_sse_keep_alive(Some(Duration::from_secs(20))),
        );

    let app = Router::new()
        .route("/health", get(|| async { axum::Json(serde_json::json!({ "ok": true })) }))
        .nest_service(&mount_path, http_service);

    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    axum::serve(listener, app).await
}
