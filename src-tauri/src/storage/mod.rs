mod approvals;
mod attachments;
mod mapping;
mod projects;
mod relations;
mod sync;
mod task_activity;
mod task_helpers;
mod task_search;
mod task_search_index;
mod tasks;

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha384};
use sqlx::migrate::{Migration, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow};
use sqlx::{query, QueryBuilder, Row, Sqlite, SqlitePool, Transaction};
use time::OffsetDateTime;
use tokio::fs;
use uuid::Uuid;

use crate::domain::{
    KnowledgeStatus, Task, TaskActivity, TaskActivityKind, TaskKind, TaskPriority, TaskStatus,
};
use crate::error::{AppError, AppResult};
use crate::search::{
    build_activity_chunk_vector_document_text, build_activity_search_chunks,
    build_task_vector_document_text, SearchVectorJob, TaskVectorDocument,
};

use mapping::{format_time, map_activity, map_task, parse_time};
use task_helpers::*;

const SEARCH_INDEX_JOB_LEASE_SECONDS: i64 = 300;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

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
    pub project_id: String,
    pub version_id: Option<String>,
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
    pub project_id: String,
    pub version_id: Option<String>,
    pub task_title: String,
    pub kind: String,
    pub summary: String,
    pub chunk_id: String,
    pub chunk_index: i64,
    pub search_text: String,
    pub attachment_id: Option<String>,
    pub score: f64,
}

#[derive(Clone, Debug)]
pub struct ActivityChunkRecord {
    pub chunk_id: String,
    pub activity_id: String,
    pub task_id: String,
    pub project_id: String,
    pub version_id: Option<String>,
    pub task_title: String,
    pub kind: String,
    pub summary: String,
    pub chunk_index: i64,
    pub chunk_text: String,
    pub attachment_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SearchIndexQueueStats {
    pub total_count: usize,
    pub pending_count: usize,
    pub processing_count: usize,
    pub failed_count: usize,
    pub due_count: usize,
    pub stale_processing_count: usize,
    pub next_retry_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SearchIndexJobRecord {
    pub task_id: Uuid,
    pub title: Option<String>,
    pub status: String,
    pub attempt_count: i64,
    pub last_error: Option<String>,
    pub next_attempt_at: Option<OffsetDateTime>,
    pub locked_at: Option<OffsetDateTime>,
    pub lease_until: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
    pub run_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct SearchIndexDocumentRecord {
    pub vector_id: String,
    pub task_id: Uuid,
    pub source_kind: String,
    pub document_hash: String,
    pub embedding_fingerprint: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct SearchIndexDocumentUpsert {
    pub vector_id: String,
    pub task_id: Uuid,
    pub source_kind: String,
    pub document_hash: String,
    pub embedding_fingerprint: String,
}

#[derive(Clone, Debug)]
pub struct SearchIndexEmbeddingProfileRecord {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub fingerprint: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct SearchIndexRunRecord {
    pub run_id: Uuid,
    pub status: String,
    pub trigger_kind: String,
    pub scanned: usize,
    pub queued: usize,
    pub skipped: usize,
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub unchanged: usize,
    pub batch_size: usize,
    pub embedding_fingerprint: Option<String>,
    pub started_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
    pub last_error: Option<String>,
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct SearchIndexRunQueueStats {
    pub pending_count: usize,
    pub processing_count: usize,
    pub retrying_count: usize,
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
        repair_line_ending_only_migration_checksums(&pool, &MIGRATOR).await?;
        MIGRATOR.run(&pool).await.map_err(AppError::from)?;

        Ok(Self {
            pool,
            attachments_dir: attachments_dir.to_path_buf(),
        })
    }
}

async fn repair_line_ending_only_migration_checksums(
    pool: &SqlitePool,
    migrator: &Migrator,
) -> AppResult<()> {
    let table_exists: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT 1
        FROM sqlite_master
        WHERE type = 'table'
          AND name = '_sqlx_migrations'
        "#,
    )
    .fetch_optional(pool)
    .await?;

    if table_exists.is_none() {
        return Ok(());
    }

    let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        r#"
        SELECT version, checksum
        FROM _sqlx_migrations
        WHERE success = TRUE
        ORDER BY version
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (version, applied_checksum) in rows {
        let Some(migration) = migrator
            .iter()
            .find(|migration| migration.version == version)
        else {
            continue;
        };

        if applied_checksum.as_slice() == migration.checksum.as_ref() {
            continue;
        }

        if migration_line_ending_checksum_matches(migration, &applied_checksum) {
            sqlx::query(
                r#"
                UPDATE _sqlx_migrations
                SET checksum = ?
                WHERE version = ?
                  AND checksum = ?
                "#,
            )
            .bind(migration.checksum.as_ref())
            .bind(version)
            .bind(applied_checksum)
            .execute(pool)
            .await?;

            tracing::info!(
                migration_version = version,
                "repaired SQLx migration checksum after line-ending normalization"
            );
        }
    }

    Ok(())
}

fn migration_line_ending_checksum_matches(migration: &Migration, checksum: &[u8]) -> bool {
    let normalized_lf = normalize_sql_line_endings_to_lf(migration.sql.as_ref());
    let normalized_crlf = normalized_lf.replace('\n', "\r\n");

    checksum == sha384_bytes(normalized_lf.as_bytes()).as_slice()
        || checksum == sha384_bytes(normalized_crlf.as_bytes()).as_slice()
}

fn normalize_sql_line_endings_to_lf(sql: &str) -> String {
    sql.replace("\r\n", "\n").replace('\r', "\n")
}

fn sha384_bytes(bytes: &[u8]) -> Vec<u8> {
    Sha384::digest(bytes).to_vec()
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use sqlx::migrate::MigrationType;

    #[test]
    fn migration_line_ending_checksum_matcher_accepts_only_lf_crlf_variants() {
        let sql = "CREATE TABLE example (\n    id TEXT PRIMARY KEY\n);\n";
        let migration = Migration {
            version: 1,
            description: Cow::Borrowed("example"),
            migration_type: MigrationType::Simple,
            sql: Cow::Borrowed(sql),
            checksum: Cow::Owned(sha384_bytes(sql.as_bytes())),
            no_tx: false,
        };

        let crlf_checksum = sha384_bytes(
            normalize_sql_line_endings_to_lf(sql)
                .replace('\n', "\r\n")
                .as_bytes(),
        );
        let changed_checksum = sha384_bytes(
            "CREATE TABLE example (\n    id TEXT PRIMARY KEY,\n    name TEXT\n);\n".as_bytes(),
        );

        assert!(migration_line_ending_checksum_matches(
            &migration,
            migration.checksum.as_ref()
        ));
        assert!(migration_line_ending_checksum_matches(
            &migration,
            &crlf_checksum
        ));
        assert!(!migration_line_ending_checksum_matches(
            &migration,
            &changed_checksum
        ));
    }
}
