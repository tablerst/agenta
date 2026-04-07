use std::sync::Arc;

use assert_cmd::Command;
use axum::body::Body;
use axum::http::{Request, header::CONTENT_TYPE};
use http_body_util::BodyExt;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig,
    session::local::LocalSessionManager,
    tower::StreamableHttpService,
};
use serde_json::{Value, json};
use tempfile::TempDir;

use agenta_lib::{
    app::{AppRuntime, BootstrapOptions},
    interface::mcp::AgentaMcpServer,
    service::{
        CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
        CreateVersionInput, SearchInput,
    },
};

const ACCEPT_BOTH: &str = "application/json, text/event-stream";
const MCP_PROTOCOL_VERSION: &str = "2025-03-26";
#[tokio::test]
async fn runtime_service_flow_covers_core_objects_and_search() -> Result<(), Box<dyn std::error::Error>>
{
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "demo-core".to_string(),
            name: "Demo Core".to_string(),
            description: Some("Core project".to_string()),
        })
        .await?;
    let version = runtime
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: "v1".to_string(),
            description: Some("first milestone".to_string()),
            status: None,
        })
        .await?;
    let task = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            title: "Ship dashboard".to_string(),
            summary: Some("Build the first dashboard".to_string()),
            description: Some("The dashboard ships with CLI, MCP, and search".to_string()),
            status: None,
            priority: None,
            created_by: Some("integration-test".to_string()),
        })
        .await?;
    let note = runtime
        .service
        .create_note(CreateNoteInput {
            task: task.task_id.to_string(),
            content: "Dashboard copy finalized".to_string(),
            created_by: Some("integration-test".to_string()),
        })
        .await?;

    let attachment_source = tempdir.path().join("sample.log");
    std::fs::write(&attachment_source, "dashboard log")?;
    let attachment = runtime
        .service
        .create_attachment(CreateAttachmentInput {
            task: task.task_id.to_string(),
            path: attachment_source,
            kind: None,
            created_by: Some("integration-test".to_string()),
            summary: Some("dashboard-log".to_string()),
        })
        .await?;
    let search = runtime
        .service
        .search(SearchInput {
            text: "dashboard".to_string(),
            limit: Some(10),
        })
        .await?;

    assert_eq!(note.kind.to_string(), "note");
    assert_eq!(attachment.summary, "dashboard-log");
    assert!(!search.tasks.is_empty());
    assert!(!search.activities.is_empty());
    assert!(runtime
        .config
        .paths
        .attachments_dir
        .join(&attachment.storage_path)
        .exists());

    Ok(())
}

#[test]
fn cli_outputs_json_and_reuses_same_database() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir)?;
    let config_path_str = config_path.to_string_lossy().to_string();

    let mut create = Command::cargo_bin("agenta-cli")?;
    let create_output = create
        .args([
            "--config",
            &config_path_str,
            "project",
            "create",
            "--slug",
            "cli-demo",
            "--name",
            "CLI Demo",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let create_json: Value = serde_json::from_slice(&create_output)?;
    assert_eq!(create_json["ok"], true);
    assert_eq!(create_json["action"], "project.create");

    let mut list = Command::cargo_bin("agenta-cli")?;
    let list_output = list
        .args(["--config", &config_path_str, "project", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let list_json: Value = serde_json::from_slice(&list_output)?;
    assert_eq!(list_json["ok"], true);
    assert_eq!(list_json["result"][0]["slug"], "cli-demo");

    Ok(())
}

#[tokio::test]
async fn mcp_streamable_http_tool_call_returns_structured_content(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir)?;
    let runtime = Arc::new(
        AppRuntime::bootstrap(BootstrapOptions {
            config_path: Some(config_path),
        })
        .await?,
    );

    let runtime_for_factory = runtime.clone();
    let service = StreamableHttpService::new(
        move || Ok(AgentaMcpServer::new(runtime_for_factory.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let initialize_response = service
        .handle(
            Request::builder()
                .method("POST")
                .header("Accept", ACCEPT_BOTH)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "initialize",
                        "params": {
                            "protocolVersion": MCP_PROTOCOL_VERSION,
                            "capabilities": {},
                            "clientInfo": {
                                "name": "integration-test",
                                "version": "1.0.0"
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert!(initialize_response.status().is_success());
    let session_id = initialize_response
        .headers()
        .get("mcp-session-id")
        .expect("session id")
        .to_str()?
        .to_string();
    let initialized_response = service
        .handle(
            Request::builder()
                .method("POST")
                .header("Accept", ACCEPT_BOTH)
                .header(CONTENT_TYPE, "application/json")
                .header("mcp-session-id", &session_id)
                .header("mcp-protocol-version", MCP_PROTOCOL_VERSION)
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/initialized"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert_eq!(initialized_response.status().as_u16(), 202);

    let tool_response = service
        .handle(
            Request::builder()
                .method("POST")
                .header("Accept", ACCEPT_BOTH)
                .header(CONTENT_TYPE, "application/json")
                .header("mcp-session-id", &session_id)
                .header("mcp-protocol-version", MCP_PROTOCOL_VERSION)
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "tools/call",
                        "params": {
                            "name": "project",
                            "arguments": {
                                "action": "create",
                                "slug": "mcp-demo",
                                "name": "MCP Demo"
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert!(tool_response.status().is_success());
    let body = tool_response.into_body().collect().await?.to_bytes();
    let body_text = String::from_utf8_lossy(&body).to_string();
    let json_line = body_text
        .lines()
        .find_map(|line| line.strip_prefix("data: ").filter(|value| value.starts_with('{')))
        .ok_or_else(|| format!("missing JSON event payload in SSE body: {body_text}"))?;
    let payload: Value = serde_json::from_str(json_line)
        .map_err(|error| format!("failed to parse tool response JSON payload: {error}; body={body_text}"))?;

    assert_eq!(payload["result"]["structuredContent"]["ok"], true);
    assert_eq!(payload["result"]["structuredContent"]["action"], "project.create");
    assert_eq!(
        payload["result"]["structuredContent"]["result"]["slug"],
        "mcp-demo"
    );

    Ok(())
}

fn write_test_config(tempdir: &TempDir) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config_path = tempdir.path().join("agenta.local.yaml");
    let data_dir = tempdir.path().join("data");
    let yaml = format!(
        "paths:\n  data_dir: {}\nmcp:\n  bind: \"127.0.0.1:8787\"\n  path: \"/mcp\"\n",
        normalize_path_for_yaml(&data_dir)
    );
    std::fs::write(&config_path, yaml)?;
    Ok(config_path)
}

fn normalize_path_for_yaml(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
