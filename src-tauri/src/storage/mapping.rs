use std::str::FromStr;

use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::{
    Attachment, Project, Task, TaskActivity, Version,
};
use crate::error::{AppError, AppResult};

pub(crate) fn map_project(row: SqliteRow) -> AppResult<Project> {
    Ok(Project {
        project_id: parse_uuid(row.get("project_id"), "project_id")?,
        slug: row.get("slug"),
        name: row.get("name"),
        description: row.get("description"),
        status: parse_enum(row.get("status"), "status")?,
        default_version_id: row
            .get::<Option<String>, _>("default_version_id")
            .map(|value| parse_uuid(value, "default_version_id"))
            .transpose()?,
        created_at: parse_time(row.get("created_at"), "created_at")?,
        updated_at: parse_time(row.get("updated_at"), "updated_at")?,
    })
}

pub(crate) fn map_version(row: SqliteRow) -> AppResult<Version> {
    Ok(Version {
        version_id: parse_uuid(row.get("version_id"), "version_id")?,
        project_id: parse_uuid(row.get("project_id"), "project_id")?,
        name: row.get("name"),
        description: row.get("description"),
        status: parse_enum(row.get("status"), "status")?,
        created_at: parse_time(row.get("created_at"), "created_at")?,
        updated_at: parse_time(row.get("updated_at"), "updated_at")?,
    })
}

pub(crate) fn map_task(row: SqliteRow) -> AppResult<Task> {
    Ok(Task {
        task_id: parse_uuid(row.get("task_id"), "task_id")?,
        project_id: parse_uuid(row.get("project_id"), "project_id")?,
        version_id: row
            .get::<Option<String>, _>("version_id")
            .map(|value| parse_uuid(value, "version_id"))
            .transpose()?,
        title: row.get("title"),
        summary: row.get("summary"),
        description: row.get("description"),
        task_search_summary: row.get("task_search_summary"),
        task_context_digest: row.get("task_context_digest"),
        status: parse_enum(row.get("status"), "status")?,
        priority: parse_enum(row.get("priority"), "priority")?,
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: parse_time(row.get("created_at"), "created_at")?,
        updated_at: parse_time(row.get("updated_at"), "updated_at")?,
        closed_at: row
            .get::<Option<String>, _>("closed_at")
            .map(|value| parse_time(value, "closed_at"))
            .transpose()?,
    })
}

pub(crate) fn map_activity(row: SqliteRow) -> AppResult<TaskActivity> {
    let metadata_json = row.get::<String, _>("metadata_json");
    Ok(TaskActivity {
        activity_id: parse_uuid(row.get("activity_id"), "activity_id")?,
        task_id: parse_uuid(row.get("task_id"), "task_id")?,
        kind: parse_enum(row.get("kind"), "kind")?,
        content: row.get("content"),
        activity_search_summary: row.get("activity_search_summary"),
        created_by: row.get("created_by"),
        created_at: parse_time(row.get("created_at"), "created_at")?,
        metadata_json: serde_json::from_str(&metadata_json)
            .map_err(|error| AppError::Storage(format!("invalid activity metadata_json: {error}")))?,
    })
}

pub(crate) fn map_attachment(row: SqliteRow) -> AppResult<Attachment> {
    Ok(Attachment {
        attachment_id: parse_uuid(row.get("attachment_id"), "attachment_id")?,
        task_id: parse_uuid(row.get("task_id"), "task_id")?,
        kind: parse_enum(row.get("kind"), "kind")?,
        mime: row.get("mime"),
        original_filename: row.get("original_filename"),
        original_path: row.get("original_path"),
        storage_path: row.get("storage_path"),
        sha256: row.get("sha256"),
        size_bytes: row.get("size_bytes"),
        summary: row.get("summary"),
        created_by: row.get("created_by"),
        created_at: parse_time(row.get("created_at"), "created_at")?,
    })
}

pub(crate) fn parse_uuid(value: String, field: &str) -> AppResult<Uuid> {
    Uuid::parse_str(&value)
        .map_err(|error| AppError::Storage(format!("invalid uuid in {field}: {error}")))
}

pub(crate) fn parse_time(value: String, field: &str) -> AppResult<OffsetDateTime> {
    OffsetDateTime::parse(&value, &Rfc3339)
        .map_err(|error| AppError::Storage(format!("invalid timestamp in {field}: {error}")))
}

pub(crate) fn parse_enum<T>(value: String, field: &str) -> AppResult<T>
where
    T: FromStr<Err = String>,
{
    value
        .parse::<T>()
        .map_err(|error| AppError::Storage(format!("invalid enum in {field}: {error}")))
}

pub(crate) fn format_time(value: OffsetDateTime) -> AppResult<String> {
    value
        .format(&Rfc3339)
        .map_err(|error| AppError::Internal(format!("failed to format timestamp: {error}")))
}

pub(crate) fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect()
}
