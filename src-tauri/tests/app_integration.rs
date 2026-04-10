use std::net::TcpListener;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::Arc;
use std::time::Duration;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use axum::body::Body;
use axum::http::{header::CONTENT_TYPE, Request};
use http_body_util::BodyExt;
use reqwest::Client;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, tower::StreamableHttpService, StreamableHttpServerConfig,
};
use serde_json::{json, Value};
use tempfile::TempDir;

use agenta_lib::{
    app::{AppRuntime, BootstrapOptions, McpHostKind, McpLaunchOverrides, McpSessionLogger},
    interface::mcp::AgentaMcpServer,
    service::{
        CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
        CreateVersionInput, SearchInput,
    },
};

const ACCEPT_BOTH: &str = "application/json, text/event-stream";
const MCP_PROTOCOL_VERSION: &str = "2025-03-26";
#[tokio::test]
async fn runtime_service_flow_covers_core_objects_and_search(
) -> Result<(), Box<dyn std::error::Error>> {
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
    let notes = runtime
        .service
        .list_notes(&task.task_id.to_string())
        .await?;
    let loaded_attachment = runtime
        .service
        .get_attachment(&attachment.attachment_id.to_string())
        .await?;

    assert_eq!(note.kind.to_string(), "note");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].activity_id, note.activity_id);
    assert_eq!(attachment.summary, "dashboard-log");
    assert_eq!(loaded_attachment.attachment_id, attachment.attachment_id);
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

    let mut create = Command::cargo_bin("agenta")?;
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

    let mut compat = Command::cargo_bin("agenta-cli")?;
    let compat_output = compat
        .args(["--config", &config_path_str, "project", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let compat_json: Value = serde_json::from_slice(&compat_output)?;

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
    assert_eq!(compat_json["result"], list_json["result"]);

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
    let logger = McpSessionLogger::new(
        "integration-mcp-session".to_string(),
        runtime
            .config
            .resolve_mcp_session(McpHostKind::Standalone, &McpLaunchOverrides::default())?,
        None,
    );
    let logger_for_factory = logger.clone();
    let service = StreamableHttpService::new(
        move || {
            Ok(AgentaMcpServer::new(
                runtime_for_factory.service.clone(),
                logger_for_factory.clone(),
            ))
        },
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

    let list_tools_response = service
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
                        "method": "tools/list",
                        "params": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await;
    assert!(list_tools_response.status().is_success());
    let list_tools_body = list_tools_response.into_body().collect().await?.to_bytes();
    let list_tools_body_text = String::from_utf8_lossy(&list_tools_body).to_string();
    let list_tools_json_line = list_tools_body_text
        .lines()
        .find_map(|line| {
            line.strip_prefix("data: ")
                .filter(|value| value.starts_with('{'))
        })
        .ok_or_else(|| format!("missing JSON event payload in SSE body: {list_tools_body_text}"))?;
    let list_tools_payload: Value =
        serde_json::from_str(list_tools_json_line).map_err(|error| {
            format!("failed to parse tools/list JSON payload: {error}; body={list_tools_body_text}")
        })?;

    let tools = list_tools_payload["result"]["tools"]
        .as_array()
        .ok_or("tools/list payload missing tools array")?;
    assert!(tools.iter().any(|tool| tool["name"] == "project_create"));
    assert!(tools.iter().any(|tool| tool["name"] == "project_get"));
    assert!(tools.iter().any(|tool| tool["name"] == "project_list"));
    assert!(tools.iter().any(|tool| tool["name"] == "project_update"));
    assert!(tools.iter().any(|tool| tool["name"] == "version_create"));
    assert!(tools.iter().any(|tool| tool["name"] == "version_get"));
    assert!(tools.iter().any(|tool| tool["name"] == "version_list"));
    assert!(tools.iter().any(|tool| tool["name"] == "version_update"));
    assert!(tools.iter().any(|tool| tool["name"] == "task_create"));
    assert!(tools.iter().any(|tool| tool["name"] == "task_get"));
    assert!(tools.iter().any(|tool| tool["name"] == "task_context_get"));
    assert!(tools.iter().any(|tool| tool["name"] == "task_list"));
    assert!(tools.iter().any(|tool| tool["name"] == "task_update"));
    assert!(tools.iter().any(|tool| tool["name"] == "note_create"));
    assert!(tools.iter().any(|tool| tool["name"] == "note_list"));
    assert!(tools.iter().any(|tool| tool["name"] == "activity_list"));
    assert!(tools.iter().any(|tool| tool["name"] == "attachment_create"));
    assert!(tools.iter().any(|tool| tool["name"] == "attachment_get"));
    assert!(tools.iter().any(|tool| tool["name"] == "attachment_list"));
    assert!(tools.iter().any(|tool| tool["name"] == "search_query"));
    assert!(!tools.iter().any(|tool| tool["name"] == "project"));
    assert!(!tools.iter().any(|tool| tool["name"] == "version"));
    assert!(!tools.iter().any(|tool| tool["name"] == "task"));
    assert!(!tools.iter().any(|tool| tool["name"] == "note"));
    assert!(!tools.iter().any(|tool| tool["name"] == "attachment"));
    assert!(!tools.iter().any(|tool| tool["name"] == "search"));

    let project_create_tool = tools
        .iter()
        .find(|tool| tool["name"] == "project_create")
        .ok_or("missing project_create tool")?;
    assert_eq!(project_create_tool["description"], "Create a new project.");
    assert!(project_create_tool["inputSchema"]["properties"]["action"].is_null());

    let project_update_tool = tools
        .iter()
        .find(|tool| tool["name"] == "project_update")
        .ok_or("missing project_update tool")?;
    let update_input_schema = serde_json::to_string(&project_update_tool["inputSchema"])?;
    assert!(update_input_schema.contains("\"active\""));
    assert!(update_input_schema.contains("\"archived\""));

    let version_create_tool = tools
        .iter()
        .find(|tool| tool["name"] == "version_create")
        .ok_or("missing version_create tool")?;
    let version_input_schema = serde_json::to_string(&version_create_tool["inputSchema"])?;
    assert!(version_input_schema.contains("\"planning\""));
    assert!(version_input_schema.contains("\"active\""));
    assert!(version_input_schema.contains("\"closed\""));
    assert!(version_input_schema.contains("\"archived\""));

    let task_create_tool = tools
        .iter()
        .find(|tool| tool["name"] == "task_create")
        .ok_or("missing task_create tool")?;
    let task_input_schema = serde_json::to_string(&task_create_tool["inputSchema"])?;
    assert!(task_input_schema.contains("\"draft\""));
    assert!(task_input_schema.contains("\"ready\""));
    assert!(task_input_schema.contains("\"in_progress\""));
    assert!(task_input_schema.contains("\"blocked\""));
    assert!(task_input_schema.contains("\"done\""));
    assert!(task_input_schema.contains("\"cancelled\""));
    assert!(task_input_schema.contains("\"low\""));
    assert!(task_input_schema.contains("\"normal\""));
    assert!(task_input_schema.contains("\"high\""));
    assert!(task_input_schema.contains("\"critical\""));
    assert!(task_input_schema.contains("default to `ready`"));
    assert!(task_input_schema.contains("default to `normal`"));

    let task_get_tool = tools
        .iter()
        .find(|tool| tool["name"] == "task_get")
        .ok_or("missing task_get tool")?;
    let task_get_output_schema = serde_json::to_string(&task_get_tool["outputSchema"])?;
    assert!(task_get_output_schema.contains("\"note_count\""));
    assert!(task_get_output_schema.contains("\"attachment_count\""));
    assert!(task_get_output_schema.contains("\"latest_activity_at\""));

    let task_context_get_tool = tools
        .iter()
        .find(|tool| tool["name"] == "task_context_get")
        .ok_or("missing task_context_get tool")?;
    let task_context_output_schema = serde_json::to_string(&task_context_get_tool["outputSchema"])?;
    assert!(task_context_output_schema.contains("\"notes\""));
    assert!(task_context_output_schema.contains("\"attachments\""));
    assert!(task_context_output_schema.contains("\"recent_activities\""));

    let attachment_create_tool = tools
        .iter()
        .find(|tool| tool["name"] == "attachment_create")
        .ok_or("missing attachment_create tool")?;
    let attachment_input_schema = serde_json::to_string(&attachment_create_tool["inputSchema"])?;
    assert!(attachment_input_schema.contains("\"screenshot\""));
    assert!(attachment_input_schema.contains("\"image\""));
    assert!(attachment_input_schema.contains("\"artifact\""));

    let search_query_tool = tools
        .iter()
        .find(|tool| tool["name"] == "search_query")
        .ok_or("missing search_query tool")?;
    assert!(search_query_tool["inputSchema"]["properties"]["action"].is_null());
    let search_output_schema = serde_json::to_string(&search_query_tool["outputSchema"])?;
    assert!(search_output_schema.contains("\"meta\""));

    let project_list_tool = tools
        .iter()
        .find(|tool| tool["name"] == "project_list")
        .ok_or("missing project_list tool")?;
    let project_list_input_schema = serde_json::to_string(&project_list_tool["inputSchema"])?;
    let project_list_output_schema = serde_json::to_string(&project_list_tool["outputSchema"])?;
    assert!(project_list_input_schema.contains("\"limit\""));
    assert!(project_list_input_schema.contains("\"cursor\""));
    assert!(project_list_output_schema.contains("\"page\""));

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
                        "id": 3,
                        "method": "tools/call",
                        "params": {
                            "name": "project_create",
                            "arguments": {
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
        .find_map(|line| {
            line.strip_prefix("data: ")
                .filter(|value| value.starts_with('{'))
        })
        .ok_or_else(|| format!("missing JSON event payload in SSE body: {body_text}"))?;
    let payload: Value = serde_json::from_str(json_line).map_err(|error| {
        format!("failed to parse tool response JSON payload: {error}; body={body_text}")
    })?;

    assert_eq!(
        payload["result"]["structuredContent"]["project"]["slug"],
        "mcp-demo"
    );
    assert_eq!(
        payload["result"]["structuredContent"]["project"]["name"],
        "MCP Demo"
    );

    Ok(())
}

#[tokio::test]
async fn standalone_agenta_mcp_binary_exposes_explicit_tools_and_runs_smoke_flow(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let bind_addr = listener.local_addr()?;
    drop(listener);

    let config_path = write_test_config_with_bind(&tempdir, &bind_addr.to_string())?;
    let stdout_path = tempdir.path().join("agenta-mcp.stdout.log");
    let stderr_path = tempdir.path().join("agenta-mcp.stderr.log");
    let stdout = std::fs::File::create(&stdout_path)?;
    let stderr = std::fs::File::create(&stderr_path)?;

    let child = ProcessCommand::new(cargo_bin("agenta-mcp"))
        .args([
            "--config",
            config_path.to_str().ok_or("invalid config path")?,
        ])
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()?;
    let mut child = ChildGuard { child };

    let client = Client::builder().build()?;
    let url = format!("http://{bind_addr}/mcp");
    let (session_id, _) = initialize_mcp_session(&client, &url).await?;
    post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    )
    .await?;

    let list_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await?;
    let tools = list_payload["result"]["tools"]
        .as_array()
        .ok_or("tools/list payload missing tools array")?;
    for required in [
        "project_create",
        "project_get",
        "project_list",
        "project_update",
        "version_create",
        "version_get",
        "version_list",
        "version_update",
        "task_create",
        "task_get",
        "task_context_get",
        "task_list",
        "task_update",
        "note_create",
        "note_list",
        "activity_list",
        "attachment_create",
        "attachment_get",
        "attachment_list",
        "search_query",
    ] {
        assert!(
            tools.iter().any(|tool| tool["name"] == required),
            "missing required tool {required}"
        );
    }
    for legacy in ["project", "version", "task", "note", "attachment", "search"] {
        assert!(
            !tools.iter().any(|tool| tool["name"] == legacy),
            "legacy tool should not be exposed: {legacy}"
        );
    }

    let project_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "project_create",
                "arguments": {
                    "slug": "binary-mcp-demo",
                    "name": "Binary MCP Demo"
                }
            }
        }),
    )
    .await?;
    assert_eq!(
        project_payload["result"]["structuredContent"]["project"]["slug"],
        "binary-mcp-demo"
    );
    let project_id = project_payload["result"]["structuredContent"]["project"]["project_id"]
        .as_str()
        .ok_or("project_create missing project_id")?
        .to_string();

    let project_get_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "tools/call",
            "params": {
                "name": "project_get",
                "arguments": {
                    "project": project_id.clone()
                }
            }
        }),
    )
    .await?;
    assert_eq!(
        project_get_payload["result"]["structuredContent"]["project"]["slug"],
        "binary-mcp-demo"
    );

    let version_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "version_create",
                "arguments": {
                    "project": "binary-mcp-demo",
                    "name": "Binary Alpha",
                    "status": "planning"
                }
            }
        }),
    )
    .await?;
    assert_eq!(
        version_payload["result"]["structuredContent"]["version"]["status"],
        "planning"
    );

    let task_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "task_create",
                "arguments": {
                    "project": "binary-mcp-demo",
                    "title": "Binary task",
                    "status": "ready",
                    "priority": "high"
                }
            }
        }),
    )
    .await?;
    let task_id = task_payload["result"]["structuredContent"]["task"]["task_id"]
        .as_str()
        .ok_or("task_create missing task_id")?
        .to_string();
    assert_eq!(
        task_payload["result"]["structuredContent"]["task"]["priority"],
        "high"
    );

    let note_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "note_create",
                "arguments": {
                    "task": task_id.clone(),
                    "content": "Binary search marker"
                }
            }
        }),
    )
    .await?;
    assert_eq!(
        note_payload["result"]["structuredContent"]["note"]["content"],
        "Binary search marker"
    );

    let attachment_source = tempdir.path().join("binary-smoke.txt");
    std::fs::write(&attachment_source, "binary attachment payload")?;
    let attachment_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "attachment_create",
                "arguments": {
                    "task": note_payload["result"]["structuredContent"]["note"]["task_id"],
                    "path": normalize_path_for_yaml(&attachment_source),
                    "kind": "artifact",
                    "summary": "Binary artifact"
                }
            }
        }),
    )
    .await?;
    assert_eq!(
        attachment_payload["result"]["structuredContent"]["attachment"]["kind"],
        "artifact"
    );

    let task_update_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 71,
            "method": "tools/call",
            "params": {
                "name": "task_update",
                "arguments": {
                    "task": task_id.clone(),
                    "status": "blocked"
                }
            }
        }),
    )
    .await?;
    assert_eq!(
        task_update_payload["result"]["structuredContent"]["task"]["status"],
        "blocked"
    );
    assert_eq!(
        task_update_payload["result"]["structuredContent"]["task"]["note_count"],
        1
    );
    assert_eq!(
        task_update_payload["result"]["structuredContent"]["task"]["attachment_count"],
        1
    );

    let activity_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 72,
            "method": "tools/call",
            "params": {
                "name": "activity_list",
                "arguments": {
                    "task": task_id.clone(),
                    "limit": 2
                }
            }
        }),
    )
    .await?;
    let activities = activity_payload["result"]["structuredContent"]["activities"]
        .as_array()
        .ok_or("activity_list missing activities")?;
    assert_eq!(activities.len(), 2);
    assert_eq!(activities[0]["kind"], "status_change");
    assert_eq!(activities[1]["kind"], "attachment_ref");
    assert_eq!(
        activity_payload["result"]["structuredContent"]["page"]["has_more"],
        true
    );
    let next_cursor = activity_payload["result"]["structuredContent"]["page"]["next_cursor"]
        .as_str()
        .ok_or("activity_list missing next_cursor")?
        .to_string();

    let activity_page_2 = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 73,
            "method": "tools/call",
            "params": {
                "name": "activity_list",
                "arguments": {
                    "task": task_id.clone(),
                    "limit": 2,
                    "cursor": next_cursor.clone()
                }
            }
        }),
    )
    .await?;
    let more_activities = activity_page_2["result"]["structuredContent"]["activities"]
        .as_array()
        .ok_or("activity_list page 2 missing activities")?;
    assert_eq!(more_activities.len(), 1);
    assert_eq!(more_activities[0]["kind"], "note");

    let task_get_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 74,
            "method": "tools/call",
            "params": {
                "name": "task_get",
                "arguments": {
                    "task": task_id.clone()
                }
            }
        }),
    )
    .await?;
    assert_eq!(
        task_get_payload["result"]["structuredContent"]["task"]["note_count"],
        1
    );
    assert!(task_get_payload["result"]["structuredContent"]["task"]["notes"].is_null());

    let task_context_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 75,
            "method": "tools/call",
            "params": {
                "name": "task_context_get",
                "arguments": {
                    "task": task_id.clone(),
                    "recent_activity_limit": 2
                }
            }
        }),
    )
    .await?;
    assert_eq!(
        task_context_payload["result"]["structuredContent"]["notes"]
            .as_array()
            .ok_or("task_context_get missing notes")?
            .len(),
        1
    );
    assert_eq!(
        task_context_payload["result"]["structuredContent"]["attachments"]
            .as_array()
            .ok_or("task_context_get missing attachments")?
            .len(),
        1
    );
    let recent_activities = task_context_payload["result"]["structuredContent"]
        ["recent_activities"]
        .as_array()
        .ok_or("task_context_get missing recent_activities")?;
    assert_eq!(recent_activities.len(), 2);
    assert_eq!(recent_activities[0]["kind"], "status_change");

    let search_payload = post_jsonrpc(
        &client,
        &url,
        Some(&session_id),
        json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "search_query",
                "arguments": {
                    "query": "Binary",
                    "limit": 1
                }
            }
        }),
    )
    .await?;
    let activities = search_payload["result"]["structuredContent"]["activities"]
        .as_array()
        .ok_or("search_query missing activities")?;
    assert!(!activities.is_empty(), "expected search activity hits");
    let tasks = search_payload["result"]["structuredContent"]["tasks"]
        .as_array()
        .ok_or("search_query missing tasks")?;
    assert!(!tasks.is_empty(), "expected search task hits");
    assert_eq!(
        search_payload["result"]["structuredContent"]["meta"]["limit_applies_per_bucket"],
        true
    );
    assert_eq!(
        search_payload["result"]["structuredContent"]["meta"]["task_limit_applied"],
        1
    );
    assert_eq!(
        search_payload["result"]["structuredContent"]["meta"]["activity_limit_applied"],
        1
    );
    assert_eq!(
        search_payload["result"]["structuredContent"]["meta"]["task_sort"],
        "bm25(tasks_fts) asc"
    );

    child.kill()?;
    Ok(())
}

fn write_test_config(tempdir: &TempDir) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    write_test_config_with_bind(tempdir, "127.0.0.1:8787")
}

fn write_test_config_with_bind(
    tempdir: &TempDir,
    bind: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config_path = tempdir.path().join("agenta.local.yaml");
    let data_dir = tempdir.path().join("data");
    let yaml = format!(
        "paths:\n  data_dir: {}\nmcp:\n  bind: \"{}\"\n  path: \"/mcp\"\n",
        normalize_path_for_yaml(&data_dir),
        bind
    );
    std::fs::write(&config_path, yaml)?;
    Ok(config_path)
}

fn normalize_path_for_yaml(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    fn kill(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
            let _ = self.child.wait();
        }
        Ok(())
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

async fn initialize_mcp_session(
    client: &Client,
    url: &str,
) -> Result<(String, Value), Box<dyn std::error::Error>> {
    let init_payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "binary-integration-test",
                "version": "1.0.0"
            }
        }
    });

    for _ in 0..30 {
        match client
            .post(url)
            .header("Accept", ACCEPT_BOTH)
            .header(CONTENT_TYPE.as_str(), "application/json")
            .json(&init_payload)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let session_id = response
                    .headers()
                    .get("mcp-session-id")
                    .ok_or("initialize response missing mcp-session-id")?
                    .to_str()?
                    .to_string();
                let body_text = response.text().await?;
                let payload = parse_sse_json_payload(&body_text)?;
                return Ok((session_id, payload));
            }
            _ => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    }

    Err("failed to initialize standalone agenta-mcp binary".into())
}

async fn post_jsonrpc(
    client: &Client,
    url: &str,
    session_id: Option<&str>,
    payload: Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut request = client
        .post(url)
        .header("Accept", ACCEPT_BOTH)
        .header(CONTENT_TYPE.as_str(), "application/json");

    if let Some(session_id) = session_id {
        request = request
            .header("mcp-session-id", session_id)
            .header("mcp-protocol-version", MCP_PROTOCOL_VERSION);
    }

    let response = request.json(&payload).send().await?;
    if !response.status().is_success() && response.status().as_u16() != 202 {
        return Err(format!("unexpected MCP response status: {}", response.status()).into());
    }

    let body_text = response.text().await?;
    if body_text.trim().is_empty() {
        return Ok(Value::Null);
    }

    parse_sse_json_payload(&body_text)
}

fn parse_sse_json_payload(body_text: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let json_line = body_text
        .lines()
        .find_map(|line| {
            line.strip_prefix("data: ")
                .filter(|value| value.starts_with('{'))
        })
        .ok_or_else(|| format!("missing JSON event payload in SSE body: {body_text}"))?;
    Ok(serde_json::from_str(json_line)?)
}
