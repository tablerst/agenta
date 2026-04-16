use sqlx::{query, QueryBuilder, Row, Sqlite, Transaction};
use uuid::Uuid;

use time::OffsetDateTime;

use crate::domain::{KnowledgeStatus, Task, TaskActivity, TaskActivityKind};
use crate::error::{AppError, AppResult};
use crate::search::{ActivitySearchHit, SearchResponse, TaskSearchHit};

use super::mapping::{format_time, map_activity, map_task, parse_time};
use super::{SqliteStore, TaskListFilter};

impl SqliteStore {
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

    pub async fn search(&self, query_text: &str, limit: usize) -> AppResult<SearchResponse> {
        let task_rows = query(
            r#"
            SELECT
                t.task_id,
                t.task_code,
                t.task_kind,
                t.title,
                t.status,
                t.priority,
                t.knowledge_status,
                COALESCE(t.latest_note_summary, t.task_search_summary) AS task_summary
            FROM tasks_fts f
            JOIN tasks t ON t.rowid = f.rowid
            WHERE tasks_fts MATCH ?
            ORDER BY bm25(tasks_fts)
            LIMIT ?
            "#,
        )
        .bind(query_text)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        let activity_rows = query(
            r#"
            SELECT a.activity_id, a.task_id, a.kind, a.activity_search_summary
            FROM task_activities_fts f
            JOIN task_activities a ON a.rowid = f.rowid
            WHERE task_activities_fts MATCH ?
            ORDER BY bm25(task_activities_fts)
            LIMIT ?
            "#,
        )
        .bind(query_text)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let tasks = task_rows
            .into_iter()
            .map(|row| TaskSearchHit {
                task_id: row.get::<String, _>("task_id"),
                task_code: row.get::<Option<String>, _>("task_code"),
                task_kind: row.get::<String, _>("task_kind"),
                title: row.get::<String, _>("title"),
                status: row.get::<String, _>("status"),
                priority: row.get::<String, _>("priority"),
                knowledge_status: row.get::<String, _>("knowledge_status"),
                summary: row.get::<String, _>("task_summary"),
            })
            .collect();
        let activities = activity_rows
            .into_iter()
            .map(|row| ActivitySearchHit {
                activity_id: row.get::<String, _>("activity_id"),
                task_id: row.get::<String, _>("task_id"),
                kind: row.get::<String, _>("kind"),
                summary: row.get::<String, _>("activity_search_summary"),
            })
            .collect();

        Ok(SearchResponse {
            query: Some(query_text.to_string()),
            tasks,
            activities,
        })
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
