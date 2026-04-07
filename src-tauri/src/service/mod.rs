use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::{
    Attachment, AttachmentKind, Project, ProjectStatus, Task, TaskActivity, TaskActivityKind,
    TaskPriority, TaskStatus, Version, VersionStatus,
};
use crate::error::{AppError, AppResult};
use crate::policy::PolicyEngine;
use crate::search::{
    build_activity_search_summary, build_task_context_digest, build_task_search_summary,
    SearchResponse,
};
use crate::storage::{SqliteStore, TaskListFilter};

#[derive(Clone)]
pub struct AgentaService {
    store: SqliteStore,
    policy: PolicyEngine,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServiceOverview {
    pub project_count: i64,
    pub task_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateProjectInput {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateProjectInput {
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<ProjectStatus>,
    pub default_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateVersionInput {
    pub project: String,
    pub name: String,
    pub description: Option<String>,
    pub status: Option<VersionStatus>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateVersionInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<VersionStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateTaskInput {
    pub project: String,
    pub version: Option<String>,
    pub title: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub created_by: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct UpdateTaskInput {
    pub version: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub updated_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateNoteInput {
    pub task: String,
    pub content: String,
    pub created_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateAttachmentInput {
    pub task: String,
    pub path: PathBuf,
    pub kind: Option<AttachmentKind>,
    pub created_by: Option<String>,
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TaskQuery {
    pub project: Option<String>,
    pub version: Option<String>,
    pub status: Option<TaskStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchInput {
    pub text: String,
    pub limit: Option<usize>,
}

impl AgentaService {
    pub fn new(store: SqliteStore, policy: PolicyEngine) -> Self {
        Self { store, policy }
    }

    pub async fn service_overview(&self) -> AppResult<ServiceOverview> {
        Ok(ServiceOverview {
            project_count: self.store.project_count().await?,
            task_count: self.store.task_count().await?,
        })
    }

    pub async fn create_project(&self, input: CreateProjectInput) -> AppResult<Project> {
        self.enforce("project.create")?;
        let slug = normalize_slug(&input.slug);
        if slug.is_empty() {
            return Err(AppError::InvalidArguments(
                "project slug must not be empty".to_string(),
            ));
        }

        let now = OffsetDateTime::now_utc();
        let project = Project {
            project_id: Uuid::new_v4(),
            slug,
            name: require_non_empty(input.name, "project name")?,
            description: clean_optional(input.description),
            status: ProjectStatus::Active,
            default_version_id: None,
            created_at: now,
            updated_at: now,
        };
        self.store.insert_project(&project).await?;
        Ok(project)
    }

    pub async fn get_project(&self, reference: &str) -> AppResult<Project> {
        self.store.get_project_by_ref(reference).await
    }

    pub async fn list_projects(&self) -> AppResult<Vec<Project>> {
        self.store.list_projects().await
    }

    pub async fn update_project(
        &self,
        reference: &str,
        input: UpdateProjectInput,
    ) -> AppResult<Project> {
        self.enforce("project.update")?;
        let mut project = self.store.get_project_by_ref(reference).await?;
        if let Some(slug) = input.slug {
            project.slug = normalize_slug(&slug);
        }
        if let Some(name) = input.name {
            project.name = require_non_empty(name, "project name")?;
        }
        if let Some(description) = input.description {
            project.description = clean_optional(Some(description));
        }
        if let Some(status) = input.status {
            project.status = status;
        }
        if let Some(default_version) = input.default_version {
            let version = self.store.get_version_by_ref(&default_version).await?;
            if version.project_id != project.project_id {
                return Err(AppError::Conflict(
                    "default version must belong to the target project".to_string(),
                ));
            }
            project.default_version_id = Some(version.version_id);
        }
        project.updated_at = OffsetDateTime::now_utc();
        self.store.update_project(&project).await?;
        Ok(project)
    }

    pub async fn create_version(&self, input: CreateVersionInput) -> AppResult<Version> {
        self.enforce("version.create")?;
        let project = self.store.get_project_by_ref(&input.project).await?;
        let now = OffsetDateTime::now_utc();
        let version = Version {
            version_id: Uuid::new_v4(),
            project_id: project.project_id,
            name: require_non_empty(input.name, "version name")?,
            description: clean_optional(input.description),
            status: input.status.unwrap_or_default(),
            created_at: now,
            updated_at: now,
        };
        self.store.insert_version(&version).await?;
        if project.default_version_id.is_none() {
            self.store
                .set_project_default_version(project.project_id, Some(version.version_id), now)
                .await?;
        }
        Ok(version)
    }

    pub async fn get_version(&self, reference: &str) -> AppResult<Version> {
        self.store.get_version_by_ref(reference).await
    }

    pub async fn list_versions(&self, project_ref: Option<&str>) -> AppResult<Vec<Version>> {
        let project_id = match project_ref {
            Some(reference) => Some(self.store.get_project_by_ref(reference).await?.project_id),
            None => None,
        };
        self.store.list_versions(project_id).await
    }

    pub async fn update_version(
        &self,
        reference: &str,
        input: UpdateVersionInput,
    ) -> AppResult<Version> {
        self.enforce("version.update")?;
        let mut version = self.store.get_version_by_ref(reference).await?;
        if let Some(name) = input.name {
            version.name = require_non_empty(name, "version name")?;
        }
        if let Some(description) = input.description {
            version.description = clean_optional(Some(description));
        }
        if let Some(status) = input.status {
            version.status = status;
        }
        version.updated_at = OffsetDateTime::now_utc();
        self.store.update_version(&version).await?;
        Ok(version)
    }

    pub async fn create_task(&self, input: CreateTaskInput) -> AppResult<Task> {
        self.enforce("task.create")?;
        let project = self.store.get_project_by_ref(&input.project).await?;
        let version_id = self.resolve_version_for_project(project.project_id, input.version.as_deref()).await?;
        let now = OffsetDateTime::now_utc();
        let created_by = input
            .created_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        let mut task = Task {
            task_id: Uuid::new_v4(),
            project_id: project.project_id,
            version_id,
            title: require_non_empty(input.title, "task title")?,
            summary: clean_optional(input.summary),
            description: clean_optional(input.description),
            task_search_summary: String::new(),
            task_context_digest: String::new(),
            status: input.status.unwrap_or_default(),
            priority: input.priority.unwrap_or_default(),
            created_by: created_by.clone(),
            updated_by: created_by,
            created_at: now,
            updated_at: now,
            closed_at: None,
        };
        task.closed_at = closed_at_for_status(task.status, now);
        task.task_search_summary = build_task_search_summary(
            &task.title,
            task.summary.as_deref(),
            task.description.as_deref(),
        );
        task.task_context_digest = build_task_context_digest(&task);
        self.store.insert_task(&task).await?;
        Ok(task)
    }

    pub async fn get_task(&self, reference: &str) -> AppResult<Task> {
        self.store.get_task_by_ref(reference).await
    }

    pub async fn list_tasks(&self, query: TaskQuery) -> AppResult<Vec<Task>> {
        let filter = TaskListFilter {
            project_id: match query.project {
                Some(reference) => Some(self.store.get_project_by_ref(&reference).await?.project_id),
                None => None,
            },
            version_id: match query.version {
                Some(reference) => Some(self.store.get_version_by_ref(&reference).await?.version_id),
                None => None,
            },
            status: query.status,
        };
        self.store.list_tasks(filter).await
    }

    pub async fn update_task(&self, reference: &str, input: UpdateTaskInput) -> AppResult<Task> {
        self.enforce("task.update")?;
        let mut task = self.store.get_task_by_ref(reference).await?;
        if let Some(version) = input.version {
            task.version_id = self.resolve_version_for_project(task.project_id, Some(&version)).await?;
        }
        if let Some(title) = input.title {
            task.title = require_non_empty(title, "task title")?;
        }
        if let Some(summary) = input.summary {
            task.summary = clean_optional(Some(summary));
        }
        if let Some(description) = input.description {
            task.description = clean_optional(Some(description));
        }
        if let Some(status) = input.status {
            task.status = status;
        }
        if let Some(priority) = input.priority {
            task.priority = priority;
        }
        task.updated_by = input
            .updated_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        task.updated_at = OffsetDateTime::now_utc();
        task.closed_at = closed_at_for_status(task.status, task.updated_at);
        task.task_search_summary = build_task_search_summary(
            &task.title,
            task.summary.as_deref(),
            task.description.as_deref(),
        );
        task.task_context_digest = build_task_context_digest(&task);
        self.store.update_task(&task).await?;
        Ok(task)
    }

    pub async fn create_note(&self, input: CreateNoteInput) -> AppResult<TaskActivity> {
        self.enforce("note.create")?;
        let task = self.store.get_task_by_ref(&input.task).await?;
        let now = OffsetDateTime::now_utc();
        let content = require_non_empty(input.content, "note content")?;
        let activity = TaskActivity {
            activity_id: Uuid::new_v4(),
            task_id: task.task_id,
            kind: TaskActivityKind::Note,
            content: content.clone(),
            activity_search_summary: build_activity_search_summary(TaskActivityKind::Note, &content),
            created_by: input
                .created_by
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "cli".to_string()),
            created_at: now,
            metadata_json: json!({}),
        };
        self.store.insert_activity(&activity).await?;
        Ok(activity)
    }

    pub async fn list_task_activities(&self, task_ref: &str) -> AppResult<Vec<TaskActivity>> {
        let task = self.store.get_task_by_ref(task_ref).await?;
        self.store.list_task_activities(task.task_id).await
    }

    pub async fn list_notes(&self, task_ref: &str) -> AppResult<Vec<TaskActivity>> {
        let activities = self.list_task_activities(task_ref).await?;
        Ok(activities
            .into_iter()
            .filter(|activity| activity.kind == TaskActivityKind::Note)
            .collect())
    }

    pub async fn create_attachment(
        &self,
        input: CreateAttachmentInput,
    ) -> AppResult<Attachment> {
        self.enforce("attachment.create")?;
        let task = self.store.get_task_by_ref(&input.task).await?;
        let now = OffsetDateTime::now_utc();
        let attachment_id = Uuid::new_v4();
        let stored = self
            .store
            .persist_attachment_file(task.task_id, attachment_id, &input.path)
            .await?;
        let summary = input
            .summary
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| stored.original_filename.clone());
        let created_by = input
            .created_by
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "cli".to_string());
        let attachment = Attachment {
            attachment_id,
            task_id: task.task_id,
            kind: input.kind.unwrap_or_default(),
            mime: stored.mime.clone(),
            original_filename: stored.original_filename.clone(),
            original_path: stored.original_path.clone(),
            storage_path: stored.storage_path.clone(),
            sha256: stored.sha256,
            size_bytes: stored.size_bytes,
            summary: summary.clone(),
            created_by: created_by.clone(),
            created_at: now,
        };
        self.store.insert_attachment(&attachment).await?;
        let activity = TaskActivity {
            activity_id: Uuid::new_v4(),
            task_id: task.task_id,
            kind: TaskActivityKind::AttachmentRef,
            content: summary.clone(),
            activity_search_summary: build_activity_search_summary(
                TaskActivityKind::AttachmentRef,
                &summary,
            ),
            created_by,
            created_at: now,
            metadata_json: json!({
                "attachment_id": attachment.attachment_id,
                "storage_path": attachment.storage_path,
            }),
        };
        self.store.insert_activity(&activity).await?;
        Ok(attachment)
    }

    pub async fn list_attachments(&self, task_ref: &str) -> AppResult<Vec<Attachment>> {
        let task = self.store.get_task_by_ref(task_ref).await?;
        self.store.list_attachments(task.task_id).await
    }

    pub async fn get_attachment(&self, reference: &str) -> AppResult<Attachment> {
        self.store.get_attachment_by_ref(reference).await
    }

    pub async fn search(&self, input: SearchInput) -> AppResult<SearchResponse> {
        let text = require_non_empty(input.text, "search text")?;
        let limit = input.limit.unwrap_or(10).clamp(1, 50);
        self.store.search(&text, limit).await
    }

    fn enforce(&self, action: &str) -> AppResult<()> {
        self.policy
            .enforce(action)
            .map_err(|violation| AppError::PolicyBlocked {
                action: violation.action,
                decision: violation.decision,
            })
    }

    async fn resolve_version_for_project(
        &self,
        project_id: Uuid,
        version_ref: Option<&str>,
    ) -> AppResult<Option<Uuid>> {
        match version_ref {
            Some(reference) => {
                let version = self.store.get_version_by_ref(reference).await?;
                if version.project_id != project_id {
                    return Err(AppError::Conflict(
                        "version must belong to the selected project".to_string(),
                    ));
                }
                Ok(Some(version.version_id))
            }
            None => Ok(None),
        }
    }
}

fn normalize_slug(value: &str) -> String {
    value.trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            '-' | '_' => '-',
            ' ' => '-',
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn require_non_empty(value: String, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::InvalidArguments(format!("{field} must not be empty")))
    } else {
        Ok(trimmed.to_string())
    }
}

fn closed_at_for_status(status: TaskStatus, now: OffsetDateTime) -> Option<OffsetDateTime> {
    match status {
        TaskStatus::Done | TaskStatus::Cancelled => Some(now),
        _ => None,
    }
}
