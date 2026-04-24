use super::*;

impl AgentaService {
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
        let _write_guard = self.write_queue.lock().await;
        self.create_project_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    pub async fn get_project(&self, reference: &str) -> AppResult<Project> {
        self.store.get_project_by_ref(reference).await
    }

    pub async fn list_projects(&self) -> AppResult<Vec<Project>> {
        self.store.list_projects().await
    }

    pub async fn list_projects_page(&self, page: PageRequest) -> AppResult<PageResult<Project>> {
        let projects = self.store.list_projects().await?;
        Ok(paginate_by_created_at(
            projects,
            page,
            |project| project.created_at,
            |project| project.project_id,
        ))
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
        let _write_guard = self.write_queue.lock().await;
        self.update_project_internal(reference, input, ApprovalMode::Standard(approval))
            .await
    }

    pub(super) async fn create_project_internal(
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
        let mut tx = self.store.pool.begin().await?;
        self.store.insert_project_tx(&mut tx, &project).await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Project,
            project.project_id,
            SyncOperation::Create,
            &project,
            project.updated_at,
        )
        .await?;
        tx.commit().await?;
        Ok(project)
    }

    pub(super) async fn update_project_internal(
        &self,
        reference: &str,
        input: UpdateProjectInput,
        mode: ApprovalMode,
    ) -> AppResult<Project> {
        self.enforce("project.update", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let mut project = self.store.get_project_by_ref_tx(&mut tx, reference).await?;
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
            let version = self
                .store
                .get_version_by_ref_tx(&mut tx, &default_version)
                .await?;
            if version.project_id != project.project_id {
                return Err(AppError::Conflict(
                    "default version must belong to the target project".to_string(),
                ));
            }
            project.default_version_id = Some(version.version_id);
        }
        project.updated_at = OffsetDateTime::now_utc();
        self.store.update_project_tx(&mut tx, &project).await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Project,
            project.project_id,
            SyncOperation::Update,
            &project,
            project.updated_at,
        )
        .await?;
        self.queue_project_task_search_jobs_tx(&mut tx, project.project_id)
            .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(project)
    }
}
