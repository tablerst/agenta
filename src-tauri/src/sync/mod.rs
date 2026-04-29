use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use serde_json::Value;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgRow};
use sqlx::{query, raw_sql, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::SyncRemotePostgresConfig;
use crate::domain::{SyncEntityKind, SyncOperation, SyncOutboxEntry};
use crate::error::{AppError, AppResult};

const REMOTE_BASE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS agenta_sync_objects (
    remote_id TEXT NOT NULL,
    entity_kind TEXT NOT NULL,
    remote_entity_id TEXT NOT NULL,
    local_id TEXT NOT NULL,
    local_version BIGINT NOT NULL,
    payload_json JSONB NOT NULL,
    origin_client_id TEXT,
    origin_mutation_id TEXT,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (remote_id, entity_kind, remote_entity_id)
);

CREATE TABLE IF NOT EXISTS agenta_sync_mutations (
    remote_mutation_id BIGSERIAL PRIMARY KEY,
    remote_id TEXT NOT NULL,
    entity_kind TEXT NOT NULL,
    remote_entity_id TEXT NOT NULL,
    local_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    local_version BIGINT NOT NULL,
    base_local_version BIGINT NOT NULL DEFAULT 0,
    origin_client_id TEXT,
    origin_mutation_id TEXT,
    payload_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_agenta_sync_mutations_remote_cursor
    ON agenta_sync_mutations(remote_id, remote_mutation_id ASC);

CREATE TABLE IF NOT EXISTS agenta_sync_attachment_blobs (
    remote_id TEXT NOT NULL,
    remote_entity_id TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    mime TEXT NOT NULL,
    content BYTEA NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (remote_id, remote_entity_id)
);
"#;

const REMOTE_SCHEMA_UPGRADE_SQL: &str = r#"
ALTER TABLE agenta_sync_objects
    ADD COLUMN IF NOT EXISTS origin_client_id TEXT;
ALTER TABLE agenta_sync_objects
    ADD COLUMN IF NOT EXISTS origin_mutation_id TEXT;
ALTER TABLE agenta_sync_mutations
    ADD COLUMN IF NOT EXISTS base_local_version BIGINT NOT NULL DEFAULT 0;
ALTER TABLE agenta_sync_mutations
    ADD COLUMN IF NOT EXISTS origin_client_id TEXT;
ALTER TABLE agenta_sync_mutations
    ADD COLUMN IF NOT EXISTS origin_mutation_id TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_agenta_sync_mutations_origin
    ON agenta_sync_mutations(remote_id, origin_client_id, origin_mutation_id)
    WHERE origin_client_id IS NOT NULL AND origin_mutation_id IS NOT NULL;
"#;

#[derive(Clone, Debug)]
pub struct RemoteMutation {
    pub remote_mutation_id: i64,
    pub entity_kind: SyncEntityKind,
    pub remote_entity_id: String,
    pub local_id: Uuid,
    pub operation: SyncOperation,
    pub local_version: i64,
    pub base_local_version: i64,
    pub origin_client_id: Option<Uuid>,
    pub origin_mutation_id: Option<Uuid>,
    pub payload_json: Value,
    pub created_at: OffsetDateTime,
    pub attachment_blob: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct PushAck {
    pub remote_entity_id: String,
    pub remote_mutation_id: i64,
    pub acked_at: OffsetDateTime,
}

pub struct PostgresSyncRemote {
    pool: sqlx::PgPool,
}

impl PostgresSyncRemote {
    pub async fn connect(config: &SyncRemotePostgresConfig) -> AppResult<Self> {
        let options = PgConnectOptions::from_str(&config.dsn)
            .map_err(|error| AppError::Config(format!("invalid sync postgres dsn: {error}")))?;
        let mut pool_options = PgPoolOptions::new()
            .max_connections(config.max_conns)
            .min_connections(config.min_conns)
            .acquire_timeout(Duration::from_secs(5));
        pool_options = pool_options.max_lifetime(Some(config.max_conn_lifetime));

        let pool = tokio::time::timeout(Duration::from_secs(5), pool_options.connect_with(options))
            .await
            .map_err(|_| AppError::Io("timed out while connecting to sync postgres".to_string()))?
            .map_err(|error| {
                AppError::Io(format!("failed to connect to sync postgres: {error}"))
            })?;

        Ok(Self { pool })
    }

    pub async fn smoke_check(&self) -> AppResult<()> {
        query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|error| AppError::Io(format!("failed postgres smoke query: {error}")))?;
        Ok(())
    }

    pub async fn ensure_schema(&self) -> AppResult<()> {
        raw_sql(REMOTE_BASE_SCHEMA_SQL)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                AppError::Io(format!("failed to initialize remote sync schema: {error}"))
            })?;
        raw_sql(REMOTE_SCHEMA_UPGRADE_SQL)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                AppError::Io(format!("failed to upgrade remote sync schema: {error}"))
            })?;
        Ok(())
    }

    pub async fn push_outbox_entry(
        &self,
        remote_id: &str,
        client_id: Uuid,
        entry: &SyncOutboxEntry,
        attachments_dir: &Path,
    ) -> AppResult<PushAck> {
        let remote_entity_id = entry.local_id.to_string();
        let mut tx = self.pool.begin().await.map_err(|error| {
            AppError::Io(format!(
                "failed to begin remote postgres transaction: {error}"
            ))
        })?;
        let origin_client_id = client_id.to_string();
        let origin_mutation_id = entry.mutation_id.to_string();
        let base_local_version = entry.local_version.saturating_sub(1);

        if let Some(row) = query(
            r#"
            SELECT remote_mutation_id, created_at
            FROM agenta_sync_mutations
            WHERE remote_id = $1
              AND origin_client_id = $2
              AND origin_mutation_id = $3
            "#,
        )
        .bind(remote_id)
        .bind(&origin_client_id)
        .bind(&origin_mutation_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| AppError::Io(format!("failed to check remote mutation ack: {error}")))?
        {
            tx.commit().await.map_err(|error| {
                AppError::Io(format!("failed to commit remote ack check: {error}"))
            })?;
            return Ok(PushAck {
                remote_entity_id,
                remote_mutation_id: row.get("remote_mutation_id"),
                acked_at: row.get("created_at"),
            });
        }

        if let Some(row) = query(
            r#"
            SELECT local_version
            FROM agenta_sync_objects
            WHERE remote_id = $1 AND entity_kind = $2 AND remote_entity_id = $3
            FOR UPDATE
            "#,
        )
        .bind(remote_id)
        .bind(entry.entity_kind.to_string())
        .bind(&remote_entity_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| AppError::Io(format!("failed to inspect remote sync object: {error}")))?
        {
            let remote_version = row.get::<i64, _>("local_version");
            if remote_version > base_local_version {
                return Err(AppError::Conflict(format!(
                    "sync conflict for {} {}: remote version {} is newer than local base {}",
                    entry.entity_kind, entry.local_id, remote_version, base_local_version
                )));
            }
        }

        query(
            r#"
            INSERT INTO agenta_sync_objects (
                remote_id, entity_kind, remote_entity_id, local_id, local_version,
                payload_json, origin_client_id, origin_mutation_id, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8, NOW())
            ON CONFLICT (remote_id, entity_kind, remote_entity_id) DO UPDATE SET
                local_id = EXCLUDED.local_id,
                local_version = EXCLUDED.local_version,
                payload_json = EXCLUDED.payload_json,
                origin_client_id = EXCLUDED.origin_client_id,
                origin_mutation_id = EXCLUDED.origin_mutation_id,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(remote_id)
        .bind(entry.entity_kind.to_string())
        .bind(&remote_entity_id)
        .bind(entry.local_id.to_string())
        .bind(entry.local_version)
        .bind(entry.payload_json.to_string())
        .bind(&origin_client_id)
        .bind(&origin_mutation_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| AppError::Io(format!("failed to upsert remote sync object: {error}")))?;

        if entry.entity_kind == SyncEntityKind::Attachment {
            let storage_path = entry
                .payload_json
                .get("storage_path")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AppError::InvalidArguments(
                        "attachment sync payload missing storage_path".to_string(),
                    )
                })?;
            let sha256 = entry
                .payload_json
                .get("sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AppError::InvalidArguments("attachment sync payload missing sha256".to_string())
                })?;
            let mime = entry
                .payload_json
                .get("mime")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    AppError::InvalidArguments("attachment sync payload missing mime".to_string())
                })?;
            let size_bytes = entry
                .payload_json
                .get("size_bytes")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    AppError::InvalidArguments(
                        "attachment sync payload missing size_bytes".to_string(),
                    )
                })?;
            let content = tokio::fs::read(attachments_dir.join(storage_path))
                .await
                .map_err(|error| {
                    AppError::Io(format!(
                        "failed to read local attachment for remote push: {error}"
                    ))
                })?;

            query(
                r#"
                INSERT INTO agenta_sync_attachment_blobs (
                    remote_id, remote_entity_id, sha256, size_bytes, mime, content, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, NOW())
                ON CONFLICT (remote_id, remote_entity_id) DO UPDATE SET
                    sha256 = EXCLUDED.sha256,
                    size_bytes = EXCLUDED.size_bytes,
                    mime = EXCLUDED.mime,
                    content = EXCLUDED.content,
                    updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(remote_id)
            .bind(&remote_entity_id)
            .bind(sha256)
            .bind(size_bytes)
            .bind(mime)
            .bind(content)
            .execute(&mut *tx)
            .await
            .map_err(|error| {
                AppError::Io(format!("failed to upsert remote attachment blob: {error}"))
            })?;
        }

        let row = query(
            r#"
            INSERT INTO agenta_sync_mutations (
                remote_id, entity_kind, remote_entity_id, local_id, operation,
                local_version, base_local_version, origin_client_id, origin_mutation_id, payload_json
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::jsonb)
            RETURNING remote_mutation_id, created_at
            "#,
        )
        .bind(remote_id)
        .bind(entry.entity_kind.to_string())
        .bind(&remote_entity_id)
        .bind(entry.local_id.to_string())
        .bind(entry.operation.to_string())
        .bind(entry.local_version)
        .bind(base_local_version)
        .bind(&origin_client_id)
        .bind(&origin_mutation_id)
        .bind(entry.payload_json.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| AppError::Io(format!("failed to append remote mutation: {error}")))?;

        tx.commit()
            .await
            .map_err(|error| AppError::Io(format!("failed to commit remote mutation: {error}")))?;

        Ok(PushAck {
            remote_entity_id,
            remote_mutation_id: row.get("remote_mutation_id"),
            acked_at: row.get("created_at"),
        })
    }

    pub async fn pull_mutations(
        &self,
        remote_id: &str,
        after_remote_mutation_id: Option<i64>,
        limit: usize,
    ) -> AppResult<Vec<RemoteMutation>> {
        let rows = query(
            r#"
            SELECT
                m.remote_mutation_id,
                m.entity_kind,
                m.remote_entity_id,
                m.local_id,
                m.operation,
                m.local_version,
                m.base_local_version,
                m.origin_client_id,
                m.origin_mutation_id,
                m.payload_json,
                m.created_at,
                b.content AS attachment_blob
            FROM agenta_sync_mutations m
            LEFT JOIN agenta_sync_attachment_blobs b
              ON b.remote_id = m.remote_id AND b.remote_entity_id = m.remote_entity_id
            WHERE m.remote_id = $1
              AND ($2::BIGINT IS NULL OR m.remote_mutation_id > $2)
            ORDER BY m.remote_mutation_id ASC
            LIMIT $3
            "#,
        )
        .bind(remote_id)
        .bind(after_remote_mutation_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| AppError::Io(format!("failed to pull remote mutations: {error}")))?;

        rows.into_iter().map(map_remote_mutation).collect()
    }

    pub async fn latest_mutation_id(&self, remote_id: &str) -> AppResult<Option<i64>> {
        let row = query(
            r#"
            SELECT MAX(remote_mutation_id) AS latest_mutation_id
            FROM agenta_sync_mutations
            WHERE remote_id = $1
            "#,
        )
        .bind(remote_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            AppError::Io(format!("failed to inspect remote mutation cursor: {error}"))
        })?;

        Ok(row.get("latest_mutation_id"))
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

fn map_remote_mutation(row: PgRow) -> AppResult<RemoteMutation> {
    let local_id = Uuid::parse_str(row.get::<String, _>("local_id").as_str())
        .map_err(|error| AppError::Storage(format!("invalid remote local_id uuid: {error}")))?;
    let payload_json = row.get::<Value, _>("payload_json");
    let origin_client_id = row
        .get::<Option<String>, _>("origin_client_id")
        .map(|value| {
            Uuid::parse_str(&value).map_err(|error| {
                AppError::Storage(format!("invalid remote origin_client_id uuid: {error}"))
            })
        })
        .transpose()?;
    let origin_mutation_id = row
        .get::<Option<String>, _>("origin_mutation_id")
        .map(|value| {
            Uuid::parse_str(&value).map_err(|error| {
                AppError::Storage(format!("invalid remote origin_mutation_id uuid: {error}"))
            })
        })
        .transpose()?;

    Ok(RemoteMutation {
        remote_mutation_id: row.get("remote_mutation_id"),
        entity_kind: row
            .get::<String, _>("entity_kind")
            .parse()
            .map_err(|error: String| AppError::Storage(error))?,
        remote_entity_id: row.get("remote_entity_id"),
        local_id,
        operation: row
            .get::<String, _>("operation")
            .parse()
            .map_err(|error: String| AppError::Storage(error))?,
        local_version: row.get("local_version"),
        base_local_version: row.get("base_local_version"),
        origin_client_id,
        origin_mutation_id,
        payload_json,
        created_at: row.get("created_at"),
        attachment_blob: row.get("attachment_blob"),
    })
}

#[cfg(test)]
mod tests {
    use super::{REMOTE_BASE_SCHEMA_SQL, REMOTE_SCHEMA_UPGRADE_SQL};

    #[test]
    fn remote_schema_creates_origin_index_after_upgrade_columns() {
        assert!(!REMOTE_BASE_SCHEMA_SQL.contains("idx_agenta_sync_mutations_origin"));

        let origin_column = REMOTE_SCHEMA_UPGRADE_SQL
            .find("ADD COLUMN IF NOT EXISTS origin_client_id TEXT")
            .expect("upgrade adds origin_client_id");
        let origin_index = REMOTE_SCHEMA_UPGRADE_SQL
            .find("idx_agenta_sync_mutations_origin")
            .expect("upgrade creates origin idempotency index");

        assert!(origin_column < origin_index);
    }
}
