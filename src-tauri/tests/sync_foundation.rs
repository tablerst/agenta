use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use agenta_lib::{
    app::{AppRuntime, BootstrapOptions},
    domain::{SyncCheckpointKind, SyncEntityKind, TaskStatus},
    error::AppError,
    service::{
        CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
        CreateVersionInput, RequestOrigin, ReviewApprovalInput, UpdateTaskInput,
    },
    storage::SqliteStore,
};
use assert_cmd::Command;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::Row;
use tempfile::TempDir;
use time::OffsetDateTime;

const SYNC_TOKEN_ENV: &str = "AGENTA_SYNC_BEARER_TOKEN";
const FAIL_SYNC_OUTBOX_WRITE_ENV: &str = "AGENTA_TEST_FAIL_SYNC_OUTBOX_WRITE";

fn environment_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[tokio::test]
async fn sync_migrations_create_tables_and_cli_status_is_stable(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = environment_lock().lock().expect("lock environment");
    std::env::set_var(SYNC_TOKEN_ENV, "sync-token");

    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir, None)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path.clone()),
    })
    .await?;

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&runtime.config.paths.database_path)
                .create_if_missing(false)
                .foreign_keys(true),
        )
        .await?;

    let tables = sqlx::query(
        r#"
        SELECT name
        FROM sqlite_master
        WHERE type = 'table'
          AND name IN ('sync_entities', 'sync_outbox', 'sync_checkpoints', 'sync_tombstones')
        ORDER BY name
        "#,
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(tables.len(), 4);

    let store = SqliteStore::open(
        &runtime.config.paths.data_dir,
        &runtime.config.paths.database_path,
        &runtime.config.paths.attachments_dir,
    )
    .await?;
    store
        .upsert_sync_checkpoint(
            "primary",
            SyncCheckpointKind::Pull,
            "pull-cursor-1",
            OffsetDateTime::now_utc(),
        )
        .await?;
    store
        .upsert_sync_checkpoint(
            "primary",
            SyncCheckpointKind::PushAck,
            "push-ack-9",
            OffsetDateTime::now_utc(),
        )
        .await?;

    let output = run_cli_json(
        &config_path,
        &["sync", "status"],
    )?;

    assert_eq!(output["ok"], true);
    assert_eq!(output["action"], "sync.status");
    assert_eq!(output["result"]["enabled"], true);
    assert_eq!(output["result"]["mode"], "manual_bidirectional");
    assert_eq!(output["result"]["remote_id"], "primary");
    assert_eq!(
        output["result"]["remote_endpoint"],
        "https://example.invalid/sync"
    );
    assert_eq!(output["result"]["pending_outbox_count"], 0);
    assert!(output["result"]["oldest_pending_at"].is_null());
    assert_eq!(output["result"]["checkpoints"]["pull"], "pull-cursor-1");
    assert_eq!(output["result"]["checkpoints"]["push_ack"], "push-ack-9");

    std::env::remove_var(SYNC_TOKEN_ENV);
    Ok(())
}

#[tokio::test]
async fn sync_core_writes_enqueue_outbox_and_cli_lists_recent_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = environment_lock().lock().expect("lock environment");
    std::env::set_var(SYNC_TOKEN_ENV, "sync-token");

    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir, None)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path.clone()),
    })
    .await?;

    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "sync-project".to_string(),
            name: "Sync Project".to_string(),
            description: Some("Sync enabled project".to_string()),
        })
        .await?;
    let version = runtime
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: "Sync v1".to_string(),
            description: Some("Sync lane".to_string()),
            status: None,
        })
        .await?;
    let task = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug.clone(),
            version: Some(version.version_id.to_string()),
            title: "Sync Task".to_string(),
            summary: Some("Sync task summary".to_string()),
            description: Some("Sync task description".to_string()),
            status: None,
            priority: None,
            created_by: Some("sync-test".to_string()),
        })
        .await?;
    let note = runtime
        .service
        .create_note(CreateNoteInput {
            task: task.task_id.to_string(),
            content: "Sync note content".to_string(),
            created_by: Some("sync-test".to_string()),
        })
        .await?;
    let source_path = tempdir.path().join("sync-attachment.txt");
    std::fs::write(&source_path, "sync attachment payload")?;
    let attachment = runtime
        .service
        .create_attachment(CreateAttachmentInput {
            task: task.task_id.to_string(),
            path: source_path,
            kind: None,
            created_by: Some("sync-test".to_string()),
            summary: Some("Sync attachment".to_string()),
        })
        .await?;
    let updated_task = runtime
        .service
        .update_task(
            &task.task_id.to_string(),
            UpdateTaskInput {
                status: Some(TaskStatus::Done),
                updated_by: Some("sync-reviewer".to_string()),
                ..Default::default()
            },
        )
        .await?;

    let outbox = runtime.service.list_sync_outbox(Some(20)).await?;
    assert_eq!(outbox.len(), 7);
    assert!(outbox.iter().any(|item| item.entity_kind == SyncEntityKind::Project));
    assert!(outbox.iter().any(|item| item.entity_kind == SyncEntityKind::Version));
    assert!(outbox.iter().any(|item| item.entity_kind == SyncEntityKind::Task));
    assert!(outbox.iter().any(|item| item.entity_kind == SyncEntityKind::Note));
    assert!(outbox.iter().any(|item| item.entity_kind == SyncEntityKind::Attachment));

    let store = SqliteStore::open(
        &runtime.config.paths.data_dir,
        &runtime.config.paths.database_path,
        &runtime.config.paths.attachments_dir,
    )
    .await?;
    let project_sync = store
        .get_sync_entity(SyncEntityKind::Project, project.project_id)
        .await?
        .expect("project sync state");
    let version_sync = store
        .get_sync_entity(SyncEntityKind::Version, version.version_id)
        .await?
        .expect("version sync state");
    let task_sync = store
        .get_sync_entity(SyncEntityKind::Task, task.task_id)
        .await?
        .expect("task sync state");
    let note_sync = store
        .get_sync_entity(SyncEntityKind::Note, note.activity_id)
        .await?
        .expect("note sync state");
    let attachment_sync = store
        .get_sync_entity(SyncEntityKind::Attachment, attachment.attachment_id)
        .await?
        .expect("attachment sync state");

    assert_eq!(project_sync.local_version, 2);
    assert_eq!(version_sync.local_version, 1);
    assert_eq!(task_sync.local_version, 2);
    assert_eq!(note_sync.local_version, 1);
    assert_eq!(attachment_sync.local_version, 1);
    assert!(task_sync.dirty);
    assert_eq!(updated_task.status, TaskStatus::Done);

    let payload_row = sqlx::query(
        r#"
        SELECT payload_json
        FROM sync_outbox
        WHERE entity_kind = ? AND local_id = ?
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(SyncEntityKind::Attachment.to_string())
    .bind(attachment.attachment_id.to_string())
    .fetch_one(&pool_from_path(&runtime.config.paths.database_path).await?)
    .await?;
    let payload_json = payload_row.get::<String, _>("payload_json");
    assert!(payload_json.contains("\"storage_path\""));
    assert!(payload_json.contains(&attachment.storage_path));
    assert!(!payload_json.contains("sync attachment payload"));

    let cli_outbox = run_cli_json(
        &config_path,
        &["sync", "outbox", "list", "--limit", "3"],
    )?;
    let items = cli_outbox["result"]
        .as_array()
        .expect("sync outbox list result array");
    assert_eq!(items.len(), 3);
    assert!(items[0]["mutation_id"].is_string());
    assert!(items[0]["entity_kind"].is_string());
    assert!(items[0]["local_id"].is_string());

    std::env::remove_var(SYNC_TOKEN_ENV);
    Ok(())
}

#[tokio::test]
async fn approval_replay_writes_sync_outbox_entries() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = environment_lock().lock().expect("lock environment");
    std::env::set_var(SYNC_TOKEN_ENV, "sync-token");

    let tempdir = TempDir::new()?;
    let config_path = write_test_config(
        &tempdir,
        Some(
            "policy:\n  default: auto\n  actions:\n    project.create: require_human\n",
        ),
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
                slug: "approval-sync".to_string(),
                name: "Approval Sync".to_string(),
                description: Some("Replay should write sync outbox".to_string()),
            },
        )
        .await
        .expect_err("project create should require approval");
    let request_id = match error {
        AppError::PolicyBlocked {
            approval_request_id: Some(request_id),
            ..
        } => request_id.to_string(),
        other => panic!("unexpected error: {other:?}"),
    };

    let reviewed = runtime
        .service
        .approve_approval_request(
            &request_id,
            ReviewApprovalInput {
                reviewed_by: Some("sync-reviewer".to_string()),
                review_note: Some("approved".to_string()),
            },
        )
        .await?;
    assert_eq!(reviewed.status.to_string(), "approved");

    let outbox = runtime.service.list_sync_outbox(Some(10)).await?;
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox[0].entity_kind, SyncEntityKind::Project);
    assert_eq!(outbox[0].operation.to_string(), "create");

    std::env::remove_var(SYNC_TOKEN_ENV);
    Ok(())
}

#[tokio::test]
async fn forced_sync_outbox_failure_rolls_back_project_write(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = environment_lock().lock().expect("lock environment");
    std::env::set_var(SYNC_TOKEN_ENV, "sync-token");
    std::env::set_var(FAIL_SYNC_OUTBOX_WRITE_ENV, "1");

    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir, None)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let error = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "rollback-project".to_string(),
            name: "Rollback Project".to_string(),
            description: None,
        })
        .await
        .expect_err("forced sync failure should roll back project");
    assert!(error.to_string().contains("forced sync outbox write failure"));

    std::env::remove_var(FAIL_SYNC_OUTBOX_WRITE_ENV);

    assert!(runtime.service.list_projects().await?.is_empty());
    assert!(runtime.service.list_sync_outbox(Some(10)).await?.is_empty());

    std::env::remove_var(SYNC_TOKEN_ENV);
    Ok(())
}

#[tokio::test]
async fn forced_sync_outbox_failure_rolls_back_attachment_and_cleans_file(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = environment_lock().lock().expect("lock environment");
    std::env::set_var(SYNC_TOKEN_ENV, "sync-token");

    let tempdir = TempDir::new()?;
    let config_path = write_test_config(&tempdir, None)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(config_path),
    })
    .await?;

    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: "attachment-rollback".to_string(),
            name: "Attachment Rollback".to_string(),
            description: None,
        })
        .await?;
    let task = runtime
        .service
        .create_task(CreateTaskInput {
            project: project.slug,
            version: None,
            title: "Attachment rollback task".to_string(),
            summary: None,
            description: None,
            status: None,
            priority: None,
            created_by: Some("sync-test".to_string()),
        })
        .await?;

    let source_path = tempdir.path().join("rollback-attachment.txt");
    std::fs::write(&source_path, "rollback attachment payload")?;

    std::env::set_var(FAIL_SYNC_OUTBOX_WRITE_ENV, "1");
    let error = runtime
        .service
        .create_attachment(CreateAttachmentInput {
            task: task.task_id.to_string(),
            path: source_path,
            kind: None,
            created_by: Some("sync-test".to_string()),
            summary: Some("Rollback attachment".to_string()),
        })
        .await
        .expect_err("forced sync failure should roll back attachment");
    std::env::remove_var(FAIL_SYNC_OUTBOX_WRITE_ENV);

    assert!(error.to_string().contains("forced sync outbox write failure"));
    assert!(runtime
        .service
        .list_attachments(&task.task_id.to_string())
        .await?
        .is_empty());
    assert!(runtime.service.list_task_activities(&task.task_id.to_string()).await?.is_empty());

    let task_dir = runtime
        .config
        .paths
        .attachments_dir
        .join(task.task_id.to_string());
    if task_dir.exists() {
        assert!(std::fs::read_dir(task_dir)?.next().is_none());
    }

    std::env::remove_var(SYNC_TOKEN_ENV);
    Ok(())
}

fn write_test_config(
    tempdir: &TempDir,
    extra_block: Option<&str>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config_path = tempdir.path().join("agenta.local.yaml");
    let data_dir = normalize_path_for_yaml(&tempdir.path().join("data"));
    let mut yaml = format!(
        "paths:\n  data_dir: {data_dir}\nmcp:\n  bind: \"127.0.0.1:8787\"\n  path: \"/mcp\"\nsync:\n  enabled: true\n  mode: manual_bidirectional\n  remote:\n    id: primary\n    endpoint: https://example.invalid/sync\n    auth:\n      bearer_token: ${{{SYNC_TOKEN_ENV}}}\n"
    );
    if let Some(extra_block) = extra_block {
        yaml.push_str(extra_block);
    }
    std::fs::write(&config_path, yaml)?;
    Ok(config_path)
}

fn run_cli_json(
    config_path: &Path,
    args: &[&str],
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut command = Command::cargo_bin("agenta")?;
    let output = command
        .arg("--config")
        .arg(config_path)
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    Ok(serde_json::from_slice(&output)?)
}

async fn pool_from_path(path: &Path) -> Result<sqlx::SqlitePool, Box<dyn std::error::Error>> {
    Ok(SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(false)
                .foreign_keys(true),
        )
        .await?)
}

fn normalize_path_for_yaml(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
