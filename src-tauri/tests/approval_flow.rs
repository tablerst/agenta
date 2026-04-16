use tempfile::TempDir;

use agenta_lib::{
    app::{AppRuntime, BootstrapOptions},
    error::AppError,
    service::{
        ApprovalQuery, CreateAttachmentInput, CreateProjectInput, CreateTaskInput, RequestOrigin,
        ReviewApprovalInput,
    },
};

#[tokio::test]
async fn require_human_creates_pending_request_and_replay_approves(
) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(
        &tempdir,
        "policy:\n  default: auto\n  actions:\n    project.create: require_human\n",
    )?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let error = runtime
        .service
        .create_project_from(
            RequestOrigin::Desktop,
            CreateProjectInput {
                slug: "approval-demo".to_string(),
                name: "Approval Demo".to_string(),
                description: Some("Needs human review".to_string()),
            },
        )
        .await
        .expect_err("project should require human review");

    let request_id = match error {
        AppError::PolicyBlocked {
            approval_request_id: Some(request_id),
            ..
        } => request_id.to_string(),
        other => panic!("unexpected error: {other:?}"),
    };

    assert_eq!(runtime.service.list_projects().await?.len(), 0);

    let pending = runtime
        .service
        .list_approval_requests(ApprovalQuery {
            project: Some("approval-demo".to_string()),
            status: Some(agenta_lib::domain::ApprovalStatus::Pending),
        })
        .await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].project_ref.as_deref(), Some("approval-demo"));
    assert_eq!(pending[0].project_name.as_deref(), Some("Approval Demo"));
    assert_eq!(pending[0].task_ref, None);

    let approved = runtime
        .service
        .approve_approval_request(
            &request_id,
            ReviewApprovalInput {
                reviewed_by: Some("tester".to_string()),
                review_note: Some("Looks safe".to_string()),
            },
        )
        .await?;

    assert_eq!(approved.status.to_string(), "approved");
    assert_eq!(runtime.service.list_projects().await?.len(), 1);

    Ok(())
}

#[tokio::test]
async fn deny_does_not_create_approval_request() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(
        &tempdir,
        "policy:\n  default: auto\n  actions:\n    task.create: deny\n",
    )?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "deny-demo".to_string(),
            name: "Deny Demo".to_string(),
            description: None,
        })
        .await?;

    let error = runtime
        .service
        .create_task_from(
            RequestOrigin::Desktop,
            CreateTaskInput {
                project: project.slug,
                version: None,
                task_code: None,
                task_kind: None,
                title: "Denied write".to_string(),
                summary: None,
                description: None,
                status: None,
                priority: None,
                created_by: Some("desktop".to_string()),
            },
        )
        .await
        .expect_err("task create should be denied");

    match error {
        AppError::PolicyBlocked {
            approval_request_id: None,
            ..
        } => {}
        other => panic!("unexpected error: {other:?}"),
    }

    let pending = runtime
        .service
        .list_approval_requests(ApprovalQuery::default())
        .await?;
    assert!(pending.is_empty());

    Ok(())
}

#[tokio::test]
async fn failed_replay_marks_request_failed() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(
        &tempdir,
        "policy:\n  default: auto\n  actions:\n    attachment.create: require_human\n",
    )?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "attachment-demo".to_string(),
            name: "Attachment Demo".to_string(),
            description: None,
        })
        .await?;
    let task = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug,
            version: None,
            task_code: None,
            task_kind: None,
            title: "Queued attachment".to_string(),
            summary: None,
            description: None,
            status: None,
            priority: None,
            created_by: Some("desktop".to_string()),
        })
        .await?;

    let source = tempdir.path().join("queued.txt");
    std::fs::write(&source, "queued attachment")?;

    let error = runtime
        .service
        .create_attachment_from(
            RequestOrigin::Desktop,
            CreateAttachmentInput {
                task: task.task_id.to_string(),
                path: source.clone(),
                kind: None,
                created_by: Some("desktop".to_string()),
                summary: Some("Queued attachment".to_string()),
            },
        )
        .await
        .expect_err("attachment should queue approval");

    let request_id = match error {
        AppError::PolicyBlocked {
            approval_request_id: Some(request_id),
            ..
        } => request_id.to_string(),
        other => panic!("unexpected error: {other:?}"),
    };

    std::fs::remove_file(&source)?;

    let reviewed = runtime
        .service
        .approve_approval_request(
            &request_id,
            ReviewApprovalInput {
                reviewed_by: Some("tester".to_string()),
                review_note: Some("Retry missing file".to_string()),
            },
        )
        .await?;
    let task_id = task.task_id.to_string();

    assert_eq!(reviewed.status.to_string(), "failed");
    assert_eq!(reviewed.project_ref.as_deref(), Some("attachment-demo"));
    assert_eq!(reviewed.task_ref.as_deref(), Some(task_id.as_str()));
    assert!(reviewed.error_json.is_some());
    assert!(runtime
        .service
        .list_attachments(&task.task_id.to_string())
        .await?
        .is_empty());

    Ok(())
}

#[tokio::test]
async fn pending_approvals_survive_restart() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let config_path = write_test_config(
        &tempdir,
        "policy:\n  default: auto\n  actions:\n    project.create: require_human\n",
    )?;

    let first_runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path.clone()),
    })
    .await?;
    let error = first_runtime
        .service
        .create_project_from(
            RequestOrigin::Desktop,
            CreateProjectInput {
                slug: "restart-demo".to_string(),
                name: "Restart Demo".to_string(),
                description: None,
            },
        )
        .await
        .expect_err("project should queue approval");

    let request_id = match error {
        AppError::PolicyBlocked {
            approval_request_id: Some(request_id),
            ..
        } => request_id.to_string(),
        other => panic!("unexpected error: {other:?}"),
    };

    drop(first_runtime);

    let second_runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;
    let pending = second_runtime
        .service
        .list_approval_requests(ApprovalQuery {
            project: Some("restart-demo".to_string()),
            status: Some(agenta_lib::domain::ApprovalStatus::Pending),
        })
        .await?;

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].request_id.to_string(), request_id);
    assert_eq!(pending[0].project_ref.as_deref(), Some("restart-demo"));

    Ok(())
}

fn write_test_config(
    tempdir: &TempDir,
    policy_block: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config_path = tempdir.path().join("agenta.local.yaml");
    let data_dir = tempdir.path().join("data");
    let yaml = format!(
        "paths:\n  data_dir: {}\nmcp:\n  bind: \"127.0.0.1:8787\"\n  path: \"/mcp\"\n{}",
        normalize_path_for_yaml(&data_dir),
        policy_block
    );
    std::fs::write(&config_path, yaml)?;
    Ok(config_path)
}

fn normalize_path_for_yaml(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
