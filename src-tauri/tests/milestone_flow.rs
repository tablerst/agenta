use std::{path::Path, process::Command, sync::Arc, time::Duration};

use agenta_lib::{
    app::{AppRuntime, BootstrapOptions, McpHostKind, McpLaunchOverrides, McpSessionLogger},
    interface::mcp::AgentaMcpServer,
    service::{
        CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
        CreateVersionInput, SearchInput,
    },
};
use assert_cmd::cargo::cargo_bin;
use rmcp::{
    model::CallToolRequestParams,
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig,
        streamable_http_server::{
            session::local::LocalSessionManager, tower::StreamableHttpService,
            StreamableHttpServerConfig,
        },
        StreamableHttpClientTransport,
    },
    ServiceExt,
};
use serde_json::Value;
use tempfile::tempdir;
use tokio::net::TcpListener;

fn write_config(root: &Path, bind: &str) -> std::path::PathBuf {
    let config_path = root.join("agenta.local.yaml");
    let contents = format!(
        "\
paths:\n  data_dir: ./data\n  database_path: ./data/agenta.sqlite3\n  attachments_dir: ./data/attachments\n\
mcp:\n  bind: {bind}\n  path: /mcp\n\
policy:\n  default: auto\n"
    );
    std::fs::write(&config_path, contents).expect("write config");
    config_path
}

#[tokio::test]
async fn service_flow_persists_and_searches() {
    let root = tempdir().expect("tempdir");
    let config_path = write_config(root.path(), "127.0.0.1:0");
    let app = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await
    .expect("bootstrap app");

    let project = app
        .service
        .create_project(CreateProjectInput {
            slug: "demo-service".to_string(),
            name: "Demo Service".to_string(),
            description: Some("Integration project".to_string()),
        })
        .await
        .expect("create project");

    let version = app
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: "v1".to_string(),
            description: Some("First version".to_string()),
            status: None,
        })
        .await
        .expect("create version");

    let task = app
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: None,
            task_kind: None,
            title: "Investigate search indexing".to_string(),
            summary: Some("Make Alpha keyword searchable".to_string()),
            description: Some("Alpha search body".to_string()),
            status: None,
            priority: None,
            created_by: Some("integration".to_string()),
        })
        .await
        .expect("create task");

    let note = app
        .service
        .create_note(CreateNoteInput {
            task: task.task_id.to_string(),
            content: "Alpha note content".to_string(),
            note_kind: None,
            created_by: Some("integration".to_string()),
        })
        .await
        .expect("create note");

    let attachment_source = root.path().join("alpha.log");
    std::fs::write(&attachment_source, "Alpha attachment payload").expect("write attachment");
    let attachment = app
        .service
        .create_attachment(CreateAttachmentInput {
            task: task.task_id.to_string(),
            path: attachment_source,
            kind: None,
            created_by: Some("integration".to_string()),
            summary: Some("Alpha artifact".to_string()),
        })
        .await
        .expect("create attachment");

    let hits = app
        .service
        .search(SearchInput {
            text: Some("Alpha".to_string()),
            project: Some(project.slug.clone()),
            version: None,
            status: None,
            priority: None,
            knowledge_status: None,
            task_kind: None,
            task_code_prefix: None,
            title_prefix: None,
            limit: Some(10),
            all_projects: false,
        })
        .await
        .expect("search");
    let notes = app
        .service
        .list_notes(&task.task_id.to_string())
        .await
        .expect("list notes");
    let loaded_attachment = app
        .service
        .get_attachment(&attachment.attachment_id.to_string())
        .await
        .expect("get attachment");

    let attachments = app
        .service
        .list_attachments(&task.task_id.to_string())
        .await
        .expect("list attachments");
    let activities = app
        .service
        .list_task_activities(&task.task_id.to_string())
        .await
        .expect("list activities");

    assert_eq!(note.task_id, task.task_id);
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].activity_id, note.activity_id);
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachment.attachment_id, attachments[0].attachment_id);
    assert_eq!(loaded_attachment.attachment_id, attachment.attachment_id);
    assert_eq!(activities.len(), 2);
    assert_eq!(hits.query.as_deref(), Some("Alpha"));
    assert!(!hits.tasks.is_empty() || !hits.activities.is_empty());
}

#[test]
fn cli_smoke_returns_json() {
    let root = tempdir().expect("tempdir");
    let config_path = write_config(root.path(), "127.0.0.1:0");
    let cli_bin = cargo_bin("agenta");

    let output = Command::new(cli_bin)
        .args([
            "--config",
            config_path.to_str().expect("config path"),
            "project",
            "create",
            "--slug",
            "demo-cli",
            "--name",
            "Demo CLI",
        ])
        .output()
        .expect("run agenta");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse CLI json output");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["action"], "project.create");
}

#[tokio::test]
async fn mcp_streamable_http_lists_tools_and_calls_project_tool() {
    let root = tempdir().expect("tempdir");
    let config_path = write_config(root.path(), "127.0.0.1:0");
    let runtime = Arc::new(
        AppRuntime::bootstrap(BootstrapOptions {
            config_path: Some(config_path),
        })
        .await
        .expect("bootstrap runtime"),
    );

    let session_manager = Arc::new(LocalSessionManager::default());
    let runtime_for_factory = runtime.clone();
    let logger = McpSessionLogger::new(
        "milestone-mcp-session".to_string(),
        runtime
            .config
            .resolve_mcp_session(McpHostKind::Standalone, &McpLaunchOverrides::default())
            .expect("resolve standalone MCP config"),
        None,
    );
    let logger_for_factory = logger.clone();
    let service: StreamableHttpService<AgentaMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || {
                Ok(AgentaMcpServer::new(
                    runtime_for_factory.service.clone(),
                    logger_for_factory.clone(),
                ))
            },
            session_manager,
            StreamableHttpServerConfig::default().with_sse_keep_alive(None),
        );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move { axum::serve(listener, router).await });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let transport = StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(format!("http://{addr}/mcp")),
    );
    let client = ().serve(transport).await.expect("connect rmcp client");

    let tools = client.list_all_tools().await.expect("list tools");
    assert!(tools.iter().any(|tool| tool.name == "project_create"));
    assert!(tools.iter().any(|tool| tool.name == "project_get"));
    assert!(tools.iter().any(|tool| tool.name == "project_list"));
    assert!(tools.iter().any(|tool| tool.name == "project_update"));
    assert!(tools.iter().any(|tool| tool.name == "version_create"));
    assert!(tools.iter().any(|tool| tool.name == "version_get"));
    assert!(tools.iter().any(|tool| tool.name == "version_list"));
    assert!(tools.iter().any(|tool| tool.name == "version_update"));
    assert!(tools.iter().any(|tool| tool.name == "task_create"));
    assert!(tools.iter().any(|tool| tool.name == "task_get"));
    assert!(tools.iter().any(|tool| tool.name == "task_context_get"));
    assert!(tools.iter().any(|tool| tool.name == "task_list"));
    assert!(tools.iter().any(|tool| tool.name == "task_update"));
    assert!(tools.iter().any(|tool| tool.name == "task_create_child"));
    assert!(tools.iter().any(|tool| tool.name == "task_attach_child"));
    assert!(tools.iter().any(|tool| tool.name == "task_detach_child"));
    assert!(tools.iter().any(|tool| tool.name == "task_add_blocker"));
    assert!(tools.iter().any(|tool| tool.name == "task_resolve_blocker"));
    assert!(tools.iter().any(|tool| tool.name == "note_create"));
    assert!(tools.iter().any(|tool| tool.name == "note_list"));
    assert!(tools.iter().any(|tool| tool.name == "activity_list"));
    assert!(tools.iter().any(|tool| tool.name == "attachment_create"));
    assert!(tools.iter().any(|tool| tool.name == "attachment_get"));
    assert!(tools.iter().any(|tool| tool.name == "attachment_list"));
    assert!(tools.iter().any(|tool| tool.name == "search_query"));
    assert!(!tools.iter().any(|tool| tool.name == "project"));
    assert!(!tools.iter().any(|tool| tool.name == "version"));
    assert!(!tools.iter().any(|tool| tool.name == "task"));
    assert!(!tools.iter().any(|tool| tool.name == "note"));
    assert!(!tools.iter().any(|tool| tool.name == "attachment"));
    assert!(!tools.iter().any(|tool| tool.name == "search"));

    let result = client
        .call_tool(
            CallToolRequestParams::new("project_create").with_arguments(
                serde_json::json!({
                    "slug": "demo-mcp",
                    "name": "Demo MCP"
                })
                .as_object()
                .expect("tool args object")
                .clone(),
            ),
        )
        .await
        .expect("call project tool");

    let payload: Value = serde_json::from_value(
        result
            .structured_content
            .clone()
            .expect("structured MCP response"),
    )
    .expect("deserialize MCP response");
    assert_eq!(payload["project"]["slug"], "demo-mcp");
    assert_eq!(payload["project"]["name"], "Demo MCP");

    let _ = client.cancel().await;
    server.abort();
}
