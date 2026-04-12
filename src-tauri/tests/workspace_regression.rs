use tempfile::TempDir;

use agenta_lib::{
    app::{AppRuntime, BootstrapOptions},
    domain::{TaskActivityKind, TaskPriority, TaskStatus, VersionStatus},
    error::AppError,
    service::{
        CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
        CreateVersionInput, PageRequest, TaskQuery, UpdateProjectInput, UpdateTaskInput,
        UpdateVersionInput,
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
    assert_eq!(updated_project.default_version_id, Some(alpha_v2.version_id));

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
