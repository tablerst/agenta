use sqlx::{query, QueryBuilder, Row, Sqlite};
use uuid::Uuid;

use crate::domain::{Task, TaskActivity};
use crate::error::{AppError, AppResult};
use crate::search::{ActivitySearchHit, SearchResponse, TaskSearchHit};

use super::mapping::{format_time, map_activity, map_task};
use super::{SqliteStore, TaskListFilter};

impl SqliteStore {
    pub async fn insert_task(&self, task: &Task) -> AppResult<()> {
        query(
            r#"
            INSERT INTO tasks (
                task_id,
                project_id,
                version_id,
                title,
                summary,
                description,
                task_search_summary,
                task_context_digest,
                status,
                priority,
                created_by,
                updated_by,
                created_at,
                updated_at,
                closed_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(task.task_id.to_string())
        .bind(task.project_id.to_string())
        .bind(task.version_id.map(|value| value.to_string()))
        .bind(&task.title)
        .bind(&task.summary)
        .bind(&task.description)
        .bind(&task.task_search_summary)
        .bind(&task.task_context_digest)
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

    pub async fn get_task_by_ref(&self, reference: &str) -> AppResult<Task> {
        let row = query(
            r#"
            SELECT
                task_id, project_id, version_id, title, summary, description,
                task_search_summary, task_context_digest, status, priority,
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

    pub async fn list_tasks(&self, filter: TaskListFilter) -> AppResult<Vec<Task>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                task_id, project_id, version_id, title, summary, description,
                task_search_summary, task_context_digest, status, priority,
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
        builder.push(" ORDER BY created_at DESC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter().map(map_task).collect()
    }

    pub async fn update_task(&self, task: &Task) -> AppResult<()> {
        query(
            r#"
            UPDATE tasks
            SET
                version_id = ?,
                title = ?,
                summary = ?,
                description = ?,
                task_search_summary = ?,
                task_context_digest = ?,
                status = ?,
                priority = ?,
                updated_by = ?,
                updated_at = ?,
                closed_at = ?
            WHERE task_id = ?
            "#,
        )
        .bind(task.version_id.map(|value| value.to_string()))
        .bind(&task.title)
        .bind(&task.summary)
        .bind(&task.description)
        .bind(&task.task_search_summary)
        .bind(&task.task_context_digest)
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

    pub async fn list_task_activities(&self, task_id: Uuid) -> AppResult<Vec<TaskActivity>> {
        let rows = query(
            r#"
            SELECT activity_id, task_id, kind, content, activity_search_summary, created_by, created_at, metadata_json
            FROM task_activities
            WHERE task_id = ?
            ORDER BY created_at DESC
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
            SELECT t.task_id, t.title, t.status, t.priority, t.task_search_summary
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
                title: row.get::<String, _>("title"),
                status: row.get::<String, _>("status"),
                priority: row.get::<String, _>("priority"),
                summary: row.get::<String, _>("task_search_summary"),
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
            query: query_text.to_string(),
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
