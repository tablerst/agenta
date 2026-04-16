use sqlx::{query, Sqlite, Transaction};
use uuid::Uuid;

use crate::domain::{TaskRelation, TaskRelationKind, TaskRelationStatus};
use crate::error::{AppError, AppResult};

use super::mapping::{format_time, map_task_relation};
use super::SqliteStore;

impl SqliteStore {
    pub async fn insert_task_relation_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        relation: &TaskRelation,
    ) -> AppResult<()> {
        query(
            r#"
            INSERT INTO task_relations (
                relation_id,
                kind,
                source_task_id,
                target_task_id,
                status,
                created_by,
                updated_by,
                created_at,
                updated_at,
                resolved_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(relation.relation_id.to_string())
        .bind(relation.kind.to_string())
        .bind(relation.source_task_id.to_string())
        .bind(relation.target_task_id.to_string())
        .bind(relation.status.to_string())
        .bind(&relation.created_by)
        .bind(&relation.updated_by)
        .bind(format_time(relation.created_at)?)
        .bind(format_time(relation.updated_at)?)
        .bind(relation.resolved_at.map(format_time).transpose()?)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn update_task_relation_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        relation: &TaskRelation,
    ) -> AppResult<()> {
        query(
            r#"
            UPDATE task_relations
            SET
                status = ?,
                updated_by = ?,
                updated_at = ?,
                resolved_at = ?
            WHERE relation_id = ?
            "#,
        )
        .bind(relation.status.to_string())
        .bind(&relation.updated_by)
        .bind(format_time(relation.updated_at)?)
        .bind(relation.resolved_at.map(format_time).transpose()?)
        .bind(relation.relation_id.to_string())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn get_task_relation_by_ref(&self, reference: &str) -> AppResult<TaskRelation> {
        let row = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            WHERE relation_id = ?
            "#,
        )
        .bind(reference)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_task_relation)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "task_relation".to_string(),
                reference: reference.to_string(),
            })
    }

    pub async fn get_task_relation_by_ref_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        reference: &str,
    ) -> AppResult<TaskRelation> {
        let row = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            WHERE relation_id = ?
            "#,
        )
        .bind(reference)
        .fetch_optional(&mut **tx)
        .await?;

        row.map(map_task_relation)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "task_relation".to_string(),
                reference: reference.to_string(),
            })
    }

    pub async fn find_active_relation(
        &self,
        kind: TaskRelationKind,
        source_task_id: Uuid,
        target_task_id: Uuid,
    ) -> AppResult<Option<TaskRelation>> {
        let row = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            WHERE kind = ? AND source_task_id = ? AND target_task_id = ? AND status = ?
            "#,
        )
        .bind(kind.to_string())
        .bind(source_task_id.to_string())
        .bind(target_task_id.to_string())
        .bind(TaskRelationStatus::Active.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_task_relation).transpose()
    }

    pub async fn find_active_relation_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        kind: TaskRelationKind,
        source_task_id: Uuid,
        target_task_id: Uuid,
    ) -> AppResult<Option<TaskRelation>> {
        let row = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            WHERE kind = ? AND source_task_id = ? AND target_task_id = ? AND status = ?
            "#,
        )
        .bind(kind.to_string())
        .bind(source_task_id.to_string())
        .bind(target_task_id.to_string())
        .bind(TaskRelationStatus::Active.to_string())
        .fetch_optional(&mut **tx)
        .await?;

        row.map(map_task_relation).transpose()
    }

    pub async fn find_active_parent_relation(
        &self,
        task_id: Uuid,
    ) -> AppResult<Option<TaskRelation>> {
        let row = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            WHERE kind = ? AND target_task_id = ? AND status = ?
            ORDER BY created_at DESC, relation_id DESC
            LIMIT 1
            "#,
        )
        .bind(TaskRelationKind::ParentChild.to_string())
        .bind(task_id.to_string())
        .bind(TaskRelationStatus::Active.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_task_relation).transpose()
    }

    pub async fn find_active_parent_relation_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
    ) -> AppResult<Option<TaskRelation>> {
        let row = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            WHERE kind = ? AND target_task_id = ? AND status = ?
            ORDER BY created_at DESC, relation_id DESC
            LIMIT 1
            "#,
        )
        .bind(TaskRelationKind::ParentChild.to_string())
        .bind(task_id.to_string())
        .bind(TaskRelationStatus::Active.to_string())
        .fetch_optional(&mut **tx)
        .await?;

        row.map(map_task_relation).transpose()
    }

    pub async fn list_active_child_relations(
        &self,
        parent_task_id: Uuid,
    ) -> AppResult<Vec<TaskRelation>> {
        let rows = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            WHERE kind = ? AND source_task_id = ? AND status = ?
            ORDER BY created_at DESC, relation_id DESC
            "#,
        )
        .bind(TaskRelationKind::ParentChild.to_string())
        .bind(parent_task_id.to_string())
        .bind(TaskRelationStatus::Active.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_task_relation).collect()
    }

    pub async fn list_active_blocker_relations(
        &self,
        blocked_task_id: Uuid,
    ) -> AppResult<Vec<TaskRelation>> {
        let rows = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            WHERE kind = ? AND target_task_id = ? AND status = ?
            ORDER BY created_at DESC, relation_id DESC
            "#,
        )
        .bind(TaskRelationKind::Blocks.to_string())
        .bind(blocked_task_id.to_string())
        .bind(TaskRelationStatus::Active.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_task_relation).collect()
    }

    pub async fn list_active_blocking_relations(
        &self,
        blocker_task_id: Uuid,
    ) -> AppResult<Vec<TaskRelation>> {
        let rows = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            WHERE kind = ? AND source_task_id = ? AND status = ?
            ORDER BY created_at DESC, relation_id DESC
            "#,
        )
        .bind(TaskRelationKind::Blocks.to_string())
        .bind(blocker_task_id.to_string())
        .bind(TaskRelationStatus::Active.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_task_relation).collect()
    }

    pub async fn list_task_relations(&self) -> AppResult<Vec<TaskRelation>> {
        let rows = query(
            r#"
            SELECT
                relation_id, kind, source_task_id, target_task_id, status,
                created_by, updated_by, created_at, updated_at, resolved_at
            FROM task_relations
            ORDER BY created_at DESC, relation_id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_task_relation).collect()
    }

    pub async fn has_active_parent_path_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        from_task_id: Uuid,
        to_task_id: Uuid,
    ) -> AppResult<bool> {
        let row = query(
            r#"
            WITH RECURSIVE descendants(task_id) AS (
                SELECT target_task_id
                FROM task_relations
                WHERE kind = ? AND status = ? AND source_task_id = ?
                UNION
                SELECT tr.target_task_id
                FROM task_relations tr
                JOIN descendants d ON tr.source_task_id = d.task_id
                WHERE tr.kind = ? AND tr.status = ?
            )
            SELECT 1 AS found
            FROM descendants
            WHERE task_id = ?
            LIMIT 1
            "#,
        )
        .bind(TaskRelationKind::ParentChild.to_string())
        .bind(TaskRelationStatus::Active.to_string())
        .bind(from_task_id.to_string())
        .bind(TaskRelationKind::ParentChild.to_string())
        .bind(TaskRelationStatus::Active.to_string())
        .bind(to_task_id.to_string())
        .fetch_optional(&mut **tx)
        .await?;

        Ok(row.is_some())
    }
}
