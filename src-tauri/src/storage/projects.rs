use sqlx::query;
use uuid::Uuid;

use crate::domain::{Project, Version};
use crate::error::{AppError, AppResult};

use super::mapping::{format_time, map_project, map_version};
use super::SqliteStore;

impl SqliteStore {
    pub async fn insert_project(&self, project: &Project) -> AppResult<()> {
        query(
            r#"
            INSERT INTO projects (
                project_id,
                slug,
                name,
                description,
                status,
                default_version_id,
                created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(project.project_id.to_string())
        .bind(&project.slug)
        .bind(&project.name)
        .bind(&project.description)
        .bind(project.status.to_string())
        .bind(project.default_version_id.map(|value| value.to_string()))
        .bind(format_time(project.created_at)?)
        .bind(format_time(project.updated_at)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_project_by_ref(&self, reference: &str) -> AppResult<Project> {
        let row = query(
            r#"
            SELECT project_id, slug, name, description, status, default_version_id, created_at, updated_at
            FROM projects
            WHERE project_id = ? OR slug = ?
            "#,
        )
        .bind(reference)
        .bind(reference)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_project)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "project".to_string(),
                reference: reference.to_string(),
            })
    }

    pub async fn list_projects(&self) -> AppResult<Vec<Project>> {
        let rows = query(
            r#"
            SELECT project_id, slug, name, description, status, default_version_id, created_at, updated_at
            FROM projects
            ORDER BY created_at DESC, project_id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_project).collect()
    }

    pub async fn update_project(&self, project: &Project) -> AppResult<()> {
        query(
            r#"
            UPDATE projects
            SET slug = ?, name = ?, description = ?, status = ?, default_version_id = ?, updated_at = ?
            WHERE project_id = ?
            "#,
        )
        .bind(&project.slug)
        .bind(&project.name)
        .bind(&project.description)
        .bind(project.status.to_string())
        .bind(project.default_version_id.map(|value| value.to_string()))
        .bind(format_time(project.updated_at)?)
        .bind(project.project_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_version(&self, version: &Version) -> AppResult<()> {
        query(
            r#"
            INSERT INTO versions (version_id, project_id, name, description, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(version.version_id.to_string())
        .bind(version.project_id.to_string())
        .bind(&version.name)
        .bind(&version.description)
        .bind(version.status.to_string())
        .bind(format_time(version.created_at)?)
        .bind(format_time(version.updated_at)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_version_by_ref(&self, reference: &str) -> AppResult<Version> {
        let row = query(
            r#"
            SELECT version_id, project_id, name, description, status, created_at, updated_at
            FROM versions
            WHERE version_id = ?
            "#,
        )
        .bind(reference)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_version)
            .transpose()?
            .ok_or_else(|| AppError::NotFound {
                entity: "version".to_string(),
                reference: reference.to_string(),
            })
    }

    pub async fn list_versions(&self, project_id: Option<Uuid>) -> AppResult<Vec<Version>> {
        let rows = if let Some(project_id) = project_id {
            query(
                r#"
                SELECT version_id, project_id, name, description, status, created_at, updated_at
                FROM versions
                WHERE project_id = ?
                ORDER BY created_at DESC, version_id DESC
                "#,
            )
            .bind(project_id.to_string())
            .fetch_all(&self.pool)
            .await?
        } else {
            query(
                r#"
                SELECT version_id, project_id, name, description, status, created_at, updated_at
                FROM versions
                ORDER BY created_at DESC, version_id DESC
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter().map(map_version).collect()
    }

    pub async fn update_version(&self, version: &Version) -> AppResult<()> {
        query(
            r#"
            UPDATE versions
            SET name = ?, description = ?, status = ?, updated_at = ?
            WHERE version_id = ?
            "#,
        )
        .bind(&version.name)
        .bind(&version.description)
        .bind(version.status.to_string())
        .bind(format_time(version.updated_at)?)
        .bind(version.version_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_project_default_version(
        &self,
        project_id: Uuid,
        version_id: Option<Uuid>,
        updated_at: time::OffsetDateTime,
    ) -> AppResult<()> {
        query(
            r#"
            UPDATE projects
            SET default_version_id = ?, updated_at = ?
            WHERE project_id = ?
            "#,
        )
        .bind(version_id.map(|value| value.to_string()))
        .bind(format_time(updated_at)?)
        .bind(project_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
