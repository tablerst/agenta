use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::{
    ApprovalRequest, ApprovalRequestedVia, ApprovalStatus, Attachment, AttachmentKind, Project,
    ProjectStatus, Task, TaskActivity, TaskActivityKind, TaskPriority, TaskStatus, Version,
    VersionStatus,
};
use crate::error::{AppError, AppResult};
use crate::policy::{PolicyEngine, WriteDecision};
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

#[derive(Clone, Copy, Debug)]
pub enum RequestOrigin {
    Cli,
    Mcp,
    Desktop,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ApprovalQuery {
    pub project: Option<String>,
    pub status: Option<ApprovalStatus>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ReviewApprovalInput {
    pub reviewed_by: Option<String>,
    pub review_note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ApprovalSeed {
    requested_via: ApprovalRequestedVia,
    resource_ref: String,
    payload_json: Value,
    request_summary: String,
    requested_by: String,
}

#[derive(Default)]
struct ApprovalContext {
    project_ref: Option<String>,
    project_name: Option<String>,
    task_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ReferencedUpdatePayload<T> {
    reference: String,
    input: T,
}

#[derive(Clone, Debug)]
enum ApprovalMode {
    Standard(ApprovalSeed),
    Replay,
}

impl RequestOrigin {
    fn requested_via(self) -> ApprovalRequestedVia {
        match self {
            Self::Cli => ApprovalRequestedVia::Cli,
            Self::Mcp => ApprovalRequestedVia::Mcp,
            Self::Desktop => ApprovalRequestedVia::Desktop,
        }
    }

    fn fallback_actor(self) -> &'static str {
        self.requested_via().as_str()
    }
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
        self.create_project_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_project_from(
        &self,
        origin: RequestOrigin,
        input: CreateProjectInput,
    ) -> AppResult<Project> {
        let slug = normalize_slug(&input.slug);
        if slug.is_empty() {
            return Err(AppError::InvalidArguments(
                "project slug must not be empty".to_string(),
            ));
        }
        let approval = self.approval_seed(
            origin,
            slug.clone(),
            format!("Create project {slug}"),
            actor_or_default(None, origin),
            &input,
        )?;
        self.create_project_internal(input, ApprovalMode::Standard(approval))
            .await
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
        self.update_project_from(RequestOrigin::Cli, reference, input)
            .await
    }

    pub async fn update_project_from(
        &self,
        origin: RequestOrigin,
        reference: &str,
        input: UpdateProjectInput,
    ) -> AppResult<Project> {
        let approval = self.approval_seed(
            origin,
            reference.to_string(),
            format!("Update project {reference}"),
            actor_or_default(None, origin),
            &ReferencedUpdatePayload {
                reference: reference.to_string(),
                input: input.clone(),
            },
        )?;
        self.update_project_internal(reference, input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn create_version(&self, input: CreateVersionInput) -> AppResult<Version> {
        self.create_version_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_version_from(
        &self,
        origin: RequestOrigin,
        input: CreateVersionInput,
    ) -> AppResult<Version> {
        let approval = self.approval_seed(
            origin,
            input.project.clone(),
            format!(
                "Create version {} in {}",
                input.name.trim(),
                input.project.trim()
            ),
            actor_or_default(None, origin),
            &input,
        )?;
        self.create_version_internal(input, ApprovalMode::Standard(approval))
            .await
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
        self.update_version_from(RequestOrigin::Cli, reference, input)
            .await
    }

    pub async fn update_version_from(
        &self,
        origin: RequestOrigin,
        reference: &str,
        input: UpdateVersionInput,
    ) -> AppResult<Version> {
        let approval = self.approval_seed(
            origin,
            reference.to_string(),
            format!("Update version {reference}"),
            actor_or_default(None, origin),
            &ReferencedUpdatePayload {
                reference: reference.to_string(),
                input: input.clone(),
            },
        )?;
        self.update_version_internal(reference, input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn create_task(&self, input: CreateTaskInput) -> AppResult<Task> {
        self.create_task_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_task_from(
        &self,
        origin: RequestOrigin,
        mut input: CreateTaskInput,
    ) -> AppResult<Task> {
        if input
            .created_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.created_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input.project.clone(),
            format!("Create task {}", input.title.trim()),
            actor_or_default(input.created_by.as_deref(), origin),
            &input,
        )?;
        self.create_task_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn get_task(&self, reference: &str) -> AppResult<Task> {
        self.store.get_task_by_ref(reference).await
    }

    pub async fn list_tasks(&self, query: TaskQuery) -> AppResult<Vec<Task>> {
        let filter = TaskListFilter {
            project_id: match query.project {
                Some(reference) => {
                    Some(self.store.get_project_by_ref(&reference).await?.project_id)
                }
                None => None,
            },
            version_id: match query.version {
                Some(reference) => {
                    Some(self.store.get_version_by_ref(&reference).await?.version_id)
                }
                None => None,
            },
            status: query.status,
        };
        self.store.list_tasks(filter).await
    }

    pub async fn update_task(&self, reference: &str, input: UpdateTaskInput) -> AppResult<Task> {
        self.update_task_from(RequestOrigin::Cli, reference, input)
            .await
    }

    pub async fn update_task_from(
        &self,
        origin: RequestOrigin,
        reference: &str,
        mut input: UpdateTaskInput,
    ) -> AppResult<Task> {
        if input
            .updated_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.updated_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            reference.to_string(),
            format!("Update task {reference}"),
            actor_or_default(input.updated_by.as_deref(), origin),
            &ReferencedUpdatePayload {
                reference: reference.to_string(),
                input: input.clone(),
            },
        )?;
        self.update_task_internal(reference, input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn create_note(&self, input: CreateNoteInput) -> AppResult<TaskActivity> {
        self.create_note_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_note_from(
        &self,
        origin: RequestOrigin,
        mut input: CreateNoteInput,
    ) -> AppResult<TaskActivity> {
        if input
            .created_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.created_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input.task.clone(),
            format!("Add note to task {}", input.task.trim()),
            actor_or_default(input.created_by.as_deref(), origin),
            &input,
        )?;
        self.create_note_internal(input, ApprovalMode::Standard(approval))
            .await
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

    pub async fn create_attachment(&self, input: CreateAttachmentInput) -> AppResult<Attachment> {
        self.create_attachment_from(RequestOrigin::Cli, input).await
    }

    pub async fn create_attachment_from(
        &self,
        origin: RequestOrigin,
        mut input: CreateAttachmentInput,
    ) -> AppResult<Attachment> {
        if input
            .created_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.created_by = Some(origin.fallback_actor().to_string());
        }
        let summary = input
            .summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                input
                    .path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("attachment")
            })
            .to_string();
        let approval = self.approval_seed(
            origin,
            input.task.clone(),
            format!("Add attachment {summary} to task {}", input.task.trim()),
            actor_or_default(input.created_by.as_deref(), origin),
            &input,
        )?;
        self.create_attachment_internal(input, ApprovalMode::Standard(approval))
            .await
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

    pub async fn list_approval_requests(
        &self,
        query: ApprovalQuery,
    ) -> AppResult<Vec<ApprovalRequest>> {
        let (project_slug_filter, project_id_filter) = match query.project.as_deref() {
            Some(reference) => match self.store.get_project_by_ref(reference).await {
                Ok(project) => (Some(project.slug), Some(project.project_id.to_string())),
                Err(AppError::NotFound { .. }) => (Some(reference.to_string()), None),
                Err(error) => return Err(error),
            },
            None => (None, None),
        };
        let items = self.store.list_approval_requests(query.status).await?;
        let mut approvals = Vec::with_capacity(items.len());

        for item in items {
            let approval = self.enrich_approval_request(item).await;
            if let Some(project_slug) = project_slug_filter.as_deref() {
                if !matches_project_filter(&approval, project_slug, project_id_filter.as_deref()) {
                    continue;
                }
            }
            approvals.push(approval);
        }

        Ok(approvals)
    }

    pub async fn get_approval_request(&self, request_id: &str) -> AppResult<ApprovalRequest> {
        let request = self
            .store
            .get_approval_request(parse_uuid(request_id, "request_id")?)
            .await?;
        Ok(self.enrich_approval_request(request).await)
    }

    pub async fn approve_approval_request(
        &self,
        request_id: &str,
        input: ReviewApprovalInput,
    ) -> AppResult<ApprovalRequest> {
        let request_id = parse_uuid(request_id, "request_id")?;
        let mut request = self.store.get_approval_request(request_id).await?;
        ensure_pending(&request)?;

        let reviewer = actor_or_default(input.reviewed_by.as_deref(), RequestOrigin::Desktop);
        let review_note = clean_optional(input.review_note);
        let reviewed_at = OffsetDateTime::now_utc();

        match self.replay_approval_request(&request).await {
            Ok(result_json) => {
                request.reviewed_at = Some(reviewed_at);
                request.reviewed_by = Some(reviewer);
                request.review_note = review_note;
                request.result_json = Some(result_json);
                request.error_json = None;
                request.status = ApprovalStatus::Approved;
            }
            Err(app_error) => {
                request.reviewed_at = Some(reviewed_at);
                request.reviewed_by = Some(reviewer);
                request.review_note = review_note;
                request.result_json = None;
                request.error_json = Some(error_value(&app_error));
                request.status = ApprovalStatus::Failed;
            }
        }
        self.store.update_approval_request(&request).await?;
        Ok(self.enrich_approval_request(request).await)
    }

    pub async fn deny_approval_request(
        &self,
        request_id: &str,
        input: ReviewApprovalInput,
    ) -> AppResult<ApprovalRequest> {
        let request_id = parse_uuid(request_id, "request_id")?;
        let mut request = self.store.get_approval_request(request_id).await?;
        ensure_pending(&request)?;
        request.reviewed_at = Some(OffsetDateTime::now_utc());
        request.reviewed_by = Some(actor_or_default(
            input.reviewed_by.as_deref(),
            RequestOrigin::Desktop,
        ));
        request.review_note = clean_optional(input.review_note);
        request.result_json = None;
        request.error_json = None;
        request.status = ApprovalStatus::Denied;
        self.store.update_approval_request(&request).await?;
        Ok(self.enrich_approval_request(request).await)
    }

    async fn create_project_internal(
        &self,
        input: CreateProjectInput,
        mode: ApprovalMode,
    ) -> AppResult<Project> {
        let slug = normalize_slug(&input.slug);
        if slug.is_empty() {
            return Err(AppError::InvalidArguments(
                "project slug must not be empty".to_string(),
            ));
        }

        self.enforce("project.create", mode).await?;

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

    async fn update_project_internal(
        &self,
        reference: &str,
        input: UpdateProjectInput,
        mode: ApprovalMode,
    ) -> AppResult<Project> {
        self.enforce("project.update", mode).await?;
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

    async fn create_version_internal(
        &self,
        input: CreateVersionInput,
        mode: ApprovalMode,
    ) -> AppResult<Version> {
        self.enforce("version.create", mode).await?;
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

    async fn update_version_internal(
        &self,
        reference: &str,
        input: UpdateVersionInput,
        mode: ApprovalMode,
    ) -> AppResult<Version> {
        self.enforce("version.update", mode).await?;
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

    async fn create_task_internal(
        &self,
        input: CreateTaskInput,
        mode: ApprovalMode,
    ) -> AppResult<Task> {
        self.enforce("task.create", mode).await?;
        let project = self.store.get_project_by_ref(&input.project).await?;
        let version_id = self
            .resolve_version_for_project(project.project_id, input.version.as_deref())
            .await?;
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

    async fn update_task_internal(
        &self,
        reference: &str,
        input: UpdateTaskInput,
        mode: ApprovalMode,
    ) -> AppResult<Task> {
        self.enforce("task.update", mode).await?;
        let mut task = self.store.get_task_by_ref(reference).await?;
        if let Some(version) = input.version {
            task.version_id = self
                .resolve_version_for_project(task.project_id, Some(&version))
                .await?;
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

    async fn create_note_internal(
        &self,
        input: CreateNoteInput,
        mode: ApprovalMode,
    ) -> AppResult<TaskActivity> {
        self.enforce("note.create", mode).await?;
        let task = self.store.get_task_by_ref(&input.task).await?;
        let now = OffsetDateTime::now_utc();
        let content = require_non_empty(input.content, "note content")?;
        let activity = TaskActivity {
            activity_id: Uuid::new_v4(),
            task_id: task.task_id,
            kind: TaskActivityKind::Note,
            content: content.clone(),
            activity_search_summary: build_activity_search_summary(
                TaskActivityKind::Note,
                &content,
            ),
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

    async fn create_attachment_internal(
        &self,
        input: CreateAttachmentInput,
        mode: ApprovalMode,
    ) -> AppResult<Attachment> {
        self.enforce("attachment.create", mode).await?;
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

    async fn enforce(&self, action: &str, mode: ApprovalMode) -> AppResult<()> {
        match self.policy.decision_for(action) {
            WriteDecision::Auto => Ok(()),
            WriteDecision::RequireHuman => match mode {
                ApprovalMode::Replay => Ok(()),
                ApprovalMode::Standard(seed) => {
                    let approval_request = ApprovalRequest {
                        request_id: Uuid::new_v4(),
                        action: action.to_string(),
                        requested_via: seed.requested_via,
                        resource_ref: seed.resource_ref,
                        project_ref: None,
                        project_name: None,
                        task_ref: None,
                        payload_json: seed.payload_json,
                        request_summary: seed.request_summary,
                        requested_at: OffsetDateTime::now_utc(),
                        requested_by: seed.requested_by,
                        reviewed_at: None,
                        reviewed_by: None,
                        review_note: None,
                        result_json: None,
                        error_json: None,
                        status: ApprovalStatus::Pending,
                    };
                    self.store
                        .insert_approval_request(&approval_request)
                        .await?;
                    Err(AppError::PolicyBlocked {
                        action: action.to_string(),
                        decision: WriteDecision::RequireHuman,
                        approval_request_id: Some(approval_request.request_id),
                        request_summary: Some(approval_request.request_summary.clone()),
                        payload_snapshot: Some(approval_request.payload_json.clone()),
                    })
                }
            },
            WriteDecision::Deny => Err(AppError::PolicyBlocked {
                action: action.to_string(),
                decision: WriteDecision::Deny,
                approval_request_id: None,
                request_summary: None,
                payload_snapshot: None,
            }),
        }
    }

    fn approval_seed(
        &self,
        origin: RequestOrigin,
        resource_ref: String,
        request_summary: String,
        requested_by: String,
        payload: &impl Serialize,
    ) -> AppResult<ApprovalSeed> {
        Ok(ApprovalSeed {
            requested_via: origin.requested_via(),
            resource_ref,
            payload_json: serde_json::to_value(payload).map_err(|error| {
                AppError::internal(format!("failed to serialize payload: {error}"))
            })?,
            request_summary,
            requested_by,
        })
    }

    async fn enrich_approval_request(&self, mut request: ApprovalRequest) -> ApprovalRequest {
        let context = self.resolve_approval_context(&request).await;
        request.project_ref = context.project_ref;
        request.project_name = context.project_name;
        request.task_ref = context.task_ref;
        request
    }

    async fn resolve_approval_context(&self, request: &ApprovalRequest) -> ApprovalContext {
        match request.action.as_str() {
            "project.create" => {
                let project_ref = json_string(&request.payload_json, "slug")
                    .or_else(|| Some(request.resource_ref.clone()));
                let project_name = json_string(&request.payload_json, "name");
                self.project_context_from_reference(project_ref, project_name)
                    .await
            }
            "project.update" => {
                let project_name = json_string(&request.payload_json, "name");
                self.project_context_from_reference(
                    Some(request.resource_ref.clone()),
                    project_name,
                )
                .await
            }
            "version.create" => {
                self.project_context_from_reference(
                    json_string(&request.payload_json, "project"),
                    None,
                )
                .await
            }
            "version.update" => {
                self.version_context_from_reference(&request.resource_ref)
                    .await
            }
            "task.create" => {
                let mut context = self
                    .project_context_from_reference(
                        json_string(&request.payload_json, "project"),
                        None,
                    )
                    .await;
                context.task_ref = request
                    .result_json
                    .as_ref()
                    .and_then(|value| json_string(value, "task_id"));
                context
            }
            "task.update" => {
                self.task_context_from_reference(&request.resource_ref)
                    .await
            }
            "note.create" | "attachment.create" => {
                let task_ref = json_string(&request.payload_json, "task")
                    .unwrap_or_else(|| request.resource_ref.clone());
                self.task_context_from_reference(&task_ref).await
            }
            _ => ApprovalContext::default(),
        }
    }

    async fn project_context_from_reference(
        &self,
        project_ref: Option<String>,
        project_name: Option<String>,
    ) -> ApprovalContext {
        let Some(reference) = project_ref else {
            return ApprovalContext {
                project_name,
                ..ApprovalContext::default()
            };
        };

        if let Ok(project) = self.store.get_project_by_ref(&reference).await {
            return ApprovalContext {
                project_ref: Some(project.slug),
                project_name: Some(project.name),
                task_ref: None,
            };
        }

        ApprovalContext {
            project_ref: Some(reference),
            project_name,
            task_ref: None,
        }
    }

    async fn version_context_from_reference(&self, reference: &str) -> ApprovalContext {
        let Ok(version) = self.store.get_version_by_ref(reference).await else {
            return ApprovalContext::default();
        };

        self.project_context_from_reference(Some(version.project_id.to_string()), None)
            .await
    }

    async fn task_context_from_reference(&self, reference: &str) -> ApprovalContext {
        let Ok(task) = self.store.get_task_by_ref(reference).await else {
            return ApprovalContext {
                task_ref: Some(reference.to_string()),
                ..ApprovalContext::default()
            };
        };

        let mut context = self
            .project_context_from_reference(Some(task.project_id.to_string()), None)
            .await;
        context.task_ref = Some(task.task_id.to_string());
        context
    }

    async fn replay_approval_request(&self, request: &ApprovalRequest) -> AppResult<Value> {
        match request.action.as_str() {
            "project.create" => {
                let input =
                    serde_json::from_value::<CreateProjectInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.create_project_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "project.update" => {
                let payload =
                    serde_json::from_value::<ReferencedUpdatePayload<UpdateProjectInput>>(
                        request.payload_json.clone(),
                    )
                    .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.update_project_internal(
                        &payload.reference,
                        payload.input,
                        ApprovalMode::Replay,
                    )
                    .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "version.create" => {
                let input =
                    serde_json::from_value::<CreateVersionInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.create_version_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "version.update" => {
                let payload =
                    serde_json::from_value::<ReferencedUpdatePayload<UpdateVersionInput>>(
                        request.payload_json.clone(),
                    )
                    .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.update_version_internal(
                        &payload.reference,
                        payload.input,
                        ApprovalMode::Replay,
                    )
                    .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.create" => {
                let input = serde_json::from_value::<CreateTaskInput>(request.payload_json.clone())
                    .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.create_task_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.update" => {
                let payload = serde_json::from_value::<ReferencedUpdatePayload<UpdateTaskInput>>(
                    request.payload_json.clone(),
                )
                .map_err(|error| {
                    AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                })?;
                serde_json::to_value(
                    self.update_task_internal(
                        &payload.reference,
                        payload.input,
                        ApprovalMode::Replay,
                    )
                    .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "note.create" => {
                let input = serde_json::from_value::<CreateNoteInput>(request.payload_json.clone())
                    .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.create_note_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "attachment.create" => {
                let input =
                    serde_json::from_value::<CreateAttachmentInput>(request.payload_json.clone())
                        .map_err(|error| {
                        AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                    })?;
                serde_json::to_value(
                    self.create_attachment_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            other => Err(AppError::InvalidAction(format!(
                "unsupported approval replay action: {other}"
            ))),
        }
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
    value
        .trim()
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
        Err(AppError::InvalidArguments(format!(
            "{field} must not be empty"
        )))
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

fn actor_or_default(value: Option<&str>, origin: RequestOrigin) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(origin.fallback_actor())
        .to_string()
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
}

fn matches_project_filter(
    request: &ApprovalRequest,
    project_slug: &str,
    project_id: Option<&str>,
) -> bool {
    request.project_ref.as_deref() == Some(project_slug)
        || project_id.is_some_and(|project_id| request.project_ref.as_deref() == Some(project_id))
}

fn parse_uuid(value: &str, field: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value)
        .map_err(|error| AppError::InvalidArguments(format!("invalid {field}: {error}")))
}

fn ensure_pending(request: &ApprovalRequest) -> AppResult<()> {
    if request.status == ApprovalStatus::Pending {
        Ok(())
    } else {
        Err(AppError::Conflict(format!(
            "approval request {} is already {}",
            request.request_id, request.status
        )))
    }
}

fn error_value(app_error: &AppError) -> Value {
    json!({
        "code": app_error.code(),
        "message": app_error.message(),
        "details": app_error.details(),
    })
}
