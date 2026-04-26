use std::path::PathBuf;
use std::sync::Arc;

use agenta_lib::app::{
    init_tracing, install_panic_hook, record_app_error, record_error_message,
    resolve_error_log_path, start_mcp_host, AppRuntime, BootstrapOptions, McpHostKind,
    McpSessionLogger,
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
    if std::env::args().any(|arg| arg == "--version" || arg == "-V") {
        println!("{}", agenta_lib::build_info::cli_version("agenta-mcp"));
        return Ok(());
    }

    let error_log_path = resolve_error_log_path(config_path_from_args());
    install_panic_hook(error_log_path.clone(), "mcp");
    init_tracing();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = record_error_message(
                &error_log_path,
                "mcp",
                "parser",
                "agenta_mcp.parse",
                "invalid_arguments",
                error.to_string(),
                json!({ "kind": format!("{:?}", error.kind()) }),
            );
            let _ = error.print();
            std::process::exit(error.exit_code());
        }
    };
    let error_log_path = resolve_error_log_path(cli.config.clone());
    let runtime = match AppRuntime::bootstrap(BootstrapOptions {
        config_path: cli.config.clone(),
        ..BootstrapOptions::default()
    })
    .await
    {
        Ok(runtime) => Arc::new(runtime),
        Err(error) => {
            let _ = record_app_error(
                &error_log_path,
                "mcp",
                "bootstrap",
                "agenta_mcp.bootstrap",
                &error,
            );
            return Err(std::io::Error::other(error.to_string()));
        }
    };

    let config = runtime.config.mcp.resolve_for_host(McpHostKind::Standalone);
    let logger = McpSessionLogger::new(Uuid::new_v4().to_string(), config.clone(), None);
    if let Err(error) = logger
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
    {
        let _ = record_app_error(
            &error_log_path,
            "mcp",
            "logger",
            "agenta_mcp.log_starting",
            &error,
        );
        return Err(std::io::Error::other(error.to_string()));
    }

    let host = match start_mcp_host(runtime, config.clone(), logger.clone()).await {
        Ok(host) => host,
        Err(error) => {
            let _ = record_app_error(&error_log_path, "mcp", "host", "agenta_mcp.start", &error);
            return Err(std::io::Error::other(error.to_string()));
        }
    };
    let actual_bind = host.actual_bind.clone();
    if let Err(error) = logger
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
    {
        let _ = record_app_error(
            &error_log_path,
            "mcp",
            "logger",
            "agenta_mcp.log_running",
            &error,
        );
        return Err(std::io::Error::other(error.to_string()));
    }

    match host.wait().await {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = record_app_error(&error_log_path, "mcp", "host", "agenta_mcp.wait", &error);
            Err(std::io::Error::other(error.to_string()))
        }
    }
}

fn config_path_from_args() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg.to_str() == Some("--") {
            break;
        }

        if arg.to_str() == Some("--config") {
            return args.next().map(PathBuf::from);
        }

        if let Some(value) = arg
            .to_str()
            .and_then(|value| value.strip_prefix("--config="))
        {
            return Some(PathBuf::from(value));
        }
    }

    None
}
