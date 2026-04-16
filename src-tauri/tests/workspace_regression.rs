use std::sync::Arc;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, Executor, SqliteConnection};
use tempfile::TempDir;
use tokio::task::JoinSet;

use agenta_lib::{
    app::{AppRuntime, BootstrapOptions},
    domain::{
        KnowledgeStatus, NoteKind, TaskActivityKind, TaskKind, TaskPriority, TaskStatus,
        VersionStatus,
    },
    error::AppError,
    service::{
        CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
        CreateVersionInput, PageRequest, SearchInput, SortOrder, TaskQuery, TaskSortBy,
        UpdateProjectInput, UpdateTaskInput, UpdateVersionInput,
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
    let yaml = format!(
        "paths:\n  data_dir: {}\nmcp:\n  bind: \"127.0.0.1:8787\"\n  path: \"/mcp\"\n",
        normalize_path_for_yaml(&data_dir),
    );
    std::fs::write(&config_path, yaml)?;
    Ok(config_path)
}

fn normalize_path_for_yaml(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
            content: "Scratch context note".to_string(),
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

    let filtered_search = runtime
        .service
        .search(SearchInput {
            text: None,
            project: Some(project.slug.clone()),
            version: Some(version.version_id.to_string()),
            task_kind: Some(TaskKind::Context),
            task_code_prefix: Some("InitCtx-".to_string()),
            title_prefix: None,
            limit: Some(10),
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
            task_kind: None,
            task_code_prefix: None,
            title_prefix: None,
            limit: Some(10),
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
