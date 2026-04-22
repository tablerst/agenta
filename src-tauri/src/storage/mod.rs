mod approvals;
mod attachments;
mod mapping;
mod projects;
mod relations;
mod sync;
mod tasks;

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use time::OffsetDateTime;
use tokio::fs;
use uuid::Uuid;

use crate::domain::{KnowledgeStatus, TaskKind, TaskPriority, TaskStatus};
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
    pub extracted_search_text: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct TaskListFilter {
    pub project_id: Option<Uuid>,
    pub version_id: Option<Uuid>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub knowledge_status: Option<KnowledgeStatus>,
    pub task_kind: Option<TaskKind>,
    pub task_code_prefix: Option<String>,
    pub title_prefix: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TaskLexicalSearchRow {
    pub task_id: String,
    pub task_code: Option<String>,
    pub task_kind: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub knowledge_status: String,
    pub task_search_summary: String,
    pub task_context_digest: String,
    pub latest_note_summary: Option<String>,
    pub lexical_score: f64,
    pub lexical_rank: usize,
    pub latest_activity_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct ActivityLexicalSearchRow {
    pub activity_id: String,
    pub task_id: String,
    pub kind: String,
    pub summary: String,
    pub search_text: String,
    pub score: f64,
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
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
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
