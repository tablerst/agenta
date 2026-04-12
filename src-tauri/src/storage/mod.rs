mod approvals;
mod attachments;
mod mapping;
mod projects;
mod sync;
mod tasks;

use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use tokio::fs;
use uuid::Uuid;

use crate::domain::TaskStatus;
use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct SqliteStore {
    pub(crate) pool: SqlitePool,
    pub(crate) attachments_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub struct StoredAttachmentFile {
    pub mime: String,
    pub original_filename: String,
    pub original_path: String,
    pub storage_path: String,
    pub sha256: String,
    pub size_bytes: i64,
}

#[derive(Clone, Debug, Default)]
pub struct TaskListFilter {
    pub project_id: Option<Uuid>,
    pub version_id: Option<Uuid>,
    pub status: Option<TaskStatus>,
}

impl SqliteStore {
    pub async fn open(
        data_dir: &Path,
        database_path: &Path,
        attachments_dir: &Path,
    ) -> AppResult<Self> {
        fs::create_dir_all(data_dir).await?;
        fs::create_dir_all(attachments_dir).await?;

        let options = SqliteConnectOptions::new()
            .filename(database_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|error| AppError::Storage(error.to_string()))?;

        Ok(Self {
            pool,
            attachments_dir: attachments_dir.to_path_buf(),
        })
    }
}
