use sqlx::{query, QueryBuilder, Row, Sqlite, Transaction};
use uuid::Uuid;

use time::OffsetDateTime;

use crate::domain::{KnowledgeStatus, Task, TaskActivity, TaskActivityKind};
use crate::error::{AppError, AppResult};
use crate::search::{build_task_vector_document_text, SearchVectorJob, TaskVectorDocument};

use super::mapping::{format_time, map_activity, map_task, parse_time};
use super::{ActivityLexicalSearchRow, SqliteStore, TaskLexicalSearchRow, TaskListFilter};

impl SqliteStore {
    pub async fn list_task_ids(&self) -> AppResult<Vec<Uuid>> {
        let rows = query(
            r#"
            SELECT task_id
            FROM tasks
            ORDER BY updated_at ASC, task_id ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                crate::storage::mapping::parse_uuid(
                    row.get::<String, _>("task_id"),
                    "tasks.task_id",
                )
            })
            .collect()
    }

    pub async fn list_task_ids_by_project_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        project_id: Uuid,
    ) -> AppResult<Vec<Uuid>> {
        let rows = query(
            r#"
            SELECT task_id
            FROM tasks
            WHERE project_id = ?
            ORDER BY updated_at ASC, task_id ASC
            "#,
        )
        .bind(project_id.to_string())
        .fetch_all(&mut **tx)
        .await?;

        rows.into_iter()
            .map(|row| {
                crate::storage::mapping::parse_uuid(
                    row.get::<String, _>("task_id"),
                    "tasks.task_id",
                )
            })
            .collect()
    }

    pub async fn list_task_ids_by_version_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        version_id: Uuid,
    ) -> AppResult<Vec<Uuid>> {
        let rows = query(
            r#"
            SELECT task_id
            FROM tasks
            WHERE version_id = ?
            ORDER BY updated_at ASC, task_id ASC
            "#,
        )
        .bind(version_id.to_string())
        .fetch_all(&mut **tx)
        .await?;

        rows.into_iter()
            .map(|row| {
                crate::storage::mapping::parse_uuid(
                    row.get::<String, _>("task_id"),
                    "tasks.task_id",
                )
            })
            .collect()
    }

    pub async fn insert_task(&self, task: &Task) -> AppResult<()> {
        query(
            r#"
            INSERT INTO tasks (
                task_id,
                project_id,
                version_id,
                task_code,
                task_kind,
                title,
                summary,
                description,
                task_search_summary,
                task_context_digest,
                latest_note_summary,
                knowledge_status,
                status,
                priority,
                created_by,
                updated_by,
                created_at,
                updated_at,
                closed_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(task.task_id.to_string())
        .bind(task.project_id.to_string())
        .bind(task.version_id.map(|value| value.to_string()))
        .bind(&task.task_code)
        .bind(task.task_kind.to_string())
        .bind(&task.title)
        .bind(&task.summary)
        .bind(&task.description)
        .bind(&task.task_search_summary)
        .bind(&task.task_context_digest)
        .bind(&task.latest_note_summary)
        .bind(task.knowledge_status.to_string())
        .bind(task.status.to_string())
        .bind(task.priority.to_string())
        .bind(&task.created_by)
        .bind(&task.updated_by)
        .bind(format_time(task.created_at)?)
        .bind(format_time(task.updated_at)?)
        .bind(task.closed_at.map(format_time).transpose()?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_task_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task: &Task,
    ) -> AppResult<()> {
        query(
            r#"
            INSERT INTO tasks (
                task_id,
                project_id,
                version_id,
                task_code,
                task_kind,
                title,
                summary,
                description,
                task_search_summary,
                task_context_digest,
                latest_note_summary,
                knowledge_status,
                status,
                priority,
                created_by,
                updated_by,
                created_at,
                updated_at,
                closed_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(task.task_id.to_string())
        .bind(task.project_id.to_string())
        .bind(task.version_id.map(|value| value.to_string()))
        .bind(&task.task_code)
        .bind(task.task_kind.to_string())
        .bind(&task.title)
        .bind(&task.summary)
        .bind(&task.description)
        .bind(&task.task_search_summary)
        .bind(&task.task_context_digest)
        .bind(&task.latest_note_summary)
        .bind(task.knowledge_status.to_string())
        .bind(task.status.to_string())
        .bind(task.priority.to_string())
        .bind(&task.created_by)
        .bind(&task.updated_by)
        .bind(format_time(task.created_at)?)
        .bind(format_time(task.updated_at)?)
        .bind(task.closed_at.map(format_time).transpose()?)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn get_task_by_ref(&self, reference: &str) -> AppResult<Task> {
        let row = query(
            r#"
            SELECT
                task_id, project_id, version_id, task_code, task_kind, title, summary, description,
                task_search_summary, task_context_digest, latest_note_summary, knowledge_status, status, priority,
                created_by, updated_by, created_at, updated_at, closed_at
            FROM tasks
            WHERE task_id = ?
            "#,
        )
        .bind(reference)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_task)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "task".to_string(),
                reference: reference.to_string(),
            })
    }

    pub async fn get_task_by_ref_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        reference: &str,
    ) -> AppResult<Task> {
        let row = query(
            r#"
            SELECT
                task_id, project_id, version_id, task_code, task_kind, title, summary, description,
                task_search_summary, task_context_digest, latest_note_summary, knowledge_status, status, priority,
                created_by, updated_by, created_at, updated_at, closed_at
            FROM tasks
            WHERE task_id = ?
            "#,
        )
        .bind(reference)
        .fetch_optional(&mut **tx)
        .await?;

        row.map(map_task)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "task".to_string(),
                reference: reference.to_string(),
            })
    }

    pub async fn get_task_with_stats_by_ref(
        &self,
        reference: &str,
    ) -> AppResult<(Task, i64, i64, OffsetDateTime, Option<Uuid>, i64, i64, i64)> {
        let row = query(
            r#"
            SELECT
                t.task_id, t.project_id, t.version_id, t.task_code, t.task_kind, t.title, t.summary, t.description,
                t.task_search_summary, t.task_context_digest, t.latest_note_summary, t.knowledge_status, t.status, t.priority,
                t.created_by, t.updated_by, t.created_at, t.updated_at, t.closed_at,
                (
                    SELECT source_task_id
                    FROM task_relations tr
                    WHERE tr.kind = ? AND tr.status = ? AND tr.target_task_id = t.task_id
                    ORDER BY tr.created_at DESC, tr.relation_id DESC
                    LIMIT 1
                ) AS parent_task_id,
                (
                    SELECT COUNT(*)
                    FROM task_relations tr
                    WHERE tr.kind = ? AND tr.status = ? AND tr.source_task_id = t.task_id
                ) AS child_count,
                (
                    SELECT COUNT(*)
                    FROM task_activities ta
                    WHERE ta.task_id = t.task_id AND ta.kind = ?
                ) AS note_count,
                (
                    SELECT COUNT(*)
                    FROM attachments a
                    WHERE a.task_id = t.task_id
                ) AS attachment_count,
                max(
                    t.updated_at,
                    COALESCE(
                        (
                            SELECT MAX(ta.created_at)
                            FROM task_activities ta
                            WHERE ta.task_id = t.task_id
                        ),
                        t.updated_at
                    )
                ) AS latest_activity_at,
                (
                    SELECT COUNT(*)
                    FROM task_relations tr
                    JOIN tasks blocker ON blocker.task_id = tr.source_task_id
                    WHERE tr.kind = ? AND tr.status = ? AND tr.target_task_id = t.task_id
                      AND blocker.status NOT IN (?, ?)
                ) AS open_blocker_count,
                (
                    SELECT COUNT(*)
                    FROM task_relations tr
                    WHERE tr.kind = ? AND tr.status = ? AND tr.source_task_id = t.task_id
                ) AS blocking_count
            FROM tasks t
            WHERE t.task_id = ?
            "#,
        )
        .bind(crate::domain::TaskRelationKind::ParentChild.to_string())
        .bind(crate::domain::TaskRelationStatus::Active.to_string())
        .bind(crate::domain::TaskRelationKind::ParentChild.to_string())
        .bind(crate::domain::TaskRelationStatus::Active.to_string())
        .bind(TaskActivityKind::Note.to_string())
        .bind(crate::domain::TaskRelationKind::Blocks.to_string())
        .bind(crate::domain::TaskRelationStatus::Active.to_string())
        .bind(crate::domain::TaskStatus::Done.to_string())
        .bind(crate::domain::TaskStatus::Cancelled.to_string())
        .bind(crate::domain::TaskRelationKind::Blocks.to_string())
        .bind(crate::domain::TaskRelationStatus::Active.to_string())
        .bind(reference)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_task_with_stats)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "task".to_string(),
                reference: reference.to_string(),
            })
    }

    pub async fn get_task_with_stats_by_ref_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        reference: &str,
    ) -> AppResult<(Task, i64, i64, OffsetDateTime, Option<Uuid>, i64, i64, i64)> {
        let row = query(
            r#"
            SELECT
                t.task_id, t.project_id, t.version_id, t.task_code, t.task_kind, t.title, t.summary, t.description,
                t.task_search_summary, t.task_context_digest, t.latest_note_summary, t.knowledge_status, t.status, t.priority,
                t.created_by, t.updated_by, t.created_at, t.updated_at, t.closed_at,
                (
                    SELECT source_task_id
                    FROM task_relations tr
                    WHERE tr.kind = ? AND tr.status = ? AND tr.target_task_id = t.task_id
                    ORDER BY tr.created_at DESC, tr.relation_id DESC
                    LIMIT 1
                ) AS parent_task_id,
                (
                    SELECT COUNT(*)
                    FROM task_relations tr
                    WHERE tr.kind = ? AND tr.status = ? AND tr.source_task_id = t.task_id
                ) AS child_count,
                (
                    SELECT COUNT(*)
                    FROM task_activities ta
                    WHERE ta.task_id = t.task_id AND ta.kind = ?
                ) AS note_count,
                (
                    SELECT COUNT(*)
                    FROM attachments a
                    WHERE a.task_id = t.task_id
                ) AS attachment_count,
                max(
                    t.updated_at,
                    COALESCE(
                        (
                            SELECT MAX(ta.created_at)
                            FROM task_activities ta
                            WHERE ta.task_id = t.task_id
                        ),
                        t.updated_at
                    )
                ) AS latest_activity_at,
                (
                    SELECT COUNT(*)
                    FROM task_relations tr
                    JOIN tasks blocker ON blocker.task_id = tr.source_task_id
                    WHERE tr.kind = ? AND tr.status = ? AND tr.target_task_id = t.task_id
                      AND blocker.status NOT IN (?, ?)
                ) AS open_blocker_count,
                (
                    SELECT COUNT(*)
                    FROM task_relations tr
                    WHERE tr.kind = ? AND tr.status = ? AND tr.source_task_id = t.task_id
                ) AS blocking_count
            FROM tasks t
            WHERE t.task_id = ?
            "#,
        )
        .bind(crate::domain::TaskRelationKind::ParentChild.to_string())
        .bind(crate::domain::TaskRelationStatus::Active.to_string())
        .bind(crate::domain::TaskRelationKind::ParentChild.to_string())
        .bind(crate::domain::TaskRelationStatus::Active.to_string())
        .bind(TaskActivityKind::Note.to_string())
        .bind(crate::domain::TaskRelationKind::Blocks.to_string())
        .bind(crate::domain::TaskRelationStatus::Active.to_string())
        .bind(crate::domain::TaskStatus::Done.to_string())
        .bind(crate::domain::TaskStatus::Cancelled.to_string())
        .bind(crate::domain::TaskRelationKind::Blocks.to_string())
        .bind(crate::domain::TaskRelationStatus::Active.to_string())
        .bind(reference)
        .fetch_optional(&mut **tx)
        .await?;

        row.map(map_task_with_stats)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "task".to_string(),
                reference: reference.to_string(),
            })
    }

    pub async fn list_tasks(&self, filter: TaskListFilter) -> AppResult<Vec<Task>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                task_id, project_id, version_id, task_code, task_kind, title, summary, description,
                task_search_summary, task_context_digest, latest_note_summary, knowledge_status, status, priority,
                created_by, updated_by, created_at, updated_at, closed_at
            FROM tasks
            WHERE 1 = 1
            "#,
        );
        if let Some(project_id) = filter.project_id {
            builder
                .push(" AND project_id = ")
                .push_bind(project_id.to_string());
        }
        if let Some(version_id) = filter.version_id {
            builder
                .push(" AND version_id = ")
                .push_bind(version_id.to_string());
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(task_kind) = filter.task_kind {
            builder
                .push(" AND task_kind = ")
                .push_bind(task_kind.to_string());
        }
        if let Some(task_code_prefix) = filter.task_code_prefix {
            builder
                .push(" AND task_code LIKE ")
                .push_bind(format!("{task_code_prefix}%"));
        }
        if let Some(title_prefix) = filter.title_prefix {
            builder
                .push(" AND title LIKE ")
                .push_bind(format!("{title_prefix}%"));
        }
        builder.push(" ORDER BY created_at DESC, task_id DESC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter().map(map_task).collect()
    }

    pub async fn list_tasks_with_stats(
        &self,
        filter: TaskListFilter,
    ) -> AppResult<Vec<(Task, i64, i64, OffsetDateTime, Option<Uuid>, i64, i64, i64)>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                t.task_id, t.project_id, t.version_id, t.task_code, t.task_kind, t.title, t.summary, t.description,
                t.task_search_summary, t.task_context_digest, t.latest_note_summary, t.knowledge_status, t.status, t.priority,
                t.created_by, t.updated_by, t.created_at, t.updated_at, t.closed_at,
                (
                    SELECT source_task_id
                    FROM task_relations tr
                    WHERE tr.kind = 
            "#,
        );
        builder.push_bind(crate::domain::TaskRelationKind::ParentChild.to_string());
        builder.push(
            r#"
                      AND tr.status = 
            "#,
        );
        builder.push_bind(crate::domain::TaskRelationStatus::Active.to_string());
        builder.push(
            r#"
                      AND tr.target_task_id = t.task_id
                    ORDER BY tr.created_at DESC, tr.relation_id DESC
                    LIMIT 1
                ) AS parent_task_id,
                (
                    SELECT COUNT(*)
                    FROM task_relations tr
                    WHERE tr.kind = 
            "#,
        );
        builder.push_bind(crate::domain::TaskRelationKind::ParentChild.to_string());
        builder.push(
            r#"
                      AND tr.status = 
            "#,
        );
        builder.push_bind(crate::domain::TaskRelationStatus::Active.to_string());
        builder.push(
            r#"
                      AND tr.source_task_id = t.task_id
                ) AS child_count,
                (
                    SELECT COUNT(*)
                    FROM task_activities ta
                    WHERE ta.task_id = t.task_id AND ta.kind = 
            "#,
        );
        builder.push_bind(TaskActivityKind::Note.to_string());
        builder.push(
            r#"
                ) AS note_count,
                (
                    SELECT COUNT(*)
                    FROM attachments a
                    WHERE a.task_id = t.task_id
                ) AS attachment_count,
                max(
                    t.updated_at,
                    COALESCE(
                        (
                            SELECT MAX(ta.created_at)
                            FROM task_activities ta
                            WHERE ta.task_id = t.task_id
                        ),
                        t.updated_at
                    )
                ) AS latest_activity_at,
                (
                    SELECT COUNT(*)
                    FROM task_relations tr
                    JOIN tasks blocker ON blocker.task_id = tr.source_task_id
                    WHERE tr.kind = 
            "#,
        );
        builder.push_bind(crate::domain::TaskRelationKind::Blocks.to_string());
        builder.push(
            r#"
                      AND tr.status = 
            "#,
        );
        builder.push_bind(crate::domain::TaskRelationStatus::Active.to_string());
        builder.push(
            r#"
                      AND tr.target_task_id = t.task_id
                      AND blocker.status NOT IN (
            "#,
        );
        builder.push_bind(crate::domain::TaskStatus::Done.to_string());
        builder.push(", ");
        builder.push_bind(crate::domain::TaskStatus::Cancelled.to_string());
        builder.push(
            r#"
                      )
                ) AS open_blocker_count,
                (
                    SELECT COUNT(*)
                    FROM task_relations tr
                    WHERE tr.kind = 
            "#,
        );
        builder.push_bind(crate::domain::TaskRelationKind::Blocks.to_string());
        builder.push(
            r#"
                      AND tr.status = 
            "#,
        );
        builder.push_bind(crate::domain::TaskRelationStatus::Active.to_string());
        builder.push(
            r#"
                      AND tr.source_task_id = t.task_id
                ) AS blocking_count
            FROM tasks t
            WHERE 1 = 1
            "#,
        );
        if let Some(project_id) = filter.project_id {
            builder
                .push(" AND t.project_id = ")
                .push_bind(project_id.to_string());
        }
        if let Some(version_id) = filter.version_id {
            builder
                .push(" AND t.version_id = ")
                .push_bind(version_id.to_string());
        }
        if let Some(status) = filter.status {
            builder
                .push(" AND t.status = ")
                .push_bind(status.to_string());
        }
        if let Some(task_kind) = filter.task_kind {
            builder
                .push(" AND t.task_kind = ")
                .push_bind(task_kind.to_string());
        }
        if let Some(task_code_prefix) = filter.task_code_prefix {
            builder
                .push(" AND t.task_code LIKE ")
                .push_bind(format!("{task_code_prefix}%"));
        }
        if let Some(title_prefix) = filter.title_prefix {
            builder
                .push(" AND t.title LIKE ")
                .push_bind(format!("{title_prefix}%"));
        }
        builder.push(" ORDER BY t.created_at DESC, t.task_id DESC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter().map(map_task_with_stats).collect()
    }

    pub async fn update_task(&self, task: &Task) -> AppResult<()> {
        query(
            r#"
            UPDATE tasks
            SET
                version_id = ?,
                task_code = ?,
                task_kind = ?,
                title = ?,
                summary = ?,
                description = ?,
                task_search_summary = ?,
                task_context_digest = ?,
                latest_note_summary = ?,
                knowledge_status = ?,
                status = ?,
                priority = ?,
                updated_by = ?,
                updated_at = ?,
                closed_at = ?
            WHERE task_id = ?
            "#,
        )
        .bind(task.version_id.map(|value| value.to_string()))
        .bind(&task.task_code)
        .bind(task.task_kind.to_string())
        .bind(&task.title)
        .bind(&task.summary)
        .bind(&task.description)
        .bind(&task.task_search_summary)
        .bind(&task.task_context_digest)
        .bind(&task.latest_note_summary)
        .bind(task.knowledge_status.to_string())
        .bind(task.status.to_string())
        .bind(task.priority.to_string())
        .bind(&task.updated_by)
        .bind(format_time(task.updated_at)?)
        .bind(task.closed_at.map(format_time).transpose()?)
        .bind(task.task_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_task_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task: &Task,
    ) -> AppResult<()> {
        query(
            r#"
            UPDATE tasks
            SET
                version_id = ?,
                task_code = ?,
                task_kind = ?,
                title = ?,
                summary = ?,
                description = ?,
                task_search_summary = ?,
                task_context_digest = ?,
                latest_note_summary = ?,
                knowledge_status = ?,
                status = ?,
                priority = ?,
                updated_by = ?,
                updated_at = ?,
                closed_at = ?
            WHERE task_id = ?
            "#,
        )
        .bind(task.version_id.map(|value| value.to_string()))
        .bind(&task.task_code)
        .bind(task.task_kind.to_string())
        .bind(&task.title)
        .bind(&task.summary)
        .bind(&task.description)
        .bind(&task.task_search_summary)
        .bind(&task.task_context_digest)
        .bind(&task.latest_note_summary)
        .bind(task.knowledge_status.to_string())
        .bind(task.status.to_string())
        .bind(task.priority.to_string())
        .bind(&task.updated_by)
        .bind(format_time(task.updated_at)?)
        .bind(task.closed_at.map(format_time).transpose()?)
        .bind(task.task_id.to_string())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn update_task_context_digest_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
        task_context_digest: &str,
    ) -> AppResult<()> {
        query(
            r#"
            UPDATE tasks
            SET task_context_digest = ?
            WHERE task_id = ?
            "#,
        )
        .bind(task_context_digest)
        .bind(task_id.to_string())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn update_task_note_rollup_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
        latest_note_summary: Option<&str>,
        knowledge_status: KnowledgeStatus,
    ) -> AppResult<()> {
        query(
            r#"
            UPDATE tasks
            SET latest_note_summary = ?, knowledge_status = ?
            WHERE task_id = ?
            "#,
        )
        .bind(latest_note_summary)
        .bind(knowledge_status.to_string())
        .bind(task_id.to_string())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn insert_activity(&self, activity: &TaskActivity) -> AppResult<()> {
        query(
            r#"
            INSERT INTO task_activities (
                activity_id, task_id, kind, content, activity_search_summary, created_by, created_at, metadata_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(activity.activity_id.to_string())
        .bind(activity.task_id.to_string())
        .bind(activity.kind.to_string())
        .bind(&activity.content)
        .bind(&activity.activity_search_summary)
        .bind(&activity.created_by)
        .bind(format_time(activity.created_at)?)
        .bind(activity.metadata_json.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_activity_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        activity: &TaskActivity,
    ) -> AppResult<()> {
        query(
            r#"
            INSERT INTO task_activities (
                activity_id,
                task_id,
                kind,
                content,
                activity_search_summary,
                created_by,
                created_at,
                metadata_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(activity.activity_id.to_string())
        .bind(activity.task_id.to_string())
        .bind(activity.kind.to_string())
        .bind(&activity.content)
        .bind(&activity.activity_search_summary)
        .bind(&activity.created_by)
        .bind(format_time(activity.created_at)?)
        .bind(activity.metadata_json.to_string())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn list_task_activities(&self, task_id: Uuid) -> AppResult<Vec<TaskActivity>> {
        let rows = query(
            r#"
            SELECT activity_id, task_id, kind, content, activity_search_summary, created_by, created_at, metadata_json
            FROM task_activities
            WHERE task_id = ?
            ORDER BY created_at DESC, activity_id DESC
            "#,
        )
        .bind(task_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_activity).collect()
    }

    pub async fn search_tasks(
        &self,
        filter: &TaskListFilter,
        query_text: &str,
        limit: usize,
    ) -> AppResult<Vec<TaskLexicalSearchRow>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                t.task_id,
                t.task_code,
                t.task_kind,
                t.title,
                t.status,
                t.priority,
                t.knowledge_status,
                t.task_search_summary,
                t.task_context_digest,
                t.latest_note_summary,
                bm25(tasks_fts, 8.0, 10.0, 1.0, 1.25, 0.75, 1.5) AS lexical_score,
                max(
                    t.updated_at,
                    COALESCE(
                        (
                            SELECT MAX(ta.created_at)
                            FROM task_activities ta
                            WHERE ta.task_id = t.task_id
                        ),
                        t.updated_at
                    )
                ) AS latest_activity_at
            FROM tasks_fts f
            JOIN tasks t ON t.rowid = f.rowid
            WHERE tasks_fts MATCH
            "#,
        );
        builder.push_bind(query_text);
        push_task_filter_predicates(&mut builder, filter);
        builder.push(" ORDER BY lexical_score ASC, latest_activity_at DESC, t.task_id ASC LIMIT ");
        builder.push_bind(limit as i64);

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter()
            .enumerate()
            .map(|(index, row)| {
                Ok(TaskLexicalSearchRow {
                    task_id: row.get::<String, _>("task_id"),
                    task_code: row.get::<Option<String>, _>("task_code"),
                    task_kind: row.get::<String, _>("task_kind"),
                    title: row.get::<String, _>("title"),
                    status: row.get::<String, _>("status"),
                    priority: row.get::<String, _>("priority"),
                    knowledge_status: row.get::<String, _>("knowledge_status"),
                    task_search_summary: row.get::<String, _>("task_search_summary"),
                    task_context_digest: row.get::<String, _>("task_context_digest"),
                    latest_note_summary: row.get::<Option<String>, _>("latest_note_summary"),
                    lexical_score: row.get::<f64, _>("lexical_score"),
                    lexical_rank: index,
                    latest_activity_at: parse_time(
                        row.get("latest_activity_at"),
                        "latest_activity_at",
                    )?,
                })
            })
            .collect()
    }

    pub async fn search_tasks_by_ids(
        &self,
        task_ids: &[String],
    ) -> AppResult<Vec<TaskLexicalSearchRow>> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                t.task_id,
                t.task_code,
                t.task_kind,
                t.title,
                t.status,
                t.priority,
                t.knowledge_status,
                t.task_search_summary,
                t.task_context_digest,
                t.latest_note_summary,
                0.0 AS lexical_score,
                max(
                    t.updated_at,
                    COALESCE(
                        (
                            SELECT MAX(ta.created_at)
                            FROM task_activities ta
                            WHERE ta.task_id = t.task_id
                        ),
                        t.updated_at
                    )
                ) AS latest_activity_at
            FROM tasks t
            WHERE t.task_id IN (
            "#,
        );
        let mut separated = builder.separated(", ");
        for task_id in task_ids {
            separated.push_bind(task_id);
        }
        builder.push(")");

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(TaskLexicalSearchRow {
                    task_id: row.get::<String, _>("task_id"),
                    task_code: row.get::<Option<String>, _>("task_code"),
                    task_kind: row.get::<String, _>("task_kind"),
                    title: row.get::<String, _>("title"),
                    status: row.get::<String, _>("status"),
                    priority: row.get::<String, _>("priority"),
                    knowledge_status: row.get::<String, _>("knowledge_status"),
                    task_search_summary: row.get::<String, _>("task_search_summary"),
                    task_context_digest: row.get::<String, _>("task_context_digest"),
                    latest_note_summary: row.get::<Option<String>, _>("latest_note_summary"),
                    lexical_score: 0.0,
                    lexical_rank: usize::MAX,
                    latest_activity_at: parse_time(
                        row.get("latest_activity_at"),
                        "latest_activity_at",
                    )?,
                })
            })
            .collect()
    }

    pub async fn search_activities(
        &self,
        filter: &TaskListFilter,
        query_text: &str,
        limit: usize,
    ) -> AppResult<Vec<ActivityLexicalSearchRow>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                a.activity_id,
                a.task_id,
                a.kind,
                a.activity_search_summary,
                bm25(task_activities_fts, 1.0) AS lexical_score
            FROM task_activities_fts f
            JOIN task_activities a ON a.rowid = f.rowid
            JOIN tasks t ON t.task_id = a.task_id
            WHERE task_activities_fts MATCH
            "#,
        );
        builder.push_bind(query_text);
        push_task_filter_predicates(&mut builder, filter);
        builder.push(" ORDER BY lexical_score ASC, a.created_at DESC, a.activity_id ASC LIMIT ");
        builder.push_bind(limit as i64);

        let rows = builder.build().fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| ActivityLexicalSearchRow {
                activity_id: row.get::<String, _>("activity_id"),
                task_id: row.get::<String, _>("task_id"),
                kind: row.get::<String, _>("kind"),
                summary: row.get::<String, _>("activity_search_summary"),
                score: row.get::<f64, _>("lexical_score"),
            })
            .collect())
    }

    pub async fn upsert_search_index_job_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        task_id: Uuid,
        now: OffsetDateTime,
    ) -> AppResult<()> {
        let now = format_time(now)?;
        query(
            r#"
            INSERT INTO search_index_jobs (
                task_id, job_kind, status, attempt_count, last_error, next_attempt_at, created_at, updated_at
            ) VALUES (?, 'task_vector_upsert', 'pending', 0, NULL, NULL, ?, ?)
            ON CONFLICT(task_id) DO UPDATE SET
                status = 'pending',
                attempt_count = 0,
                last_error = NULL,
                next_attempt_at = NULL,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(task_id.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn claim_next_search_index_job(
        &self,
        now: OffsetDateTime,
    ) -> AppResult<Option<SearchVectorJob>> {
        let mut tx = self.pool.begin().await?;
        let now_text = format_time(now)?;
        let row = query(
            r#"
            SELECT task_id, attempt_count
            FROM search_index_jobs
            WHERE status IN ('pending', 'failed')
              AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
            ORDER BY updated_at ASC, task_id ASC
            LIMIT 1
            "#,
        )
        .bind(&now_text)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };

        let task_id = crate::storage::mapping::parse_uuid(
            row.get::<String, _>("task_id"),
            "search_index_jobs.task_id",
        )?;
        let attempt_count = row.get::<i64, _>("attempt_count") + 1;
        query(
            r#"
            UPDATE search_index_jobs
            SET status = 'processing',
                attempt_count = ?,
                updated_at = ?
            WHERE task_id = ?
            "#,
        )
        .bind(attempt_count)
        .bind(&now_text)
        .bind(task_id.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(Some(SearchVectorJob {
            task_id,
            attempt_count,
        }))
    }

    pub async fn complete_search_index_job(&self, task_id: Uuid) -> AppResult<()> {
        query("DELETE FROM search_index_jobs WHERE task_id = ?")
            .bind(task_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn fail_search_index_job(
        &self,
        task_id: Uuid,
        error_message: &str,
        next_attempt_at: OffsetDateTime,
    ) -> AppResult<()> {
        query(
            r#"
            UPDATE search_index_jobs
            SET status = 'failed',
                last_error = ?,
                next_attempt_at = ?,
                updated_at = ?
            WHERE task_id = ?
            "#,
        )
        .bind(error_message)
        .bind(format_time(next_attempt_at)?)
        .bind(format_time(OffsetDateTime::now_utc())?)
        .bind(task_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn pending_search_index_job_count(&self) -> AppResult<usize> {
        let row = query("SELECT COUNT(*) AS count FROM search_index_jobs")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count").max(0) as usize)
    }

    pub async fn get_task_vector_document(
        &self,
        task_id: Uuid,
    ) -> AppResult<Option<TaskVectorDocument>> {
        let row = query(
            r#"
            SELECT
                t.task_id,
                t.project_id,
                p.slug AS project_slug,
                p.name AS project_name,
                p.description AS project_description,
                t.version_id,
                v.name AS version_name,
                v.description AS version_description,
                t.task_code,
                t.task_kind,
                t.title,
                t.status,
                t.priority,
                t.knowledge_status,
                t.latest_note_summary,
                (
                    SELECT ta.content
                    FROM task_activities ta
                    WHERE ta.task_id = t.task_id
                      AND ta.kind = 'attachment_ref'
                    ORDER BY ta.created_at DESC, ta.activity_id DESC
                    LIMIT 1
                ) AS latest_attachment_summary,
                t.task_search_summary,
                t.task_context_digest,
                t.updated_at
            FROM tasks t
            JOIN projects p ON p.project_id = t.project_id
            LEFT JOIN versions v ON v.version_id = t.version_id
            WHERE t.task_id = ?
            "#,
        )
        .bind(task_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };

        let task_code = row.get::<Option<String>, _>("task_code");
        let title = row.get::<String, _>("title");
        let latest_note_summary = row.get::<Option<String>, _>("latest_note_summary");
        let latest_attachment_summary = row.get::<Option<String>, _>("latest_attachment_summary");
        let task_search_summary = row.get::<String, _>("task_search_summary");
        let task_context_digest = row.get::<String, _>("task_context_digest");

        Ok(Some(TaskVectorDocument {
            task_id: row.get::<String, _>("task_id"),
            project_id: row.get::<String, _>("project_id"),
            project_slug: row.get::<String, _>("project_slug"),
            project_name: row.get::<String, _>("project_name"),
            project_description: row.get::<Option<String>, _>("project_description"),
            version_id: row.get::<Option<String>, _>("version_id"),
            version_name: row.get::<Option<String>, _>("version_name"),
            version_description: row.get::<Option<String>, _>("version_description"),
            task_code: task_code.clone(),
            task_kind: row.get::<String, _>("task_kind"),
            title: title.clone(),
            status: row.get::<String, _>("status"),
            priority: row.get::<String, _>("priority"),
            knowledge_status: row.get::<String, _>("knowledge_status"),
            latest_note_summary: latest_note_summary.clone(),
            latest_attachment_summary: latest_attachment_summary.clone(),
            task_search_summary: task_search_summary.clone(),
            task_context_digest: task_context_digest.clone(),
            updated_at: row.get::<String, _>("updated_at"),
            document: build_task_vector_document_text(
                &row.get::<String, _>("project_slug"),
                &row.get::<String, _>("project_name"),
                row.get::<Option<String>, _>("project_description")
                    .as_deref(),
                row.get::<Option<String>, _>("version_name").as_deref(),
                row.get::<Option<String>, _>("version_description")
                    .as_deref(),
                task_code.as_deref(),
                &title,
                latest_note_summary.as_deref(),
                latest_attachment_summary.as_deref(),
                &task_search_summary,
                &task_context_digest,
            ),
        }))
    }

    pub async fn task_count(&self) -> AppResult<i64> {
        let row = query("SELECT COUNT(*) AS count FROM tasks")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    pub async fn project_count(&self) -> AppResult<i64> {
        let row = query("SELECT COUNT(*) AS count FROM projects")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }
}

fn push_task_filter_predicates(builder: &mut QueryBuilder<'_, Sqlite>, filter: &TaskListFilter) {
    if let Some(project_id) = filter.project_id {
        builder
            .push(" AND t.project_id = ")
            .push_bind(project_id.to_string());
    }
    if let Some(version_id) = filter.version_id {
        builder
            .push(" AND t.version_id = ")
            .push_bind(version_id.to_string());
    }
    if let Some(status) = filter.status {
        builder
            .push(" AND t.status = ")
            .push_bind(status.to_string());
    }
    if let Some(task_kind) = filter.task_kind {
        builder
            .push(" AND t.task_kind = ")
            .push_bind(task_kind.to_string());
    }
    if let Some(task_code_prefix) = filter.task_code_prefix.as_deref() {
        builder
            .push(" AND t.task_code LIKE ")
            .push_bind(format!("{task_code_prefix}%"));
    }
    if let Some(title_prefix) = filter.title_prefix.as_deref() {
        builder
            .push(" AND t.title LIKE ")
            .push_bind(format!("{title_prefix}%"));
    }
}

fn map_task_with_stats(
    row: sqlx::sqlite::SqliteRow,
) -> AppResult<(Task, i64, i64, OffsetDateTime, Option<Uuid>, i64, i64, i64)> {
    let note_count = row.get::<i64, _>("note_count");
    let attachment_count = row.get::<i64, _>("attachment_count");
    let latest_activity_at = parse_time(row.get("latest_activity_at"), "latest_activity_at")?;
    let parent_task_id = row
        .get::<Option<String>, _>("parent_task_id")
        .map(|value| crate::storage::mapping::parse_uuid(value, "parent_task_id"))
        .transpose()?;
    let child_count = row.get::<i64, _>("child_count");
    let open_blocker_count = row.get::<i64, _>("open_blocker_count");
    let blocking_count = row.get::<i64, _>("blocking_count");
    let task = map_task(row)?;
    Ok((
        task,
        note_count,
        attachment_count,
        latest_activity_at,
        parent_task_id,
        child_count,
        open_blocker_count,
        blocking_count,
    ))
}
