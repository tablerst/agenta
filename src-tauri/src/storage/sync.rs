use std::env;

use serde_json::Value;
use sqlx::{query, Row, Sqlite, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::{
    SyncCheckpoint, SyncCheckpointKind, SyncEntityKind, SyncEntityState, SyncOperation,
    SyncOutboxEntry, SyncOutboxStatus, SyncTombstone,
};
use crate::error::{AppError, AppResult};

use super::mapping::{
    format_time, map_sync_checkpoint, map_sync_entity, map_sync_outbox_entry, map_sync_tombstone,
    parse_time,
};
use super::SqliteStore;

const DEFAULT_OUTBOX_LIST_LIMIT: usize = 20;
const MAX_OUTBOX_LIST_LIMIT: usize = 100;
const FAIL_SYNC_OUTBOX_WRITE_ENV: &str = "AGENTA_TEST_FAIL_SYNC_OUTBOX_WRITE";

impl SqliteStore {
    pub async fn list_sync_outbox_for_delivery(
        &self,
        remote_id: &str,
        limit: Option<usize>,
    ) -> AppResult<Vec<SyncOutboxEntry>> {
        let limit = limit
            .unwrap_or(DEFAULT_OUTBOX_LIST_LIMIT)
            .clamp(1, MAX_OUTBOX_LIST_LIMIT) as i64;
        let rows = query(
            r#"
            SELECT
                mutation_id, remote_id, entity_kind, local_id, operation,
                local_version, payload_json, status, attempt_count,
                last_attempt_at, acked_at, last_error, created_at
            FROM sync_outbox
            WHERE remote_id = ?
              AND status IN (?, ?)
            ORDER BY created_at ASC, mutation_id ASC
            LIMIT ?
            "#,
        )
        .bind(remote_id)
        .bind(SyncOutboxStatus::Pending.to_string())
        .bind(SyncOutboxStatus::Failed.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_sync_outbox_entry).collect()
    }

    pub async fn get_sync_entity(
        &self,
        entity_kind: SyncEntityKind,
        local_id: Uuid,
    ) -> AppResult<Option<SyncEntityState>> {
        let row = query(
            r#"
            SELECT
                entity_kind, local_id, remote_id, remote_entity_id, local_version, dirty,
                last_synced_at, last_enqueued_mutation_id, updated_at
            FROM sync_entities
            WHERE entity_kind = ? AND local_id = ?
            "#,
        )
        .bind(entity_kind.to_string())
        .bind(local_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_sync_entity).transpose()
    }

    pub async fn pending_sync_outbox_count(&self, remote_id: &str) -> AppResult<i64> {
        let row = query(
            r#"
            SELECT COUNT(*) AS pending_count
            FROM sync_outbox
            WHERE remote_id = ? AND status = ?
            "#,
        )
        .bind(remote_id)
        .bind(SyncOutboxStatus::Pending.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("pending_count"))
    }

    pub async fn oldest_pending_sync_outbox_at(
        &self,
        remote_id: &str,
    ) -> AppResult<Option<OffsetDateTime>> {
        let row = query(
            r#"
            SELECT MIN(created_at) AS oldest_pending_at
            FROM sync_outbox
            WHERE remote_id = ? AND status = ?
            "#,
        )
        .bind(remote_id)
        .bind(SyncOutboxStatus::Pending.to_string())
        .fetch_one(&self.pool)
        .await?;

        row.get::<Option<String>, _>("oldest_pending_at")
            .map(|value| parse_time(value, "oldest_pending_at"))
            .transpose()
    }

    pub async fn list_sync_outbox(&self, limit: Option<usize>) -> AppResult<Vec<SyncOutboxEntry>> {
        let limit = limit
            .unwrap_or(DEFAULT_OUTBOX_LIST_LIMIT)
            .clamp(1, MAX_OUTBOX_LIST_LIMIT) as i64;
        let rows = query(
            r#"
            SELECT
                mutation_id, remote_id, entity_kind, local_id, operation,
                local_version, payload_json, status, attempt_count,
                last_attempt_at, acked_at, last_error, created_at
            FROM sync_outbox
            ORDER BY created_at DESC, mutation_id DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_sync_outbox_entry).collect()
    }

    pub async fn get_sync_checkpoint(
        &self,
        remote_id: &str,
        checkpoint_kind: SyncCheckpointKind,
    ) -> AppResult<Option<SyncCheckpoint>> {
        let row = query(
            r#"
            SELECT remote_id, checkpoint_kind, checkpoint_value, updated_at
            FROM sync_checkpoints
            WHERE remote_id = ? AND checkpoint_kind = ?
            "#,
        )
        .bind(remote_id)
        .bind(checkpoint_kind.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_sync_checkpoint).transpose()
    }

    pub async fn list_sync_tombstones(&self, remote_id: &str) -> AppResult<Vec<SyncTombstone>> {
        let rows = query(
            r#"
            SELECT entity_kind, local_id, remote_id, remote_entity_id, deleted_at, purge_after
            FROM sync_tombstones
            WHERE remote_id = ?
            ORDER BY deleted_at DESC, local_id DESC
            "#,
        )
        .bind(remote_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_sync_tombstone).collect()
    }

    pub async fn upsert_sync_checkpoint(
        &self,
        remote_id: &str,
        checkpoint_kind: SyncCheckpointKind,
        checkpoint_value: &str,
        updated_at: OffsetDateTime,
    ) -> AppResult<()> {
        query(
            r#"
            INSERT INTO sync_checkpoints (
                remote_id, checkpoint_kind, checkpoint_value, updated_at
            ) VALUES (?, ?, ?, ?)
            ON CONFLICT(remote_id, checkpoint_kind) DO UPDATE SET
                checkpoint_value = excluded.checkpoint_value,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(remote_id)
        .bind(checkpoint_kind.to_string())
        .bind(checkpoint_value)
        .bind(format_time(updated_at)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_sync_outbox_acked(
        &self,
        mutation_id: Uuid,
        acked_at: OffsetDateTime,
    ) -> AppResult<()> {
        query(
            r#"
            UPDATE sync_outbox
            SET
                status = ?,
                attempt_count = attempt_count + 1,
                last_attempt_at = ?,
                acked_at = ?,
                last_error = NULL
            WHERE mutation_id = ?
            "#,
        )
        .bind(SyncOutboxStatus::Acked.to_string())
        .bind(format_time(acked_at)?)
        .bind(format_time(acked_at)?)
        .bind(mutation_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_sync_outbox_failed(
        &self,
        mutation_id: Uuid,
        failed_at: OffsetDateTime,
        last_error: &str,
    ) -> AppResult<()> {
        query(
            r#"
            UPDATE sync_outbox
            SET
                status = ?,
                attempt_count = attempt_count + 1,
                last_attempt_at = ?,
                last_error = ?
            WHERE mutation_id = ?
            "#,
        )
        .bind(SyncOutboxStatus::Failed.to_string())
        .bind(format_time(failed_at)?)
        .bind(last_error)
        .bind(mutation_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_sync_entity_acked(
        &self,
        entity_kind: SyncEntityKind,
        local_id: Uuid,
        remote_id: &str,
        remote_entity_id: &str,
        last_enqueued_mutation_id: Uuid,
        acked_at: OffsetDateTime,
    ) -> AppResult<()> {
        query(
            r#"
            UPDATE sync_entities
            SET
                remote_id = ?,
                remote_entity_id = ?,
                dirty = 0,
                last_synced_at = ?,
                last_enqueued_mutation_id = ?
            WHERE entity_kind = ? AND local_id = ?
            "#,
        )
        .bind(remote_id)
        .bind(remote_entity_id)
        .bind(format_time(acked_at)?)
        .bind(last_enqueued_mutation_id.to_string())
        .bind(entity_kind.to_string())
        .bind(local_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_synced_entity_state(
        &self,
        entity_kind: SyncEntityKind,
        local_id: Uuid,
        remote_id: &str,
        remote_entity_id: &str,
        local_version: i64,
        synced_at: OffsetDateTime,
    ) -> AppResult<()> {
        query(
            r#"
            INSERT INTO sync_entities (
                entity_kind,
                local_id,
                remote_id,
                remote_entity_id,
                local_version,
                dirty,
                last_synced_at,
                last_enqueued_mutation_id,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(entity_kind, local_id) DO UPDATE SET
                remote_id = excluded.remote_id,
                remote_entity_id = excluded.remote_entity_id,
                local_version = excluded.local_version,
                dirty = excluded.dirty,
                last_synced_at = excluded.last_synced_at,
                last_enqueued_mutation_id = excluded.last_enqueued_mutation_id,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(entity_kind.to_string())
        .bind(local_id.to_string())
        .bind(remote_id)
        .bind(remote_entity_id)
        .bind(local_version)
        .bind(0_i64)
        .bind(format_time(synced_at)?)
        .bind(Option::<String>::None)
        .bind(format_time(synced_at)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_synced_entity_state_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        entity_kind: SyncEntityKind,
        local_id: Uuid,
        remote_id: &str,
        remote_entity_id: &str,
        local_version: i64,
        synced_at: OffsetDateTime,
    ) -> AppResult<()> {
        query(
            r#"
            INSERT INTO sync_entities (
                entity_kind,
                local_id,
                remote_id,
                remote_entity_id,
                local_version,
                dirty,
                last_synced_at,
                last_enqueued_mutation_id,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(entity_kind, local_id) DO UPDATE SET
                remote_id = excluded.remote_id,
                remote_entity_id = excluded.remote_entity_id,
                local_version = excluded.local_version,
                dirty = excluded.dirty,
                last_synced_at = excluded.last_synced_at,
                last_enqueued_mutation_id = excluded.last_enqueued_mutation_id,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(entity_kind.to_string())
        .bind(local_id.to_string())
        .bind(remote_id)
        .bind(remote_entity_id)
        .bind(local_version)
        .bind(0_i64)
        .bind(format_time(synced_at)?)
        .bind(Option::<String>::None)
        .bind(format_time(synced_at)?)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn record_sync_mutation_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        remote_id: &str,
        entity_kind: SyncEntityKind,
        local_id: Uuid,
        operation: SyncOperation,
        payload_json: &Value,
        updated_at: OffsetDateTime,
    ) -> AppResult<SyncOutboxEntry> {
        let current_version = query(
            r#"
            SELECT local_version
            FROM sync_entities
            WHERE entity_kind = ? AND local_id = ?
            "#,
        )
        .bind(entity_kind.to_string())
        .bind(local_id.to_string())
        .fetch_optional(&mut **tx)
        .await?
        .map(|row| row.get::<i64, _>("local_version"))
        .unwrap_or(0);
        let local_version = current_version + 1;
        let mutation_id = Uuid::new_v4();

        query(
            r#"
            INSERT INTO sync_entities (
                entity_kind,
                local_id,
                remote_id,
                remote_entity_id,
                local_version,
                dirty,
                last_synced_at,
                last_enqueued_mutation_id,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(entity_kind, local_id) DO UPDATE SET
                remote_id = excluded.remote_id,
                local_version = excluded.local_version,
                dirty = excluded.dirty,
                last_enqueued_mutation_id = excluded.last_enqueued_mutation_id,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(entity_kind.to_string())
        .bind(local_id.to_string())
        .bind(remote_id)
        .bind(Option::<String>::None)
        .bind(local_version)
        .bind(1_i64)
        .bind(Option::<String>::None)
        .bind(Some(mutation_id.to_string()))
        .bind(format_time(updated_at)?)
        .execute(&mut **tx)
        .await?;

        maybe_fail_sync_outbox_write()?;

        query(
            r#"
            INSERT INTO sync_outbox (
                mutation_id,
                remote_id,
                entity_kind,
                local_id,
                operation,
                local_version,
                payload_json,
                status,
                attempt_count,
                last_attempt_at,
                acked_at,
                last_error,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(mutation_id.to_string())
        .bind(remote_id)
        .bind(entity_kind.to_string())
        .bind(local_id.to_string())
        .bind(operation.to_string())
        .bind(local_version)
        .bind(payload_json.to_string())
        .bind(SyncOutboxStatus::Pending.to_string())
        .bind(0_i64)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(format_time(updated_at)?)
        .execute(&mut **tx)
        .await?;

        Ok(SyncOutboxEntry {
            mutation_id,
            remote_id: remote_id.to_string(),
            entity_kind,
            local_id,
            operation,
            local_version,
            payload_json: payload_json.clone(),
            status: SyncOutboxStatus::Pending,
            attempt_count: 0,
            last_attempt_at: None,
            acked_at: None,
            last_error: None,
            created_at: updated_at,
        })
    }
}

fn maybe_fail_sync_outbox_write() -> AppResult<()> {
    if env::var_os(FAIL_SYNC_OUTBOX_WRITE_ENV).is_some() {
        return Err(AppError::Internal(
            "forced sync outbox write failure for testing".to_string(),
        ));
    }
    Ok(())
}
