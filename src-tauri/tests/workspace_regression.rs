use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, Executor, Row, SqliteConnection};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use agenta_lib::{
    app::{AppRuntime, BootstrapOptions},
    domain::{
        KnowledgeStatus, NoteKind, TaskActivityKind, TaskKind, TaskPriority, TaskStatus,
        VersionStatus,
    },
    error::AppError,
    service::{
        ApprovalQuery, ContextInitInput, ContextInitStatus, CreateAttachmentInput, CreateNoteInput,
        CreateProjectInput, CreateTaskInput, CreateVersionInput, PageRequest, SearchInput,
        SortOrder, TaskQuery, TaskSortBy, UpdateProjectInput, UpdateTaskInput, UpdateVersionInput,
    },
};

#[tokio::test]
async fn workspace_flow_persists_updates_filters_and_paginates(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let alpha = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "workspace-alpha".to_string(),
            name: "Workspace Alpha".to_string(),
            description: Some("Primary workspace".to_string()),
        })
        .await?;
    let beta = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "workspace-beta".to_string(),
            name: "Workspace Beta".to_string(),
            description: Some("Secondary workspace".to_string()),
        })
        .await?;

    let alpha_v1 = runtime
        .service
        .create_version(CreateVersionInput {
            project: alpha.slug.clone(),
            name: "Alpha v1".to_string(),
            description: Some("Initial release lane".to_string()),
            status: Some(VersionStatus::Planning),
        })
        .await?;
    let alpha_v2 = runtime
        .service
        .create_version(CreateVersionInput {
            project: alpha.slug.clone(),
            name: "Alpha v2".to_string(),
            description: Some("Follow-up release lane".to_string()),
            status: Some(VersionStatus::Active),
        })
        .await?;
    let beta_v1 = runtime
        .service
        .create_version(CreateVersionInput {
            project: beta.slug.clone(),
            name: "Beta v1".to_string(),
            description: Some("Foreign release lane".to_string()),
            status: Some(VersionStatus::Planning),
        })
        .await?;

    let updated_project = runtime
        .service
        .update_project(
            &alpha.slug,
            UpdateProjectInput {
                name: Some("Workspace Alpha Prime".to_string()),
                description: Some("Updated workspace".to_string()),
                default_version: Some(alpha_v2.version_id.to_string()),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(updated_project.name, "Workspace Alpha Prime");
    assert_eq!(
        updated_project.default_version_id,
        Some(alpha_v2.version_id)
    );

    let foreign_default_error = runtime
        .service
        .update_project(
            &alpha.slug,
            UpdateProjectInput {
                default_version: Some(beta_v1.version_id.to_string()),
                ..Default::default()
            },
        )
        .await
        .expect_err("foreign default version should fail");
    match foreign_default_error {
        AppError::Conflict(message) => {
            assert!(message.contains("default version must belong to the target project"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let updated_version = runtime
        .service
        .update_version(
            &alpha_v1.version_id.to_string(),
            UpdateVersionInput {
                status: Some(VersionStatus::Closed),
                description: Some("Closed after promotion".to_string()),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(updated_version.status, VersionStatus::Closed);
    assert_eq!(
        updated_version.description.as_deref(),
        Some("Closed after promotion")
    );

    let projects_page_1 = runtime
        .service
        .list_projects_page(PageRequest {
            limit: Some(1),
            cursor: None,
        })
        .await?;
    assert_eq!(projects_page_1.items.len(), 1);
    assert!(projects_page_1.has_more);
    let projects_page_2 = runtime
        .service
        .list_projects_page(PageRequest {
            limit: Some(1),
            cursor: projects_page_1.next_cursor,
        })
        .await?;
    assert_eq!(projects_page_2.items.len(), 1);

    let versions_page_1 = runtime
        .service
        .list_versions_page(
            Some(alpha.slug.as_str()),
            PageRequest {
                limit: Some(1),
                cursor: None,
            },
        )
        .await?;
    assert_eq!(versions_page_1.items.len(), 1);
    assert!(versions_page_1.has_more);
    let versions_page_2 = runtime
        .service
        .list_versions_page(
            Some(alpha.slug.as_str()),
            PageRequest {
                limit: Some(1),
                cursor: versions_page_1.next_cursor,
            },
        )
        .await?;
    assert_eq!(versions_page_2.items.len(), 1);

    let task = runtime
        .service
        .create_task(CreateTaskInput {
            project: alpha.slug.clone(),
            version: Some(alpha_v1.version_id.to_string()),
            task_code: None,
            task_kind: None,
            title: "Regression workspace task".to_string(),
            summary: Some("Track workspace regressions".to_string()),
            description: Some("Initial workspace task description".to_string()),
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("workspace-test".to_string()),
        })
        .await?;
    let _secondary_task = runtime
        .service
        .create_task(CreateTaskInput {
            project: alpha.slug.clone(),
            version: Some(alpha_v2.version_id.to_string()),
            task_code: None,
            task_kind: None,
            title: "Secondary task".to_string(),
            summary: Some("Helps verify pagination".to_string()),
            description: None,
            status: Some(TaskStatus::Draft),
            priority: Some(TaskPriority::Low),
            created_by: Some("workspace-test".to_string()),
        })
        .await?;

    let note = runtime
        .service
        .create_note(CreateNoteInput {
            task: task.task_id.to_string(),
            content: "Workspace note payload".to_string(),
            note_kind: None,
            created_by: Some("workspace-test".to_string()),
        })
        .await?;
    let attachment_source = tempdir.path().join("workspace-regression.txt");
    std::fs::write(&attachment_source, "workspace attachment payload")?;
    let attachment = runtime
        .service
        .create_attachment(CreateAttachmentInput {
            task: task.task_id.to_string(),
            path: attachment_source,
            kind: None,
            created_by: Some("workspace-test".to_string()),
            summary: Some("Workspace artifact".to_string()),
        })
        .await?;

    let updated_task = runtime
        .service
        .update_task(
            &task.task_id.to_string(),
            UpdateTaskInput {
                version: Some(alpha_v2.version_id.to_string()),
                summary: Some("Track workspace regressions after promotion".to_string()),
                description: Some("Updated workspace task description".to_string()),
                status: Some(TaskStatus::Done),
                priority: Some(TaskPriority::Critical),
                updated_by: Some("workspace-reviewer".to_string()),
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(updated_task.version_id, Some(alpha_v2.version_id));
    assert_eq!(updated_task.status, TaskStatus::Done);
    assert_eq!(updated_task.priority, TaskPriority::Critical);
    assert_eq!(updated_task.updated_by, "workspace-reviewer");
    assert!(updated_task.closed_at.is_some());

    let task_detail = runtime
        .service
        .get_task_detail(&task.task_id.to_string())
        .await?;
    assert_eq!(task_detail.task.task_id, task.task_id);
    assert_eq!(task_detail.note_count, 1);
    assert_eq!(task_detail.attachment_count, 1);
    assert!(task_detail.latest_activity_at >= note.created_at);

    let context = runtime
        .service
        .get_task_context(&task.task_id.to_string(), Some(2))
        .await?;
    assert_eq!(context.notes.len(), 1);
    assert_eq!(context.attachments.len(), 1);
    assert_eq!(context.recent_activities.len(), 2);
    assert!(context
        .recent_activities
        .iter()
        .any(|activity| activity.kind == TaskActivityKind::StatusChange));

    let activities_page_1 = runtime
        .service
        .list_task_activities_page(
            &task.task_id.to_string(),
            PageRequest {
                limit: Some(2),
                cursor: None,
            },
        )
        .await?;
    assert_eq!(activities_page_1.items.len(), 2);
    assert!(activities_page_1.has_more);
    let activities_page_2 = runtime
        .service
        .list_task_activities_page(
            &task.task_id.to_string(),
            PageRequest {
                limit: Some(2),
                cursor: activities_page_1.next_cursor,
            },
        )
        .await?;
    assert_eq!(activities_page_2.items.len(), 1);

    let filtered_tasks = runtime
        .service
        .list_task_details_page(
            TaskQuery {
                project: Some(alpha.slug.clone()),
                version: Some(alpha_v2.version_id.to_string()),
                status: Some(TaskStatus::Done),
                task_kind: None,
                task_code_prefix: None,
                title_prefix: None,
                sort_by: None,
                sort_order: None,
                all_projects: false,
            },
            PageRequest {
                limit: Some(10),
                cursor: None,
            },
        )
        .await?;
    assert_eq!(filtered_tasks.items.len(), 1);
    assert_eq!(filtered_tasks.items[0].task.task_id, task.task_id);

    let loaded_attachment = runtime
        .service
        .get_attachment(&attachment.attachment_id.to_string())
        .await?;
    assert_eq!(loaded_attachment.summary, "Workspace artifact");
    assert!(runtime
        .config
        .paths
        .attachments_dir
        .join(&loaded_attachment.storage_path)
        .exists());

    Ok(())
}

fn write_test_config(tempdir: &TempDir) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config_path = tempdir.path().join("agenta.local.yaml");
    let data_dir = tempdir.path().join("data");
    let isolated_context_dir = tempdir.path().join("isolated-context");
    let yaml = format!(
        "paths:\n  data_dir: {}\nproject_context:\n  paths:\n    - {}\n  manifest: project.yaml\nmcp:\n  bind: \"127.0.0.1:8787\"\n  path: \"/mcp\"\n",
        normalize_path_for_yaml(&data_dir),
        normalize_path_for_yaml(&isolated_context_dir),
    );
    std::fs::write(&config_path, yaml)?;
    Ok(config_path)
}

fn write_test_config_with_project_context(
    tempdir: &TempDir,
    context_dir: &std::path::Path,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config_path = tempdir.path().join("agenta.local.yaml");
    let data_dir = tempdir.path().join("data");
    let yaml = format!(
        "paths:\n  data_dir: {}\nproject_context:\n  paths:\n    - {}\n  manifest: project.yaml\nmcp:\n  bind: \"127.0.0.1:8787\"\n  path: \"/mcp\"\n",
        normalize_path_for_yaml(&data_dir),
        normalize_path_for_yaml(context_dir),
    );
    std::fs::write(&config_path, yaml)?;
    Ok(config_path)
}

fn normalize_path_for_yaml(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn scoped_search_input(project: &str, version: &str, text: Option<&str>) -> SearchInput {
    SearchInput {
        text: text.map(ToOwned::to_owned),
        project: Some(project.to_string()),
        version: Some(version.to_string()),
        status: None,
        priority: None,
        knowledge_status: None,
        task_kind: None,
        task_code_prefix: None,
        title_prefix: None,
        limit: Some(10),
        all_projects: false,
    }
}

async fn clear_search_index_jobs(runtime: &AppRuntime) -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&runtime.config.paths.database_path)
            .create_if_missing(false)
            .busy_timeout(std::time::Duration::from_secs(5)),
    )
    .await?;
    sqlx::query("DELETE FROM search_index_jobs")
        .execute(&mut connection)
        .await?;
    Ok(())
}

async fn search_index_job_count(
    runtime: &AppRuntime,
    task_id: &str,
) -> Result<i64, Box<dyn std::error::Error>> {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&runtime.config.paths.database_path)
            .create_if_missing(false)
            .busy_timeout(std::time::Duration::from_secs(5)),
    )
    .await?;
    let row = sqlx::query("SELECT COUNT(*) AS count FROM search_index_jobs WHERE task_id = ?")
        .bind(task_id)
        .fetch_one(&mut connection)
        .await?;
    Ok(row.get::<i64, _>("count"))
}

fn write_vector_test_config(
    tempdir: &TempDir,
    endpoint: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config_path = tempdir.path().join("agenta.local.yaml");
    let data_dir = tempdir.path().join("data");
    let yaml = format!(
        "paths:\n  data_dir: {}\nsearch:\n  vector:\n    enabled: true\n    endpoint: {}\n    autostart_sidecar: false\n  embedding:\n    provider: openai_compatible\n    base_url: {}\n    api_key: inline-search-key\n    model: test-embedding\n",
        normalize_path_for_yaml(&data_dir),
        endpoint,
        endpoint,
    );
    std::fs::write(&config_path, yaml)?;
    Ok(config_path)
}

#[derive(Clone, Default)]
struct MockSearchServerState {
    embedding_batch_sizes: Arc<Mutex<Vec<usize>>>,
    upsert_batch_sizes: Arc<Mutex<Vec<usize>>>,
    upsert_documents: Arc<Mutex<Vec<Vec<String>>>>,
}

async fn search_heartbeat() -> StatusCode {
    StatusCode::OK
}

async fn search_collections() -> Json<Value> {
    Json(json!({ "id": "test-collection" }))
}

async fn search_embeddings(
    State(state): State<MockSearchServerState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let inputs = payload["input"].as_array().cloned().unwrap_or_default();
    state.embedding_batch_sizes.lock().await.push(inputs.len());
    Json(json!({
        "data": inputs
            .iter()
            .enumerate()
            .map(|(index, value)| {
                json!({
                    "index": index,
                    "embedding": [index as f32 + 1.0, value.as_str().unwrap_or_default().len() as f32],
                })
            })
            .collect::<Vec<_>>()
    }))
}

async fn search_upsert(
    State(state): State<MockSearchServerState>,
    Path(_collection_id): Path<String>,
    Json(payload): Json<Value>,
) -> StatusCode {
    let ids = payload["ids"].as_array().cloned().unwrap_or_default();
    state.upsert_batch_sizes.lock().await.push(ids.len());
    let documents = payload["documents"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    state.upsert_documents.lock().await.push(documents);
    StatusCode::OK
}

async fn spawn_mock_search_server(
) -> Result<(String, MockSearchServerState, tokio::task::JoinHandle<()>), Box<dyn std::error::Error>>
{
    let state = MockSearchServerState::default();
    let app = Router::new()
        .route("/api/v2/heartbeat", get(search_heartbeat))
        .route(
            "/api/v2/tenants/default_tenant/databases/default_database/collections",
            post(search_collections),
        )
        .route(
            "/api/v2/tenants/default_tenant/databases/default_database/collections/{collection_id}/upsert",
            post(search_upsert),
        )
        .route("/v1/embeddings", post(search_embeddings))
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = format!("http://{}", listener.local_addr()?);
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("search mock server");
    });
    Ok((address, state, server))
}

#[tokio::test]
async fn task_context_retrieval_fields_sort_summary_and_search(
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
            slug: "context-flow".to_string(),
            name: "Context Flow".to_string(),
            description: None,
        })
        .await?;
    let version = runtime
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: "workspace-baseline".to_string(),
            description: None,
            status: None,
        })
        .await?;

    let ctx10 = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("InitCtx-10".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Context index tail".to_string(),
            summary: None,
            description: None,
            status: Some(TaskStatus::Done),
            priority: Some(TaskPriority::Normal),
            created_by: Some("context-test".to_string()),
        })
        .await?;
    let ctx1 = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("InitCtx-1".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Context index head".to_string(),
            summary: None,
            description: None,
            status: Some(TaskStatus::Done),
            priority: Some(TaskPriority::Normal),
            created_by: Some("context-test".to_string()),
        })
        .await?;
    let ctx2 = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("InitCtx-2".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Context reusable note".to_string(),
            summary: None,
            description: None,
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("context-test".to_string()),
        })
        .await?;

    let listed = runtime
        .service
        .list_task_details_page(
            TaskQuery {
                project: Some(project.slug.clone()),
                version: Some(version.version_id.to_string()),
                status: None,
                task_kind: Some(TaskKind::Context),
                task_code_prefix: Some("InitCtx-".to_string()),
                title_prefix: None,
                sort_by: Some(TaskSortBy::TaskCode),
                sort_order: Some(SortOrder::Asc),
                all_projects: false,
            },
            PageRequest {
                limit: None,
                cursor: None,
            },
        )
        .await?;
    let codes = listed
        .items
        .iter()
        .map(|detail| detail.task.task_code.as_deref().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(codes, vec!["InitCtx-1", "InitCtx-2", "InitCtx-10"]);
    assert_eq!(listed.summary.status_counts.done, 2);
    assert_eq!(listed.summary.status_counts.ready, 1);
    assert_eq!(listed.summary.kind_counts.context, 3);

    runtime
        .service
        .create_note(CreateNoteInput {
            task: ctx1.task_id.to_string(),
            content: "Scratch context note with archival-keyword-alpha".to_string(),
            note_kind: Some(NoteKind::Scratch),
            created_by: Some("context-test".to_string()),
        })
        .await?;
    runtime
        .service
        .create_note(CreateNoteInput {
            task: ctx1.task_id.to_string(),
            content: "Latest scratch update without the archival token".to_string(),
            note_kind: Some(NoteKind::Scratch),
            created_by: Some("context-test".to_string()),
        })
        .await?;
    runtime
        .service
        .create_note(CreateNoteInput {
            task: ctx2.task_id.to_string(),
            content: "Reusable conclusion for InitCtx-2".to_string(),
            note_kind: Some(NoteKind::Conclusion),
            created_by: Some("context-test".to_string()),
        })
        .await?;
    let reusable = runtime
        .service
        .get_task_detail(&ctx2.task_id.to_string())
        .await?;
    assert_eq!(reusable.task.knowledge_status, KnowledgeStatus::Reusable);
    assert!(reusable
        .task
        .latest_note_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("Reusable conclusion")));
    let attachment_source = tempdir.path().join("context-evidence.md");
    std::fs::write(
        &attachment_source,
        "# Context attachment\nThis file carries attachment-needle-omega for retrieval.\n",
    )?;
    runtime
        .service
        .create_attachment(CreateAttachmentInput {
            task: ctx10.task_id.to_string(),
            path: attachment_source,
            kind: None,
            created_by: Some("context-test".to_string()),
            summary: Some("context-evidence.md".to_string()),
        })
        .await?;

    let filtered_search = runtime
        .service
        .search(SearchInput {
            text: None,
            project: Some(project.slug.clone()),
            version: Some(version.version_id.to_string()),
            status: None,
            priority: None,
            knowledge_status: None,
            task_kind: Some(TaskKind::Context),
            task_code_prefix: Some("InitCtx-".to_string()),
            title_prefix: None,
            limit: Some(10),
            all_projects: false,
        })
        .await?;
    assert_eq!(filtered_search.query, None);
    assert_eq!(filtered_search.tasks.len(), 3);

    let note_search = runtime
        .service
        .search(SearchInput {
            text: Some("reusable conclusion".to_string()),
            project: Some(project.slug),
            version: Some(version.version_id.to_string()),
            status: None,
            priority: None,
            knowledge_status: None,
            task_kind: None,
            task_code_prefix: None,
            title_prefix: None,
            limit: Some(10),
            all_projects: false,
        })
        .await?;
    assert!(note_search
        .tasks
        .iter()
        .any(|hit| hit.task_id == ctx2.task_id.to_string()));
    assert!(note_search
        .tasks
        .iter()
        .all(|hit| hit.task_id != ctx10.task_id.to_string()));

    let archival_note_search = runtime
        .service
        .search(SearchInput {
            text: Some("archival-keyword-alpha".to_string()),
            project: Some("context-flow".to_string()),
            version: Some(version.version_id.to_string()),
            status: None,
            priority: None,
            knowledge_status: None,
            task_kind: None,
            task_code_prefix: None,
            title_prefix: None,
            limit: Some(10),
            all_projects: false,
        })
        .await?;
    assert!(archival_note_search
        .tasks
        .iter()
        .any(|hit| hit.task_id == ctx1.task_id.to_string()));
    assert!(archival_note_search.tasks.iter().any(|hit| {
        hit.task_id == ctx1.task_id.to_string()
            && hit.evidence_source.as_deref() == Some("activity_search_text")
            && hit
                .evidence_snippet
                .as_deref()
                .is_some_and(|snippet| snippet.contains("archival-keyword-alpha"))
    }));
    assert!(archival_note_search
        .activities
        .iter()
        .any(|hit| hit.task_id == ctx1.task_id.to_string()));

    let attachment_search = runtime
        .service
        .search(SearchInput {
            text: Some("attachment-needle-omega".to_string()),
            project: Some("context-flow".to_string()),
            version: Some(version.version_id.to_string()),
            status: None,
            priority: None,
            knowledge_status: None,
            task_kind: None,
            task_code_prefix: None,
            title_prefix: None,
            limit: Some(10),
            all_projects: false,
        })
        .await?;
    assert!(attachment_search
        .tasks
        .iter()
        .any(|hit| hit.task_id == ctx10.task_id.to_string()));
    assert!(attachment_search
        .activities
        .iter()
        .any(|hit| hit.task_id == ctx10.task_id.to_string()));

    let identifier_search = runtime
        .service
        .search(SearchInput {
            text: Some("InitCtx-1".to_string()),
            project: Some("context-flow".to_string()),
            version: Some(version.version_id.to_string()),
            status: None,
            priority: None,
            knowledge_status: None,
            task_kind: None,
            task_code_prefix: None,
            title_prefix: None,
            limit: Some(10),
            all_projects: false,
        })
        .await?;
    assert_eq!(
        identifier_search
            .tasks
            .first()
            .map(|hit| hit.task_id.clone()),
        Some(ctx1.task_id.to_string())
    );

    let ready_only_search = runtime
        .service
        .search(SearchInput {
            text: None,
            project: Some("context-flow".to_string()),
            version: Some(version.version_id.to_string()),
            status: Some(TaskStatus::Ready),
            priority: None,
            knowledge_status: None,
            task_kind: None,
            task_code_prefix: None,
            title_prefix: None,
            limit: Some(10),
            all_projects: false,
        })
        .await?;
    assert_eq!(ready_only_search.tasks.len(), 1);
    assert_eq!(ready_only_search.tasks[0].task_id, ctx2.task_id.to_string());

    Ok(())
}

#[tokio::test]
async fn search_quality_golden_queries_hold_expected_hits() -> Result<(), Box<dyn std::error::Error>>
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
            slug: "search-quality".to_string(),
            name: "Search Quality".to_string(),
            description: Some("Golden query baseline".to_string()),
        })
        .await?;
    let version = runtime
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: "search-golden".to_string(),
            description: Some("Golden query fixtures".to_string()),
            status: Some(VersionStatus::Active),
        })
        .await?;

    let search_index = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("SearchV2-00".to_string()),
            task_kind: Some(TaskKind::Index),
            title: "Runtime console failure recovery".to_string(),
            summary: Some("Canonical phrase query fixture".to_string()),
            description: None,
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("golden-test".to_string()),
        })
        .await?;
    let search_filters = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("SearchV2-04".to_string()),
            task_kind: Some(TaskKind::Standard),
            title: "Search filter design".to_string(),
            summary: Some("High priority status and knowledge filters".to_string()),
            description: None,
            status: Some(TaskStatus::InProgress),
            priority: Some(TaskPriority::High),
            created_by: Some("golden-test".to_string()),
        })
        .await?;
    let init_ctx = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("InitCtx-01".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Tracing bootstrap flow".to_string(),
            summary: Some("Historical note recall fixture".to_string()),
            description: None,
            status: Some(TaskStatus::Done),
            priority: Some(TaskPriority::Normal),
            created_by: Some("golden-test".to_string()),
        })
        .await?;
    let reusable = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("Reuse-01".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Reusable conclusion lane".to_string(),
            summary: Some("Knowledge status fixture".to_string()),
            description: None,
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("golden-test".to_string()),
        })
        .await?;
    let attachment_task = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("SearchV2-08".to_string()),
            task_kind: Some(TaskKind::Standard),
            title: "Attachment evidence ingest".to_string(),
            summary: Some("Attachment body search fixture".to_string()),
            description: None,
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("golden-test".to_string()),
        })
        .await?;
    let deep_chunk = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("Deep-01".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Deep chunk retrieval lane".to_string(),
            summary: Some("Chunk retrieval fixture".to_string()),
            description: None,
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("golden-test".to_string()),
        })
        .await?;

    runtime
        .service
        .create_note(CreateNoteInput {
            task: init_ctx.task_id.to_string(),
            content: "Historic archival-keyword-alpha note for recall.".to_string(),
            note_kind: Some(NoteKind::Scratch),
            created_by: Some("golden-test".to_string()),
        })
        .await?;
    runtime
        .service
        .create_note(CreateNoteInput {
            task: init_ctx.task_id.to_string(),
            content: "Latest tracing update without archival token.".to_string(),
            note_kind: Some(NoteKind::Scratch),
            created_by: Some("golden-test".to_string()),
        })
        .await?;
    runtime
        .service
        .create_note(CreateNoteInput {
            task: reusable.task_id.to_string(),
            content: "Reusable conclusion for search golden baseline.".to_string(),
            note_kind: Some(NoteKind::Conclusion),
            created_by: Some("golden-test".to_string()),
        })
        .await?;
    let deep_note = format!(
        "{} deep-chunk-needle-zeta {}",
        "prefix ".repeat(1200),
        "suffix ".repeat(1200)
    );
    runtime
        .service
        .create_note(CreateNoteInput {
            task: deep_chunk.task_id.to_string(),
            content: deep_note,
            note_kind: Some(NoteKind::Scratch),
            created_by: Some("golden-test".to_string()),
        })
        .await?;

    let attachment_source = tempdir.path().join("golden-attachment.md");
    std::fs::write(
        &attachment_source,
        "# Attachment body\nThis file contains attachment-needle-omega for search quality.\n",
    )?;
    runtime
        .service
        .create_attachment(CreateAttachmentInput {
            task: attachment_task.task_id.to_string(),
            path: attachment_source,
            kind: None,
            created_by: Some("golden-test".to_string()),
            summary: Some("golden-attachment.md".to_string()),
        })
        .await?;

    let identifier = runtime
        .service
        .search(scoped_search_input(
            &project.slug,
            &version.version_id.to_string(),
            Some("SearchV2-04"),
        ))
        .await?;
    assert_eq!(
        identifier
            .tasks
            .first()
            .and_then(|hit| hit.task_code.as_deref()),
        Some("SearchV2-04")
    );
    assert_eq!(
        identifier
            .tasks
            .first()
            .and_then(|hit| hit.evidence_source.as_deref()),
        Some("task_code")
    );
    assert_eq!(identifier.meta.retrieval_mode, "lexical_only");

    let phrase = runtime
        .service
        .search(scoped_search_input(
            &project.slug,
            &version.version_id.to_string(),
            Some("\"runtime console failure recovery\""),
        ))
        .await?;
    assert_eq!(
        phrase
            .tasks
            .first()
            .and_then(|hit| hit.task_code.as_deref()),
        Some("SearchV2-00")
    );
    assert_eq!(
        phrase
            .tasks
            .first()
            .and_then(|hit| hit.evidence_source.as_deref()),
        Some("title")
    );

    let archival_note = runtime
        .service
        .search(scoped_search_input(
            &project.slug,
            &version.version_id.to_string(),
            Some("archival-keyword-alpha"),
        ))
        .await?;
    assert!(archival_note
        .tasks
        .iter()
        .any(|hit| hit.task_code.as_deref() == Some("InitCtx-01")));
    assert!(archival_note.tasks.iter().any(|hit| {
        hit.task_code.as_deref() == Some("InitCtx-01")
            && hit.evidence_source.as_deref() == Some("activity_search_text")
            && hit
                .evidence_snippet
                .as_deref()
                .is_some_and(|snippet| snippet.contains("archival-keyword-alpha"))
    }));

    let attachment_body = runtime
        .service
        .search(scoped_search_input(
            &project.slug,
            &version.version_id.to_string(),
            Some("attachment-needle-omega"),
        ))
        .await?;
    assert!(attachment_body
        .tasks
        .iter()
        .any(|hit| hit.task_code.as_deref() == Some("SearchV2-08")));
    assert!(attachment_body.tasks.iter().any(|hit| {
        hit.task_code.as_deref() == Some("SearchV2-08")
            && hit.evidence_source.as_deref() == Some("activity_search_text")
    }));

    let deep_chunk_search = runtime
        .service
        .search(scoped_search_input(
            &project.slug,
            &version.version_id.to_string(),
            Some("deep-chunk-needle-zeta"),
        ))
        .await?;
    assert!(deep_chunk_search
        .tasks
        .iter()
        .any(|hit| hit.task_code.as_deref() == Some("Deep-01")));
    assert!(deep_chunk_search.tasks.iter().any(|hit| {
        hit.task_code.as_deref() == Some("Deep-01")
            && hit.evidence_source.as_deref() == Some("activity_search_text")
            && hit
                .evidence_snippet
                .as_deref()
                .is_some_and(|snippet| snippet.contains("deep-chunk-needle-zeta"))
    }));

    let ready_only = runtime
        .service
        .search(SearchInput {
            status: Some(TaskStatus::Ready),
            ..scoped_search_input(&project.slug, &version.version_id.to_string(), None)
        })
        .await?;
    assert!(ready_only
        .tasks
        .iter()
        .all(|hit| hit.status == TaskStatus::Ready.to_string()));
    assert_eq!(ready_only.meta.retrieval_mode, "structured_only");

    let reusable_only = runtime
        .service
        .search(SearchInput {
            knowledge_status: Some(KnowledgeStatus::Reusable),
            ..scoped_search_input(&project.slug, &version.version_id.to_string(), None)
        })
        .await?;
    assert_eq!(reusable_only.tasks.len(), 1);
    assert_eq!(
        reusable_only.tasks[0].task_code.as_deref(),
        Some("Reuse-01")
    );

    let high_priority = runtime
        .service
        .search(SearchInput {
            priority: Some(TaskPriority::High),
            ..scoped_search_input(&project.slug, &version.version_id.to_string(), None)
        })
        .await?;
    assert_eq!(high_priority.tasks.len(), 1);
    assert_eq!(
        high_priority.tasks[0].task_code.as_deref(),
        Some("SearchV2-04")
    );

    let _ = search_index;
    let _ = search_filters;

    Ok(())
}

#[tokio::test]
async fn task_search_reindex_jobs_persist_when_vector_runtime_is_unavailable(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = tempdir.path().join("agenta.local.yaml");
    let data_dir = tempdir.path().join("data");
    std::env::set_var("AGENTA_TEST_SEARCH_EMBEDDING_KEY", "test-key");
    std::fs::write(
        &config_path,
        format!(
            "paths:\n  data_dir: {}\nsearch:\n  vector:\n    enabled: true\n    endpoint: http://127.0.0.1:65535\n    autostart_sidecar: false\n  embedding:\n    provider: openai_compatible\n    base_url: http://127.0.0.1:65535\n    api_key_env: AGENTA_TEST_SEARCH_EMBEDDING_KEY\n    model: test-embedding\n",
            normalize_path_for_yaml(&data_dir),
        ),
    )?;

    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "search-index-jobs".to_string(),
            name: "Search Index Jobs".to_string(),
            description: None,
        })
        .await?;
    let version = runtime
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: "v1".to_string(),
            description: None,
            status: None,
        })
        .await?;
    let task = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug,
            version: Some(version.version_id.to_string()),
            task_code: Some("InitCtx-Search".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Vector job source".to_string(),
            summary: Some("Queue a vector job".to_string()),
            description: Some("This should enqueue a durable search index job.".to_string()),
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("search-test".to_string()),
        })
        .await?;
    runtime
        .service
        .create_note(CreateNoteInput {
            task: task.task_id.to_string(),
            content: "Conclusion note that refreshes the task digest".to_string(),
            note_kind: Some(NoteKind::Conclusion),
            created_by: Some("search-test".to_string()),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(250)).await;

    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&runtime.config.paths.database_path)
            .create_if_missing(false)
            .busy_timeout(std::time::Duration::from_secs(5)),
    )
    .await?;
    let row = sqlx::query("SELECT COUNT(*) AS count FROM search_index_jobs WHERE task_id = ?")
        .bind(task.task_id.to_string())
        .fetch_one(&mut connection)
        .await?;
    assert!(row.get::<i64, _>("count") >= 1);
    let search_status = runtime.service.search_index_status().await?;
    assert!(search_status.total_count >= 1);
    assert!(search_status.last_error.is_some() || search_status.processing_count >= 1);

    Ok(())
}

#[tokio::test]
async fn search_backfill_batches_embeddings_and_upserts_task_documents(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let disabled_config = write_test_config(&tempdir)?;
    let disabled_runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(disabled_config),
    })
    .await?;

    let project = disabled_runtime
        .service
        .create_project(CreateProjectInput {
            slug: "search-batch".to_string(),
            name: "Search Batch Project".to_string(),
            description: Some("Project context for vector backfill".to_string()),
        })
        .await?;
    let version = disabled_runtime
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: "Search Batch v1".to_string(),
            description: Some("Version context for vector backfill".to_string()),
            status: Some(VersionStatus::Active),
        })
        .await?;
    let task_a = disabled_runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("InitCtx-Batch-A".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Batch vector source".to_string(),
            summary: Some("Backfill should batch embeddings".to_string()),
            description: Some("History should become searchable through chroma.".to_string()),
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::High),
            created_by: Some("search-batch".to_string()),
        })
        .await?;
    let task_b = disabled_runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug,
            version: Some(version.version_id.to_string()),
            task_code: Some("InitCtx-Batch-B".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Attachment vector source".to_string(),
            summary: Some("Attachment summaries should enter the vector document".to_string()),
            description: Some(
                "Batch processing should still include related attachment context.".to_string(),
            ),
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("search-batch".to_string()),
        })
        .await?;
    disabled_runtime
        .service
        .create_note(CreateNoteInput {
            task: task_a.task_id.to_string(),
            content: "Backfill conclusion note".to_string(),
            note_kind: Some(NoteKind::Conclusion),
            created_by: Some("search-batch".to_string()),
        })
        .await?;
    let attachment_source = tempdir.path().join("search-batch-attachment.md");
    std::fs::write(&attachment_source, "# Architecture")?;
    disabled_runtime
        .service
        .create_attachment(CreateAttachmentInput {
            task: task_b.task_id.to_string(),
            path: attachment_source,
            kind: None,
            created_by: Some("search-batch".to_string()),
            summary: Some("Architecture Overview".to_string()),
        })
        .await?;
    drop(disabled_runtime);

    let (endpoint, state, server) = spawn_mock_search_server().await?;
    let enabled_config = write_vector_test_config(&tempdir, &endpoint)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(enabled_config),
    })
    .await?;

    let summary = runtime.service.search_backfill(Some(10), Some(1)).await?;
    assert_eq!(summary.scanned, 2);
    assert_eq!(summary.queued, 2);
    assert_eq!(summary.skipped, 0);
    assert_eq!(summary.status, "completed");
    assert_eq!(summary.processed, 2);
    assert_eq!(summary.succeeded, 2);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.pending_after, 0);
    assert!(summary.processing_error.is_none());

    let search_status = runtime.service.search_index_status().await?;
    assert_eq!(search_status.pending_count, 0);
    assert_eq!(search_status.processing_count, 0);
    assert_eq!(search_status.failed_count, 0);
    assert_eq!(
        search_status.latest_run.as_ref().map(|run| (
            run.run_id,
            run.status.as_str(),
            run.succeeded
        )),
        Some((summary.run_id, "completed", 2))
    );

    assert_eq!(*state.embedding_batch_sizes.lock().await, vec![1, 1]);
    assert_eq!(*state.upsert_batch_sizes.lock().await, vec![1, 1]);
    let upsert_documents = state.upsert_documents.lock().await.clone();
    let flattened = upsert_documents.into_iter().flatten().collect::<Vec<_>>();
    assert!(flattened
        .iter()
        .any(|document| document.contains("project search-batch | Search Batch Project")));
    assert!(flattened
        .iter()
        .any(|document| document.contains("version Search Batch v1")));
    assert!(flattened
        .iter()
        .any(|document| document.contains("attachment Architecture Overview")));

    server.abort();
    Ok(())
}

#[tokio::test]
async fn retry_failed_search_jobs_requeues_them_into_a_new_run(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let disabled_config = write_test_config(&tempdir)?;
    let disabled_runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(disabled_config),
    })
    .await?;

    let project = disabled_runtime
        .service
        .create_project(CreateProjectInput {
            slug: "search-retry".to_string(),
            name: "Search Retry".to_string(),
            description: Some("Retry failed jobs".to_string()),
        })
        .await?;
    let task = disabled_runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug,
            version: None,
            task_code: Some("InitCtx-Retry".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Retry failed vector job".to_string(),
            summary: Some("Should be retried as a new run".to_string()),
            description: Some("Search recovery flow should requeue failed jobs.".to_string()),
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("search-retry".to_string()),
        })
        .await?;
    drop(disabled_runtime);

    let (endpoint, _state, server) = spawn_mock_search_server().await?;
    let enabled_config = write_vector_test_config(&tempdir, &endpoint)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(enabled_config),
    })
    .await?;

    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&runtime.config.paths.database_path)
            .create_if_missing(false)
            .busy_timeout(std::time::Duration::from_secs(5)),
    )
    .await?;
    sqlx::query(
        r#"
        INSERT INTO search_index_jobs (
            task_id, job_kind, status, attempt_count, last_error, next_attempt_at,
            run_id, locked_at, lease_until, created_at, updated_at
        ) VALUES (?, 'task_vector_upsert', 'failed', 3, 'previous vector error',
            '2026-01-01T00:00:00Z', NULL, NULL, NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')
        "#,
    )
    .bind(task.task_id.to_string())
    .execute(&mut connection)
    .await?;

    let summary = runtime
        .service
        .retry_failed_search_index_jobs(Some(10), Some(1))
        .await?;
    assert_eq!(summary.status, "completed");
    assert_eq!(summary.trigger_kind, "retry_failed");
    assert_eq!(summary.queued, 1);
    assert_eq!(summary.succeeded, 1);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.pending_after, 0);
    assert!(summary.processing_error.is_none());

    let status = runtime.service.search_index_status().await?;
    assert_eq!(status.failed_count, 0);
    assert_eq!(
        status.latest_run.as_ref().map(|run| (
            run.trigger_kind.as_str(),
            run.succeeded,
            run.remaining_count
        )),
        Some(("retry_failed", 1, 0))
    );

    server.abort();
    Ok(())
}

#[tokio::test]
async fn recover_stale_search_jobs_requeues_expired_processing_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let disabled_config = write_test_config(&tempdir)?;
    let disabled_runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(disabled_config),
    })
    .await?;

    let project = disabled_runtime
        .service
        .create_project(CreateProjectInput {
            slug: "search-stale".to_string(),
            name: "Search Stale".to_string(),
            description: Some("Recover stale jobs".to_string()),
        })
        .await?;
    let task = disabled_runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug,
            version: None,
            task_code: Some("InitCtx-Stale".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Recover stale vector job".to_string(),
            summary: Some("Expired processing job should be recovered".to_string()),
            description: Some(
                "Search recovery flow should reclaim stale processing jobs.".to_string(),
            ),
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("search-stale".to_string()),
        })
        .await?;
    drop(disabled_runtime);

    let (endpoint, _state, server) = spawn_mock_search_server().await?;
    let enabled_config = write_vector_test_config(&tempdir, &endpoint)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(enabled_config),
    })
    .await?;

    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&runtime.config.paths.database_path)
            .create_if_missing(false)
            .busy_timeout(std::time::Duration::from_secs(5)),
    )
    .await?;
    sqlx::query(
        r#"
        INSERT INTO search_index_jobs (
            task_id, job_kind, status, attempt_count, last_error, next_attempt_at,
            run_id, locked_at, lease_until, created_at, updated_at
        ) VALUES (?, 'task_vector_upsert', 'processing', 1, NULL,
            NULL, NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z',
            '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')
        "#,
    )
    .bind(task.task_id.to_string())
    .execute(&mut connection)
    .await?;

    let summary = runtime
        .service
        .recover_stale_search_index_jobs(Some(10), Some(1))
        .await?;
    assert_eq!(summary.status, "completed");
    assert_eq!(summary.trigger_kind, "recover_stale");
    assert_eq!(summary.queued, 1);
    assert_eq!(summary.succeeded, 1);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.pending_after, 0);
    assert!(summary.processing_error.is_none());

    let status = runtime.service.search_index_status().await?;
    assert_eq!(status.stale_processing_count, 0);
    assert_eq!(
        status.latest_run.as_ref().map(|run| (
            run.trigger_kind.as_str(),
            run.succeeded,
            run.remaining_count
        )),
        Some(("recover_stale", 1, 0))
    );

    server.abort();
    Ok(())
}

#[tokio::test]
async fn project_version_and_attachment_changes_requeue_task_vector_jobs(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = tempdir.path().join("agenta.local.yaml");
    let data_dir = tempdir.path().join("data");
    std::fs::write(
        &config_path,
        format!(
            "paths:\n  data_dir: {}\nsearch:\n  vector:\n    enabled: true\n    endpoint: http://127.0.0.1:65535\n    autostart_sidecar: false\n  embedding:\n    provider: openai_compatible\n    base_url: http://127.0.0.1:65535\n    api_key: inline-search-key\n    model: test-embedding\n",
            normalize_path_for_yaml(&data_dir),
        ),
    )?;

    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "search-requeue".to_string(),
            name: "Search Requeue".to_string(),
            description: Some("Project before requeue".to_string()),
        })
        .await?;
    let version = runtime
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: "Search Requeue v1".to_string(),
            description: Some("Version before requeue".to_string()),
            status: Some(VersionStatus::Planning),
        })
        .await?;
    let task = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            task_code: Some("InitCtx-Requeue".to_string()),
            task_kind: Some(TaskKind::Context),
            title: "Requeue source".to_string(),
            summary: Some("A task that should be requeued".to_string()),
            description: Some(
                "Project, version, and attachment changes should requeue this task.".to_string(),
            ),
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("search-requeue".to_string()),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    clear_search_index_jobs(&runtime).await?;

    runtime
        .service
        .update_project(
            &project.slug,
            UpdateProjectInput {
                name: Some("Search Requeue Updated".to_string()),
                description: Some("Project update should requeue".to_string()),
                ..Default::default()
            },
        )
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    let count_after_project = search_index_job_count(&runtime, &task.task_id.to_string()).await?;
    assert_eq!(count_after_project, 1);
    clear_search_index_jobs(&runtime).await?;

    runtime
        .service
        .update_version(
            &version.version_id.to_string(),
            UpdateVersionInput {
                name: Some("Search Requeue v2".to_string()),
                description: Some("Version update should requeue".to_string()),
                status: Some(VersionStatus::Active),
            },
        )
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    let count_after_version = search_index_job_count(&runtime, &task.task_id.to_string()).await?;
    assert_eq!(count_after_version, 1);
    clear_search_index_jobs(&runtime).await?;

    let attachment_source = tempdir.path().join("search-requeue-attachment.txt");
    std::fs::write(&attachment_source, "attachment payload")?;
    runtime
        .service
        .create_attachment(CreateAttachmentInput {
            task: task.task_id.to_string(),
            path: attachment_source,
            kind: None,
            created_by: Some("search-requeue".to_string()),
            summary: Some("Requeue Attachment".to_string()),
        })
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    let count_after_attachment =
        search_index_job_count(&runtime, &task.task_id.to_string()).await?;
    assert_eq!(count_after_attachment, 1);

    Ok(())
}

#[tokio::test]
async fn concurrent_writes_share_the_same_runtime_write_queue(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir)?;
    let runtime = Arc::new(
        AppRuntime::bootstrap(BootstrapOptions {
            config_path: Some(config_path),
        })
        .await?,
    );

    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "queue-runtime".to_string(),
            name: "Queue Runtime".to_string(),
            description: Some("Concurrent write regression".to_string()),
        })
        .await?;
    let version = runtime
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: "Queue Runtime v1".to_string(),
            description: Some("Concurrent lane".to_string()),
            status: Some(VersionStatus::Active),
        })
        .await?;

    let mut create_set = JoinSet::new();
    for index in 0..8 {
        let service = runtime.service.clone();
        let project_slug = project.slug.clone();
        let version_id = version.version_id.to_string();
        create_set.spawn(async move {
            service
                .create_task(CreateTaskInput {
                    project: project_slug,
                    version: Some(version_id),
                    task_code: None,
                    task_kind: None,
                    title: format!("Concurrent task {index}"),
                    summary: Some(format!("Concurrent summary {index}")),
                    description: Some(format!("Concurrent description {index}")),
                    status: Some(TaskStatus::Ready),
                    priority: Some(TaskPriority::Normal),
                    created_by: Some("queue-test".to_string()),
                })
                .await
        });
    }

    let mut task_ids = Vec::new();
    while let Some(result) = create_set.join_next().await {
        let task = result??;
        task_ids.push(task.task_id.to_string());
    }
    assert_eq!(task_ids.len(), 8);

    let mut mutation_set = JoinSet::new();
    for (index, task_id) in task_ids.iter().enumerate() {
        let note_service = runtime.service.clone();
        let note_task_id = task_id.clone();
        mutation_set.spawn(async move {
            note_service
                .create_note(CreateNoteInput {
                    task: note_task_id,
                    content: format!("Concurrent note {index}"),
                    note_kind: None,
                    created_by: Some("queue-test".to_string()),
                })
                .await
                .map(|_| None)
        });

        let update_service = runtime.service.clone();
        let update_task_id = task_id.clone();
        mutation_set.spawn(async move {
            update_service
                .update_task(
                    &update_task_id,
                    UpdateTaskInput {
                        summary: Some(format!("Updated summary {index}")),
                        description: Some(format!("Updated description {index}")),
                        status: Some(TaskStatus::InProgress),
                        priority: Some(TaskPriority::High),
                        updated_by: Some("queue-reviewer".to_string()),
                        ..Default::default()
                    },
                )
                .await
                .map(|_| None)
        });
    }

    let mut expected_attachment_paths = Vec::new();
    for index in 0..4 {
        let source = tempdir.path().join(format!("queue-attachment-{index}.txt"));
        std::fs::write(&source, format!("attachment payload {index}"))?;
        let service = runtime.service.clone();
        let task_id = task_ids[index].clone();
        mutation_set.spawn(async move {
            service
                .create_attachment(CreateAttachmentInput {
                    task: task_id,
                    path: source,
                    kind: None,
                    created_by: Some("queue-test".to_string()),
                    summary: Some(format!("Concurrent attachment {index}")),
                })
                .await
                .map(|attachment| Some(attachment.storage_path))
        });
    }

    while let Some(result) = mutation_set.join_next().await {
        match result?? {
            Some(storage_path) => expected_attachment_paths.push(storage_path),
            None => {}
        }
    }

    let sample_context = runtime
        .service
        .get_task_context(&task_ids[0], Some(10))
        .await?;
    assert_eq!(sample_context.notes.len(), 1);
    assert_eq!(sample_context.attachments.len(), 1);
    assert!(sample_context
        .recent_activities
        .iter()
        .any(|activity| activity.kind == TaskActivityKind::StatusChange));

    for task_id in task_ids.iter().take(4) {
        let attachments = runtime.service.list_attachments(task_id).await?;
        assert_eq!(attachments.len(), 1);
    }

    for storage_path in expected_attachment_paths {
        assert!(runtime
            .config
            .paths
            .attachments_dir
            .join(storage_path)
            .exists());
    }

    Ok(())
}

#[tokio::test]
async fn cross_connection_write_lock_returns_storage_busy() -> Result<(), Box<dyn std::error::Error>>
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
            slug: "busy-project".to_string(),
            name: "Busy Project".to_string(),
            description: Some("Storage busy regression".to_string()),
        })
        .await?;

    let mut lock_holder = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(&runtime.config.paths.database_path),
    )
    .await?;
    lock_holder.execute("BEGIN IMMEDIATE").await?;

    let result = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug,
            version: None,
            task_code: None,
            task_kind: None,
            title: "Blocked by external write lock".to_string(),
            summary: Some("Should surface storage_busy".to_string()),
            description: None,
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("busy-test".to_string()),
        })
        .await;

    lock_holder.execute("ROLLBACK").await?;

    match result {
        Err(AppError::StorageBusy(message)) => {
            let normalized = message.to_ascii_lowercase();
            assert!(
                normalized.contains("locked") || normalized.contains("busy"),
                "unexpected storage busy message: {message}"
            );
        }
        Err(other) => panic!("expected storage busy error, got {other:?}"),
        Ok(task) => panic!("expected storage busy error, got task {}", task.task_id),
    }

    Ok(())
}

#[tokio::test]
async fn multiple_projects_without_scope_return_ambiguous_context(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let alpha = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "scope-alpha".to_string(),
            name: "Scope Alpha".to_string(),
            description: None,
        })
        .await?;
    let beta = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "scope-beta".to_string(),
            name: "Scope Beta".to_string(),
            description: None,
        })
        .await?;

    for project in [&alpha, &beta] {
        runtime
            .service
            .create_task(CreateTaskInput {
                project: project.slug.clone(),
                version: None,
                task_code: None,
                task_kind: None,
                title: format!("{} task", project.name),
                summary: Some("ambiguous scope".to_string()),
                description: None,
                status: Some(TaskStatus::Ready),
                priority: Some(TaskPriority::Normal),
                created_by: Some("scope-test".to_string()),
            })
            .await?;
    }

    let task_error = runtime
        .service
        .list_tasks(TaskQuery::default())
        .await
        .expect_err("task query should require an explicit scope");
    assert!(matches!(task_error, AppError::AmbiguousContext(_)));

    let search_error = runtime
        .service
        .search(SearchInput {
            text: Some("task".to_string()),
            project: None,
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
        .expect_err("search should require an explicit scope");
    assert!(matches!(search_error, AppError::AmbiguousContext(_)));

    let approval_error = runtime
        .service
        .list_approval_requests(ApprovalQuery::default())
        .await
        .expect_err("approval list should require an explicit scope");
    assert!(matches!(approval_error, AppError::AmbiguousContext(_)));

    Ok(())
}

#[tokio::test]
async fn context_init_creates_and_updates_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let context_dir = tempdir.path().join("workspace").join("custom-context");
    let config_path = write_test_config_with_project_context(&tempdir, &context_dir)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "context-init-demo".to_string(),
            name: "Context Init Demo".to_string(),
            description: None,
        })
        .await?;

    let created = runtime
        .service
        .init_project_context(ContextInitInput {
            project: None,
            workspace_root: Some(tempdir.path().join("workspace")),
            context_dir: None,
            instructions: None,
            memory_dir: None,
            force: false,
            dry_run: false,
        })
        .await?;
    assert_eq!(created.project, project.slug);
    assert_eq!(created.status, ContextInitStatus::Created);
    assert!(created.manifest_path.exists());
    assert!(created.context_dir.join("memory").exists());
    let manifest = std::fs::read_to_string(&created.manifest_path)?;
    assert!(manifest.contains("project: context-init-demo"));
    assert!(manifest.contains("instructions: README.md"));
    assert!(manifest.contains("memory_dir: memory"));

    let conflict = runtime
        .service
        .init_project_context(ContextInitInput {
            project: Some(project.slug.clone()),
            workspace_root: Some(tempdir.path().join("workspace")),
            context_dir: None,
            instructions: Some("docs/overview.md".to_string()),
            memory_dir: Some("notes".to_string()),
            force: false,
            dry_run: false,
        })
        .await
        .expect_err("different manifest should require force");
    assert!(matches!(conflict, AppError::Conflict(_)));

    let updated = runtime
        .service
        .init_project_context(ContextInitInput {
            project: Some(project.slug),
            workspace_root: Some(tempdir.path().join("workspace")),
            context_dir: None,
            instructions: Some("docs/overview.md".to_string()),
            memory_dir: Some("notes".to_string()),
            force: true,
            dry_run: false,
        })
        .await?;
    assert_eq!(updated.status, ContextInitStatus::Updated);
    assert!(updated.context_dir.join("notes").exists());
    let updated_manifest = std::fs::read_to_string(&updated.manifest_path)?;
    assert!(updated_manifest.contains("instructions: docs/overview.md"));
    assert!(updated_manifest.contains("memory_dir: notes"));

    Ok(())
}

#[tokio::test]
async fn context_manifest_scopes_queries_and_all_projects_opt_in(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let context_dir = tempdir.path().join("workspace").join(".agenta");
    std::fs::create_dir_all(&context_dir)?;
    std::fs::write(
        context_dir.join("project.yaml"),
        "project: manifest-alpha\ninstructions: README.md\nmemory_dir: memory\n",
    )?;
    let config_path = write_test_config_with_project_context(&tempdir, &context_dir)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let alpha = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "manifest-alpha".to_string(),
            name: "Manifest Alpha".to_string(),
            description: None,
        })
        .await?;
    let beta = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "manifest-beta".to_string(),
            name: "Manifest Beta".to_string(),
            description: None,
        })
        .await?;

    runtime
        .service
        .create_task(CreateTaskInput {
            project: alpha.slug.clone(),
            version: None,
            task_code: None,
            task_kind: None,
            title: "Scoped alpha task".to_string(),
            summary: Some("manifest scoped".to_string()),
            description: None,
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("manifest-test".to_string()),
        })
        .await?;
    runtime
        .service
        .create_task(CreateTaskInput {
            project: beta.slug.clone(),
            version: None,
            task_code: None,
            task_kind: None,
            title: "Scoped beta task".to_string(),
            summary: Some("manifest scoped".to_string()),
            description: None,
            status: Some(TaskStatus::Ready),
            priority: Some(TaskPriority::Normal),
            created_by: Some("manifest-test".to_string()),
        })
        .await?;

    let scoped_tasks = runtime.service.list_tasks(TaskQuery::default()).await?;
    assert_eq!(scoped_tasks.len(), 1);
    assert_eq!(scoped_tasks[0].title, "Scoped alpha task");

    let scoped_search = runtime
        .service
        .search(SearchInput {
            text: None,
            project: None,
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
        .await?;
    assert_eq!(scoped_search.tasks.len(), 1);
    assert_eq!(scoped_search.tasks[0].title, "Scoped alpha task");

    let global_tasks = runtime
        .service
        .list_tasks(TaskQuery {
            all_projects: true,
            ..Default::default()
        })
        .await?;
    assert_eq!(global_tasks.len(), 2);

    let global_search = runtime
        .service
        .search(SearchInput {
            text: Some("Scoped".to_string()),
            project: None,
            version: None,
            status: None,
            priority: None,
            knowledge_status: None,
            task_kind: None,
            task_code_prefix: None,
            title_prefix: None,
            limit: Some(10),
            all_projects: true,
        })
        .await?;
    assert_eq!(global_search.tasks.len(), 2);

    Ok(())
}
