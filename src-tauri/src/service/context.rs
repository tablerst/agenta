use super::*;

impl AgentaService {
    pub async fn init_project_context(
        &self,
        input: ContextInitInput,
    ) -> AppResult<ContextInitResult> {
        let _write_guard = self.write_queue.lock().await;

        let target = self.resolve_context_init_target(
            input.workspace_root.as_deref(),
            input.context_dir.as_deref(),
        )?;
        let existing_manifest = if target.manifest_path.is_file() {
            Some(self.read_project_context_manifest(&target.manifest_path)?)
        } else {
            None
        };
        let project = self
            .resolve_context_init_project(input.project.as_deref(), existing_manifest.as_ref())
            .await?;

        let instructions = input
            .instructions
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("README.md")
            .to_string();
        let memory_dir = input
            .memory_dir
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("memory")
            .to_string();
        let desired_manifest = ProjectContextManifest {
            project: Some(project.clone()),
            instructions: Some(instructions.clone()),
            memory_dir: Some(memory_dir.clone()),
        };
        let used_defaults = input.context_dir.is_none()
            || input.instructions.is_none()
            || input.memory_dir.is_none();

        let status = match existing_manifest {
            Some(existing) => {
                let unchanged = manifests_match(&existing, &desired_manifest);
                if unchanged {
                    ContextInitStatus::Unchanged
                } else if input.dry_run {
                    ContextInitStatus::WouldUpdate
                } else if input.force {
                    self.write_project_context(
                        &target.context_dir,
                        &target.manifest_path,
                        &desired_manifest,
                    )
                    .await?;
                    ContextInitStatus::Updated
                } else {
                    return Err(AppError::Conflict(
                        "context manifest already exists with different values; pass force to update"
                            .to_string(),
                    ));
                }
            }
            None => {
                if input.dry_run {
                    ContextInitStatus::WouldCreate
                } else {
                    self.write_project_context(
                        &target.context_dir,
                        &target.manifest_path,
                        &desired_manifest,
                    )
                    .await?;
                    ContextInitStatus::Created
                }
            }
        };

        Ok(ContextInitResult {
            project,
            context_dir: target.context_dir,
            manifest_path: target.manifest_path,
            status,
            used_defaults,
        })
    }

    pub(super) async fn resolve_context_init_project(
        &self,
        explicit_project: Option<&str>,
        existing_manifest: Option<&ProjectContextManifest>,
    ) -> AppResult<String> {
        if let Some(project) = explicit_project
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(project.to_string());
        }
        if let Some(project) = existing_manifest
            .and_then(|manifest| manifest.project.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(project.to_string());
        }
        self.single_project_scope().await?.ok_or_else(|| {
            AppError::AmbiguousContext(
                "project must be provided when multiple projects are available".to_string(),
            )
        })
    }

    pub(super) fn resolve_context_init_target(
        &self,
        workspace_root: Option<&Path>,
        context_dir: Option<&Path>,
    ) -> AppResult<ContextInitTarget> {
        if let Some(context_dir) = context_dir {
            let base_dir = workspace_root
                .map(Path::to_path_buf)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let resolved_context_dir = if context_dir.is_absolute() {
                context_dir.to_path_buf()
            } else {
                base_dir.join(context_dir)
            };
            let manifest_path = resolved_context_dir.join(&self.project_context.manifest);
            return Ok(ContextInitTarget {
                context_dir: resolved_context_dir,
                manifest_path,
            });
        }

        let workspace_root = workspace_root
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        if let Some(manifest_path) = self.find_context_manifest_from_base(&workspace_root)? {
            let context_dir = manifest_path
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| {
                    AppError::Config("context manifest must have a parent directory".to_string())
                })?;
            return Ok(ContextInitTarget {
                context_dir,
                manifest_path,
            });
        }

        let candidate = self.project_context.paths.first().cloned().ok_or_else(|| {
            AppError::Config("project_context.paths must not be empty".to_string())
        })?;
        let context_dir = if candidate.is_absolute() {
            candidate
        } else {
            workspace_root.join(candidate)
        };
        let manifest_path = context_dir.join(&self.project_context.manifest);
        Ok(ContextInitTarget {
            context_dir,
            manifest_path,
        })
    }

    pub(super) async fn write_project_context(
        &self,
        context_dir: &Path,
        manifest_path: &Path,
        manifest: &ProjectContextManifest,
    ) -> AppResult<()> {
        fs::create_dir_all(context_dir).await?;
        if let Some(memory_dir) = manifest
            .memory_dir
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            fs::create_dir_all(context_dir.join(memory_dir)).await?;
        }
        let serialized = serde_yaml::to_string(manifest)?;
        fs::write(manifest_path, serialized)
            .await
            .map_err(AppError::from)
    }

    pub(super) fn read_project_context_manifest(
        &self,
        manifest_path: &Path,
    ) -> AppResult<ProjectContextManifest> {
        let content = std_fs::read_to_string(manifest_path).map_err(AppError::from)?;
        serde_yaml::from_str::<ProjectContextManifest>(&content).map_err(AppError::from)
    }

    pub(super) async fn resolve_project_scope(
        &self,
        explicit_project: Option<&str>,
        version_ref: Option<&str>,
        all_projects: bool,
    ) -> AppResult<Option<String>> {
        if let Some(project) = explicit_project
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(Some(project.to_string()));
        }
        if all_projects || version_ref.is_some_and(|value| !value.trim().is_empty()) {
            return Ok(None);
        }
        if let Some(project) = self.project_from_context_manifest()? {
            return Ok(Some(project));
        }
        self.single_project_scope().await
    }

    pub(super) async fn single_project_scope(&self) -> AppResult<Option<String>> {
        let projects = self.store.list_projects().await?;
        if projects.is_empty() {
            return Ok(None);
        }
        let active_projects = projects
            .iter()
            .filter(|project| project.status == ProjectStatus::Active)
            .collect::<Vec<_>>();
        if active_projects.len() == 1 {
            return Ok(Some(active_projects[0].slug.clone()));
        }
        if active_projects.is_empty() && projects.len() == 1 {
            return Ok(Some(projects[0].slug.clone()));
        }
        Err(AppError::AmbiguousContext(
            "multiple projects are available; pass project explicitly or set all_projects=true"
                .to_string(),
        ))
    }

    pub(super) fn project_from_context_manifest(&self) -> AppResult<Option<String>> {
        let Some(manifest_path) = self.find_project_context_manifest()? else {
            return Ok(None);
        };
        let manifest = self.read_project_context_manifest(&manifest_path)?;
        Ok(manifest.project.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }))
    }

    pub(super) fn find_project_context_manifest(&self) -> AppResult<Option<PathBuf>> {
        let current_dir = std::env::current_dir().map_err(AppError::from)?;
        self.find_context_manifest_from_ancestors(&current_dir)
    }

    pub(super) fn find_context_manifest_from_base(
        &self,
        base_dir: &Path,
    ) -> AppResult<Option<PathBuf>> {
        for context_path in &self.project_context.paths {
            let context_dir = if context_path.is_absolute() {
                context_path.clone()
            } else {
                base_dir.join(context_path)
            };
            let manifest_path = context_dir.join(&self.project_context.manifest);
            if manifest_path.is_file() {
                return Ok(Some(manifest_path));
            }
        }
        Ok(None)
    }

    pub(super) fn find_context_manifest_from_ancestors(
        &self,
        current_dir: &Path,
    ) -> AppResult<Option<PathBuf>> {
        for base_dir in current_dir.ancestors() {
            if let Some(manifest_path) = self.find_context_manifest_from_base(base_dir)? {
                return Ok(Some(manifest_path));
            }
        }
        Ok(None)
    }
}
