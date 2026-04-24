use super::*;

impl AgentaService {
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
        let _write_guard = self.write_queue.lock().await;
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

    pub async fn list_versions_page(
        &self,
        project_ref: Option<&str>,
        page: PageRequest,
    ) -> AppResult<PageResult<Version>> {
        let versions = self.list_versions(project_ref).await?;
        Ok(paginate_by_created_at(
            versions,
            page,
            |version| version.created_at,
            |version| version.version_id,
        ))
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
        let _write_guard = self.write_queue.lock().await;
        self.update_version_internal(reference, input, ApprovalMode::Standard(approval))
            .await
    }

    pub(super) async fn create_version_internal(
        &self,
        input: CreateVersionInput,
        mode: ApprovalMode,
    ) -> AppResult<Version> {
        self.enforce("version.create", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let mut project = self
            .store
            .get_project_by_ref_tx(&mut tx, &input.project)
            .await?;
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
        self.store.insert_version_tx(&mut tx, &version).await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Version,
            version.version_id,
            SyncOperation::Create,
            &version,
            version.updated_at,
        )
        .await?;
        if project.default_version_id.is_none() {
            project.default_version_id = Some(version.version_id);
            project.updated_at = now;
            self.store
                .set_project_default_version_tx(
                    &mut tx,
                    project.project_id,
                    Some(version.version_id),
                    now,
                )
                .await?;
            self.enqueue_sync_mutation_tx(
                &mut tx,
                SyncEntityKind::Project,
                project.project_id,
                SyncOperation::Update,
                &project,
                project.updated_at,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(version)
    }

    pub(super) async fn update_version_internal(
        &self,
        reference: &str,
        input: UpdateVersionInput,
        mode: ApprovalMode,
    ) -> AppResult<Version> {
        self.enforce("version.update", mode).await?;
        let mut tx = self.store.pool.begin().await?;
        let mut version = self.store.get_version_by_ref_tx(&mut tx, reference).await?;
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
        self.store.update_version_tx(&mut tx, &version).await?;
        self.enqueue_sync_mutation_tx(
            &mut tx,
            SyncEntityKind::Version,
            version.version_id,
            SyncOperation::Update,
            &version,
            version.updated_at,
        )
        .await?;
        self.queue_version_task_search_jobs_tx(&mut tx, version.version_id)
            .await?;
        tx.commit().await?;
        self.search.trigger_index_worker(self.store.clone());
        Ok(version)
    }

    pub(super) async fn resolve_version_for_project_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        project_id: Uuid,
        version_ref: Option<&str>,
    ) -> AppResult<Option<Uuid>> {
        match version_ref {
            Some(reference) => {
                let version = self.store.get_version_by_ref_tx(tx, reference).await?;
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
