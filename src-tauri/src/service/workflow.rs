use super::*;

const DEFAULT_OPEN_TASK_LIMIT: usize = 10;
const MAX_OPEN_TASK_LIMIT: usize = 50;

struct WorkflowManifestState {
    path: Option<PathBuf>,
    manifest: Option<ProjectContextManifest>,
    parse_error: Option<String>,
}

impl AgentaService {
    pub async fn workflow_check(
        &self,
        mut input: WorkflowCheckInput,
    ) -> AppResult<WorkflowCheckResult> {
        input.project = clean_optional(input.project);
        input.version = clean_optional(input.version);
        input.task = clean_optional(input.task);
        input.task_code_prefix = clean_optional(input.task_code_prefix);

        let workspace_root = input
            .workspace_root
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let manifest_state = self.workflow_manifest_state(input.workspace_root.as_deref())?;
        let mut warnings = Vec::new();
        let mut missing_surfaces = Vec::new();
        let mut blocked = Vec::new();
        if let Some(error) = manifest_state.parse_error.clone() {
            warnings.push(format!("Context manifest could not be read: {error}"));
        }

        let target_task_context = match input.task.as_deref() {
            Some(task_ref) => match self
                .get_task_context_with_options(
                    task_ref,
                    TaskContextOptions {
                        recent_activity_limit: input.recent_activity_limit,
                        include_notes: false,
                        notes_limit: None,
                        include_attachments: false,
                        attachments_limit: None,
                    },
                )
                .await
            {
                Ok(context) => Some(context),
                Err(error) => {
                    blocked.push(format!("Task `{task_ref}` could not be resolved: {error}"));
                    None
                }
            },
            None => None,
        };
        let target_task = target_task_context.as_ref().map(|context| &context.task);

        let explicit_version = match input.version.as_deref() {
            Some(version_ref) => match self.get_version(version_ref).await {
                Ok(version) => Some(version),
                Err(error) => {
                    blocked.push(format!(
                        "Version `{version_ref}` could not be resolved: {error}"
                    ));
                    None
                }
            },
            None => None,
        };

        let manifest_project = manifest_state
            .manifest
            .as_ref()
            .and_then(|manifest| clean_optional(manifest.project.clone()));
        let project_ref = input
            .project
            .clone()
            .or(manifest_project)
            .or_else(|| {
                explicit_version
                    .as_ref()
                    .map(|version| version.project_id.to_string())
            })
            .or_else(|| target_task.map(|detail| detail.task.project_id.to_string()));

        let project_ref = match project_ref {
            Some(value) => Some(value),
            None => match self.single_project_scope().await {
                Ok(value) => value,
                Err(error) => {
                    blocked.push(format!("Project scope could not be inferred: {error}"));
                    None
                }
            },
        };

        let project = match project_ref.as_deref() {
            Some(reference) => match self.get_project(reference).await {
                Ok(project) => Some(project),
                Err(error) => {
                    blocked.push(format!(
                        "Project `{reference}` could not be resolved: {error}"
                    ));
                    None
                }
            },
            None => {
                missing_surfaces.push("project_scope".to_string());
                warnings.push(
                    "No project scope was provided, found in context, or inferable.".to_string(),
                );
                None
            }
        };

        let selected_version = if explicit_version.is_some() {
            explicit_version
        } else if let Some(project) = project.as_ref() {
            match project.default_version_id {
                Some(default_version_id) => {
                    match self.get_version(&default_version_id.to_string()).await {
                        Ok(version) => Some(version),
                        Err(error) => {
                            warnings.push(format!(
                            "Default version `{default_version_id}` could not be resolved: {error}"
                        ));
                            missing_surfaces.push("default_version".to_string());
                            None
                        }
                    }
                }
                None => {
                    missing_surfaces.push("default_version".to_string());
                    warnings.push("Project has no default version.".to_string());
                    if let Some(version_id) = target_task.and_then(|detail| detail.task.version_id)
                    {
                        match self.get_version(&version_id.to_string()).await {
                            Ok(version) => Some(version),
                            Err(error) => {
                                warnings.push(format!(
                                    "Target task version `{version_id}` could not be resolved: {error}"
                                ));
                                None
                            }
                        }
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        };

        if let (Some(project), Some(version)) = (project.as_ref(), selected_version.as_ref()) {
            if version.project_id != project.project_id {
                blocked.push(format!(
                    "Version `{}` does not belong to project `{}`.",
                    version.version_id, project.slug
                ));
            }
        }
        if let (Some(project), Some(task)) = (project.as_ref(), target_task) {
            if task.task.project_id != project.project_id {
                blocked.push(format!(
                    "Task `{}` does not belong to project `{}`.",
                    task.task.task_id, project.slug
                ));
            }
        }
        if input.version.is_some() {
            if let (Some(version), Some(task)) = (selected_version.as_ref(), target_task) {
                if task.task.version_id != Some(version.version_id) {
                    blocked.push(format!(
                        "Task `{}` does not belong to version `{}`.",
                        task.task.task_id, version.version_id
                    ));
                }
            }
        }

        let scoped_details = if blocked.is_empty() {
            self.collect_sorted_task_details(TaskQuery {
                project: project
                    .as_ref()
                    .map(|project| project.project_id.to_string()),
                version: selected_version
                    .as_ref()
                    .map(|version| version.version_id.to_string()),
                status: None,
                task_kind: None,
                task_code_prefix: input.task_code_prefix.clone(),
                title_prefix: None,
                sort_by: Some(TaskSortBy::TaskCode),
                sort_order: Some(SortOrder::Asc),
                all_projects: false,
            })
            .await
            .map(|(details, _, _)| details)
            .unwrap_or_else(|error| {
                warnings.push(format!("Scoped task list could not be loaded: {error}"));
                Vec::new()
            })
        } else {
            Vec::new()
        };

        if manifest_state.path.is_none() {
            missing_surfaces.push("context_manifest".to_string());
            warnings.push("No project context manifest was found for this workspace.".to_string());
        }

        if let Some(project) = project.as_ref() {
            if project.status != ProjectStatus::Active {
                warnings.push(format!("Project `{}` is not active.", project.slug));
            }
        }
        let selected_default_version = project
            .as_ref()
            .and_then(|project| project.default_version_id)
            .zip(selected_version.as_ref().map(|version| version.version_id))
            .is_some_and(|(default_id, version_id)| default_id == version_id);
        if input.version.is_none() {
            if let Some(version) = selected_version.as_ref() {
                if selected_default_version && version.status != VersionStatus::Active {
                    missing_surfaces.push("active_default_version".to_string());
                    warnings.push(format!(
                        "Default version `{}` is `{}` instead of active.",
                        version.name, version.status
                    ));
                }
            }
        }

        if let Some(task) = target_task {
            if task.note_count == 0 {
                missing_surfaces.push("task_notes".to_string());
                warnings.push(format!(
                    "Task `{}` has no notes for future recovery.",
                    task.task.task_id
                ));
            }
        }

        let mut recovery_candidates = self
            .workflow_manifest_recovery_candidates(
                manifest_state.manifest.as_ref(),
                project.as_ref(),
                selected_version.as_ref(),
                &mut warnings,
            )
            .await;
        self.workflow_scan_recovery_candidates(&scoped_details, &mut recovery_candidates);
        if recovery_candidates.is_empty() {
            missing_surfaces.push("recovery_entry".to_string());
            warnings.push("No reusable recovery task was found in the selected scope.".to_string());
        }

        let feedback_inbox = self
            .workflow_feedback_inbox(
                manifest_state.manifest.as_ref(),
                project.as_ref(),
                &mut warnings,
            )
            .await;
        if !feedback_inbox.configured {
            missing_surfaces.push("feedback_route".to_string());
            warnings.push("No feedback inbox route is configured.".to_string());
        }

        let execution_plans = if input.include_execution_plans {
            let plans =
                self.workflow_execution_plans(&workspace_root, &scoped_details, &mut warnings);
            if !plans.unlinked_plans.is_empty() {
                missing_surfaces.push("execution_plan_link".to_string());
                warnings.push(format!(
                    "{} active execution plan(s) are not linked to a task.",
                    plans.unlinked_plans.len()
                ));
            }
            plans
        } else {
            WorkflowExecutionPlans {
                included: false,
                active_plan_count: 0,
                linked_plan_count: 0,
                linked_plans: Vec::new(),
                unlinked_plans: Vec::new(),
            }
        };

        dedup_strings(&mut missing_surfaces);
        dedup_strings(&mut warnings);
        dedup_strings(&mut blocked);

        let open_tasks = workflow_open_tasks(&scoped_details, input.open_task_limit);
        let scope = WorkflowCheckScope {
            project: project.as_ref().map(workflow_project_summary),
            version: selected_version.as_ref().map(|version| {
                workflow_version_summary(
                    version,
                    project
                        .as_ref()
                        .and_then(|project| project.default_version_id)
                        == Some(version.version_id),
                )
            }),
            task: target_task_context.as_ref().map(|context| {
                workflow_task_summary(&context.task, Some(context.recent_activities.len()))
            }),
            task_code_prefix: input.task_code_prefix,
            workspace_root: Some(workspace_root.to_string_lossy().to_string()),
            context_manifest_path: manifest_state
                .path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
        };

        let surface_statuses = workflow_surface_statuses(
            &scope,
            &feedback_inbox,
            &recovery_candidates,
            &execution_plans,
            &missing_surfaces,
            !blocked.is_empty(),
        );
        let health = if !blocked.is_empty() {
            "blocked"
        } else if !missing_surfaces.is_empty() || !warnings.is_empty() {
            "attention"
        } else {
            "healthy"
        }
        .to_string();
        let mut all_warnings = blocked;
        all_warnings.extend(warnings);
        dedup_strings(&mut all_warnings);
        let recommended_next_actions =
            workflow_recommended_next_actions(&health, &missing_surfaces, &all_warnings);
        let digest = WorkflowCheckDigest {
            health: health.clone(),
            summary: workflow_digest_summary(&health, &scope, &missing_surfaces, &all_warnings),
        };

        Ok(WorkflowCheckResult {
            digest,
            scope,
            surface_statuses,
            missing_surfaces,
            warnings: all_warnings,
            recommended_next_actions,
            open_tasks,
            recovery_candidates,
            feedback_inbox,
            execution_plans,
        })
    }

    fn workflow_manifest_state(
        &self,
        workspace_root: Option<&Path>,
    ) -> AppResult<WorkflowManifestState> {
        let manifest_path = match workspace_root {
            Some(root) => self.find_context_manifest_from_base(root)?,
            None => self.find_project_context_manifest()?,
        };
        let Some(path) = manifest_path else {
            return Ok(WorkflowManifestState {
                path: None,
                manifest: None,
                parse_error: None,
            });
        };
        match self.read_project_context_manifest(&path) {
            Ok(manifest) => Ok(WorkflowManifestState {
                path: Some(path),
                manifest: Some(manifest),
                parse_error: None,
            }),
            Err(error) => Ok(WorkflowManifestState {
                path: Some(path),
                manifest: None,
                parse_error: Some(error.to_string()),
            }),
        }
    }

    async fn workflow_manifest_recovery_candidates(
        &self,
        manifest: Option<&ProjectContextManifest>,
        project: Option<&Project>,
        version: Option<&Version>,
        warnings: &mut Vec<String>,
    ) -> Vec<WorkflowRecoveryCandidate> {
        let mut candidates = Vec::new();
        let Some(manifest) = manifest else {
            return candidates;
        };
        if let Some(task_id) = clean_optional(manifest.entry_task_id.clone()) {
            match self.get_task_detail(&task_id).await {
                Ok(detail) => candidates.push(WorkflowRecoveryCandidate {
                    source: "context_manifest".to_string(),
                    reason: "entry_task_id".to_string(),
                    task: workflow_task_summary(&detail, None),
                }),
                Err(error) => warnings.push(format!(
                    "Configured entry_task_id `{task_id}` could not be resolved: {error}"
                )),
            }
            return candidates;
        }
        if let Some(task_code) = clean_optional(manifest.entry_task_code.clone()) {
            let matches = self
                .collect_sorted_task_details(TaskQuery {
                    project: project.map(|project| project.project_id.to_string()),
                    version: version.map(|version| version.version_id.to_string()),
                    status: None,
                    task_kind: None,
                    task_code_prefix: Some(task_code.clone()),
                    title_prefix: None,
                    sort_by: Some(TaskSortBy::TaskCode),
                    sort_order: Some(SortOrder::Asc),
                    all_projects: false,
                })
                .await;
            match matches {
                Ok((details, _, _)) => {
                    if let Some(detail) = details
                        .into_iter()
                        .find(|detail| detail.task.task_code.as_deref() == Some(task_code.as_str()))
                    {
                        candidates.push(WorkflowRecoveryCandidate {
                            source: "context_manifest".to_string(),
                            reason: "entry_task_code".to_string(),
                            task: workflow_task_summary(&detail, None),
                        });
                    } else {
                        warnings.push(format!(
                            "Configured entry_task_code `{task_code}` did not match a task."
                        ));
                    }
                }
                Err(error) => warnings.push(format!(
                    "Configured entry_task_code `{task_code}` could not be checked: {error}"
                )),
            }
        }
        candidates
    }

    fn workflow_scan_recovery_candidates(
        &self,
        details: &[TaskDetail],
        candidates: &mut Vec<WorkflowRecoveryCandidate>,
    ) {
        for detail in details {
            if candidates
                .iter()
                .any(|candidate| candidate.task.task_id == detail.task.task_id.to_string())
            {
                continue;
            }
            let reason = if detail.task.task_kind == TaskKind::Index {
                Some("index_task")
            } else if detail.task.task_kind == TaskKind::Context {
                Some("context_task")
            } else if detail.task.knowledge_status == KnowledgeStatus::Reusable {
                Some("reusable_knowledge")
            } else {
                None
            };
            if let Some(reason) = reason {
                candidates.push(WorkflowRecoveryCandidate {
                    source: "task_scan".to_string(),
                    reason: reason.to_string(),
                    task: workflow_task_summary(detail, None),
                });
            }
            if candidates.len() >= 5 {
                break;
            }
        }
    }

    async fn workflow_feedback_inbox(
        &self,
        manifest: Option<&ProjectContextManifest>,
        project: Option<&Project>,
        warnings: &mut Vec<String>,
    ) -> WorkflowFeedbackInbox {
        let task_ref =
            manifest.and_then(|manifest| clean_optional(manifest.feedback_task_id.clone()));
        let task_code =
            manifest.and_then(|manifest| clean_optional(manifest.feedback_task_code.clone()));
        let feedback_file =
            manifest.and_then(|manifest| clean_optional(manifest.feedback_file.clone()));
        let mut configured = task_ref.is_some() || task_code.is_some() || feedback_file.is_some();
        let mut task = None;

        if let Some(task_ref) = task_ref {
            match self.get_task_detail(&task_ref).await {
                Ok(detail) => task = Some(workflow_task_summary(&detail, None)),
                Err(error) => warnings.push(format!(
                    "Configured feedback_task_id `{task_ref}` could not be resolved: {error}"
                )),
            }
        } else if let (Some(project), Some(task_code)) = (project, task_code.as_ref()) {
            match self
                .collect_sorted_task_details(TaskQuery {
                    project: Some(project.project_id.to_string()),
                    version: None,
                    status: None,
                    task_kind: Some(TaskKind::Context),
                    task_code_prefix: Some(task_code.clone()),
                    title_prefix: None,
                    sort_by: Some(TaskSortBy::TaskCode),
                    sort_order: Some(SortOrder::Asc),
                    all_projects: false,
                })
                .await
            {
                Ok((details, _, _)) => {
                    task = details
                        .into_iter()
                        .find(|detail| detail.task.task_code.as_deref() == Some(task_code.as_str()))
                        .map(|detail| workflow_task_summary(&detail, None));
                }
                Err(error) => warnings.push(format!(
                    "Configured feedback_task_code `{task_code}` could not be checked: {error}"
                )),
            }
        }

        if configured && task.is_none() && feedback_file.is_none() {
            warnings.push(
                "Feedback route is configured but no feedback inbox task was found.".to_string(),
            );
        }
        if !configured {
            configured = false;
        }

        WorkflowFeedbackInbox {
            configured,
            task,
            feedback_file,
            source: configured.then(|| "context_manifest".to_string()),
        }
    }

    fn workflow_execution_plans(
        &self,
        workspace_root: &Path,
        details: &[TaskDetail],
        warnings: &mut Vec<String>,
    ) -> WorkflowExecutionPlans {
        let active_dir = workspace_root
            .join("dev_docs")
            .join("execution-plans")
            .join("active");
        let mut linked_plans = Vec::new();
        let mut unlinked_plans = Vec::new();
        if !active_dir.is_dir() {
            return WorkflowExecutionPlans {
                included: true,
                active_plan_count: 0,
                linked_plan_count: 0,
                linked_plans,
                unlinked_plans,
            };
        }

        let entries = match std_fs::read_dir(&active_dir) {
            Ok(entries) => entries,
            Err(error) => {
                warnings.push(format!(
                    "Active execution plan directory could not be read: {error}"
                ));
                return WorkflowExecutionPlans {
                    included: true,
                    active_plan_count: 0,
                    linked_plan_count: 0,
                    linked_plans,
                    unlinked_plans,
                };
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("md") {
                continue;
            }
            let relative = path
                .strip_prefix(workspace_root)
                .unwrap_or(path.as_path())
                .to_string_lossy()
                .replace('\\', "/");
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            if workflow_plan_is_linked(&relative, &stem, details) {
                linked_plans.push(relative);
            } else {
                unlinked_plans.push(relative);
            }
        }
        linked_plans.sort();
        unlinked_plans.sort();
        WorkflowExecutionPlans {
            included: true,
            active_plan_count: linked_plans.len() + unlinked_plans.len(),
            linked_plan_count: linked_plans.len(),
            linked_plans,
            unlinked_plans,
        }
    }
}

fn workflow_project_summary(project: &Project) -> WorkflowProjectSummary {
    WorkflowProjectSummary {
        project_id: project.project_id.to_string(),
        slug: project.slug.clone(),
        name: project.name.clone(),
        status: project.status.to_string(),
        default_version_id: project.default_version_id.map(|value| value.to_string()),
    }
}

fn workflow_version_summary(version: &Version, is_default: bool) -> WorkflowVersionSummary {
    WorkflowVersionSummary {
        version_id: version.version_id.to_string(),
        project_id: version.project_id.to_string(),
        name: version.name.clone(),
        status: version.status.to_string(),
        is_default,
    }
}

fn workflow_task_summary(
    detail: &TaskDetail,
    recent_activity_count: Option<usize>,
) -> WorkflowTaskSummary {
    WorkflowTaskSummary {
        task_id: detail.task.task_id.to_string(),
        project_id: detail.task.project_id.to_string(),
        version_id: detail.task.version_id.map(|value| value.to_string()),
        task_code: detail.task.task_code.clone(),
        task_kind: detail.task.task_kind.to_string(),
        title: detail.task.title.clone(),
        status: detail.task.status.to_string(),
        knowledge_status: detail.task.knowledge_status.to_string(),
        note_count: detail.note_count,
        latest_note_summary: detail.task.latest_note_summary.clone(),
        task_context_digest: detail.task.task_context_digest.clone(),
        ready_to_start: detail.ready_to_start,
        recent_activity_count,
    }
}

fn workflow_open_tasks(
    details: &[TaskDetail],
    requested_limit: Option<usize>,
) -> WorkflowOpenTasks {
    let limit = requested_limit
        .unwrap_or(DEFAULT_OPEN_TASK_LIMIT)
        .clamp(1, MAX_OPEN_TASK_LIMIT);
    let open = details
        .iter()
        .filter(|detail| !matches!(detail.task.status, TaskStatus::Done | TaskStatus::Cancelled))
        .collect::<Vec<_>>();
    WorkflowOpenTasks {
        total: open.len(),
        ready_to_start_count: open.iter().filter(|detail| detail.ready_to_start).count(),
        in_progress_count: open
            .iter()
            .filter(|detail| detail.task.status == TaskStatus::InProgress)
            .count(),
        blocked_count: open
            .iter()
            .filter(|detail| detail.task.status == TaskStatus::Blocked)
            .count(),
        limit_applied: limit,
        tasks: open
            .into_iter()
            .take(limit)
            .map(|detail| workflow_task_summary(detail, None))
            .collect(),
    }
}

fn workflow_surface_statuses(
    scope: &WorkflowCheckScope,
    feedback_inbox: &WorkflowFeedbackInbox,
    recovery_candidates: &[WorkflowRecoveryCandidate],
    execution_plans: &WorkflowExecutionPlans,
    missing_surfaces: &[String],
    blocked: bool,
) -> Vec<WorkflowSurfaceStatus> {
    let status_for = |surface: &str| {
        if blocked {
            "blocked"
        } else if missing_surfaces.iter().any(|value| value == surface) {
            "missing"
        } else {
            "ok"
        }
    };
    let mut statuses = vec![
        WorkflowSurfaceStatus {
            surface: "context_manifest".to_string(),
            status: status_for("context_manifest").to_string(),
            summary: scope
                .context_manifest_path
                .clone()
                .unwrap_or_else(|| "not found".to_string()),
        },
        WorkflowSurfaceStatus {
            surface: "project_scope".to_string(),
            status: status_for("project_scope").to_string(),
            summary: scope
                .project
                .as_ref()
                .map(|project| project.slug.clone())
                .unwrap_or_else(|| "not resolved".to_string()),
        },
        WorkflowSurfaceStatus {
            surface: "version_scope".to_string(),
            status: if blocked {
                "blocked"
            } else if missing_surfaces
                .iter()
                .any(|value| value == "default_version" || value == "active_default_version")
            {
                "attention"
            } else {
                "ok"
            }
            .to_string(),
            summary: scope
                .version
                .as_ref()
                .map(|version| format!("{} ({})", version.name, version.status))
                .unwrap_or_else(|| "not resolved".to_string()),
        },
        WorkflowSurfaceStatus {
            surface: "recovery_entry".to_string(),
            status: status_for("recovery_entry").to_string(),
            summary: format!("{} candidate(s)", recovery_candidates.len()),
        },
        WorkflowSurfaceStatus {
            surface: "feedback_route".to_string(),
            status: status_for("feedback_route").to_string(),
            summary: if feedback_inbox.configured {
                "configured".to_string()
            } else {
                "not configured".to_string()
            },
        },
    ];
    if scope.task.is_some() {
        statuses.push(WorkflowSurfaceStatus {
            surface: "task_readback".to_string(),
            status: status_for("task_notes").to_string(),
            summary: scope
                .task
                .as_ref()
                .map(|task| format!("{} note(s)", task.note_count))
                .unwrap_or_else(|| "not checked".to_string()),
        });
    }
    if execution_plans.included {
        statuses.push(WorkflowSurfaceStatus {
            surface: "execution_plan_link".to_string(),
            status: status_for("execution_plan_link").to_string(),
            summary: format!(
                "{} linked / {} active",
                execution_plans.linked_plan_count, execution_plans.active_plan_count
            ),
        });
    }
    statuses
}

fn workflow_recommended_next_actions(
    health: &str,
    missing_surfaces: &[String],
    warnings: &[String],
) -> Vec<String> {
    let mut actions = Vec::new();
    if health == "blocked" {
        actions
            .push("Resolve invalid workflow_check scope references before continuing.".to_string());
    }
    if missing_surfaces
        .iter()
        .any(|value| value == "context_manifest")
    {
        actions.push("Run context_init or `agenta context init` for this workspace.".to_string());
    }
    if missing_surfaces
        .iter()
        .any(|value| value == "default_version" || value == "active_default_version")
    {
        actions.push("Set an active default version before starting new work.".to_string());
    }
    if missing_surfaces
        .iter()
        .any(|value| value == "recovery_entry")
    {
        actions.push(
            "Create or update a context/index task with reusable recovery notes.".to_string(),
        );
    }
    if missing_surfaces
        .iter()
        .any(|value| value == "feedback_route")
    {
        actions.push(
            "Configure feedback_task_code or feedback_file in `.agenta/project.yaml`.".to_string(),
        );
    }
    if missing_surfaces.iter().any(|value| value == "task_notes") {
        actions.push(
            "Append a finding or conclusion note to the target task, then read it back."
                .to_string(),
        );
    }
    if missing_surfaces
        .iter()
        .any(|value| value == "execution_plan_link")
    {
        actions.push(
            "Mention the active execution plan path in the relevant task or note.".to_string(),
        );
    }
    if actions.is_empty() && warnings.is_empty() {
        actions.push(
            "Proceed with the requested work; close out with ledger_delta and readback."
                .to_string(),
        );
    }
    dedup_strings(&mut actions);
    actions
}

fn workflow_digest_summary(
    health: &str,
    scope: &WorkflowCheckScope,
    missing_surfaces: &[String],
    warnings: &[String],
) -> String {
    let project = scope
        .project
        .as_ref()
        .map(|project| project.slug.as_str())
        .unwrap_or("unresolved");
    let version = scope
        .version
        .as_ref()
        .map(|version| version.name.as_str())
        .unwrap_or("unresolved");
    if health == "healthy" {
        return format!(
            "Workflow ledger is healthy for project `{project}` on version `{version}`."
        );
    }
    let gap_count = missing_surfaces.len();
    let warning_count = warnings.len();
    format!(
        "Workflow ledger needs {health}: project `{project}`, version `{version}`, {gap_count} missing surface(s), {warning_count} warning(s)."
    )
}

fn workflow_plan_is_linked(relative: &str, stem: &str, details: &[TaskDetail]) -> bool {
    let relative_lower = relative.to_lowercase();
    let stem_lower = stem.to_lowercase();
    let normalized_stem = normalize_plan_token(stem);
    let base_stem = strip_plan_version_suffix(stem);
    let normalized_base = normalize_plan_token(&base_stem);
    details.iter().any(|detail| {
        let text = format!(
            "{} {} {} {} {} {}",
            detail.task.task_code.as_deref().unwrap_or_default(),
            detail.task.title,
            detail.task.summary.as_deref().unwrap_or_default(),
            detail.task.description.as_deref().unwrap_or_default(),
            detail
                .task
                .latest_note_summary
                .as_deref()
                .unwrap_or_default(),
            detail.task.task_search_summary
        )
        .to_lowercase();
        let normalized_text = normalize_plan_token(&text);
        text.contains(&relative_lower)
            || text.contains(&stem_lower)
            || (!normalized_stem.is_empty() && normalized_text.contains(&normalized_stem))
            || (!normalized_base.is_empty() && normalized_text.contains(&normalized_base))
    })
}

fn strip_plan_version_suffix(stem: &str) -> String {
    let Some((base, suffix)) = stem.rsplit_once("-v") else {
        return stem.to_string();
    };
    if !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
        base.to_string()
    } else {
        stem.to_string()
    }
}

fn normalize_plan_token(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn dedup_strings(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}
