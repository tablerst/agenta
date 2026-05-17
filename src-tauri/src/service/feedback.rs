use super::*;

impl AgentaService {
    pub async fn submit_feedback_from(
        &self,
        origin: RequestOrigin,
        mut input: SubmitFeedbackInput,
    ) -> AppResult<SubmitFeedbackResult> {
        if input
            .created_by
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            input.created_by = Some(origin.fallback_actor().to_string());
        }
        let approval = self.approval_seed(
            origin,
            input
                .project
                .clone()
                .unwrap_or_else(|| "feedback".to_string()),
            format!("Submit feedback {}", input.title.trim()),
            actor_or_default(input.created_by.as_deref(), origin),
            &input,
        )?;
        let _write_guard = self.write_queue.lock().await;
        self.submit_feedback_internal(input, ApprovalMode::Standard(approval))
            .await
    }

    async fn submit_feedback_internal(
        &self,
        input: SubmitFeedbackInput,
        mode: ApprovalMode,
    ) -> AppResult<SubmitFeedbackResult> {
        let actor = input
            .created_by
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "agent".to_string());
        let mut input = input;
        input.surface = require_non_empty(input.surface, "feedback surface")?;
        input.title = require_non_empty(input.title, "feedback title")?;
        input.friction = require_non_empty(input.friction, "feedback friction")?;
        let route = self.feedback_route_from_context_manifest()?;
        let project_ref = clean_optional(input.project.clone())
            .or_else(|| {
                route
                    .as_ref()
                    .and_then(|manifest| clean_optional(manifest.project.clone()))
            })
            .or_else(|| self.project_from_context_manifest().ok().flatten());
        let task_ref = clean_optional(input.feedback_task_id.clone()).or_else(|| {
            route
                .as_ref()
                .and_then(|manifest| clean_optional(manifest.feedback_task_id.clone()))
        });
        let task_code = clean_optional(input.feedback_task_code.clone()).or_else(|| {
            route
                .as_ref()
                .and_then(|manifest| clean_optional(manifest.feedback_task_code.clone()))
        });
        let feedback_file = route
            .as_ref()
            .and_then(|manifest| clean_optional(manifest.feedback_file.clone()));
        let project_ref = match project_ref {
            Some(value) => value,
            None if task_ref.is_none() => self.single_project_scope().await?.ok_or_else(|| {
                AppError::InvalidArguments(
                    "project is required when no feedback task is configured".to_string(),
                )
            })?,
            None => String::new(),
        };

        let (task, created_task) = self
            .resolve_or_create_feedback_task(
                project_ref,
                task_ref,
                task_code,
                input.create_task_if_missing,
                actor.clone(),
                mode.clone(),
            )
            .await?;
        let note = self
            .create_note_internal(
                CreateNoteInput {
                    task: task.task_id.to_string(),
                    content: build_feedback_note_content(&input),
                    note_kind: Some(NoteKind::Finding),
                    created_by: Some(actor),
                },
                mode,
            )
            .await?;

        Ok(SubmitFeedbackResult {
            task,
            note,
            created_task,
            feedback_file,
        })
    }

    async fn resolve_or_create_feedback_task(
        &self,
        project_ref: String,
        task_ref: Option<String>,
        task_code: Option<String>,
        create_task_if_missing: bool,
        actor: String,
        mode: ApprovalMode,
    ) -> AppResult<(Task, bool)> {
        if let Some(task_ref) = task_ref {
            return Ok((self.store.get_task_by_ref(&task_ref).await?, false));
        }

        let task_code = task_code.unwrap_or_else(|| "AgentFeedback-00".to_string());
        let matches = self
            .list_task_details(TaskQuery {
                project: Some(project_ref.clone()),
                version: None,
                status: None,
                task_kind: Some(TaskKind::Context),
                task_code_prefix: Some(task_code.clone()),
                title_prefix: None,
                sort_by: Some(TaskSortBy::CreatedAt),
                sort_order: Some(SortOrder::Desc),
                all_projects: false,
            })
            .await?;
        if let Some(detail) = matches
            .into_iter()
            .find(|detail| detail.task.task_code.as_deref() == Some(task_code.as_str()))
        {
            return Ok((detail.task, false));
        }

        if !create_task_if_missing {
            return Err(AppError::NotFound {
                entity: "feedback task".to_string(),
                reference: task_code,
            });
        }

        let task = self
            .create_task_internal(
                CreateTaskInput {
                    project: project_ref,
                    version: None,
                    task_code: Some(task_code),
                    task_kind: Some(TaskKind::Context),
                    title: "[AgentFeedback-00] Agent 使用反馈收集箱".to_string(),
                    summary: Some(
                        "Collects Agent-submitted feedback about Agenta workflow, tools, docs, and usability."
                            .to_string(),
                    ),
                    description: Some(
                        "Append finding notes here when an Agent encounters Agenta workflow friction, unclear skill guidance, tool output noise, or integration gaps."
                            .to_string(),
                    ),
                    status: Some(TaskStatus::InProgress),
                    priority: Some(TaskPriority::Normal),
                    created_by: Some(actor),
                },
                mode,
            )
            .await?;
        Ok((task, true))
    }
}

fn build_feedback_note_content(input: &SubmitFeedbackInput) -> String {
    let mut lines = vec![
        "# Agent Feedback".to_string(),
        String::new(),
        format!("- surface: {}", input.surface.trim()),
        format!(
            "- severity: {}",
            input
                .severity
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("normal")
        ),
        format!("- title: {}", input.title.trim()),
        String::new(),
        "## Friction".to_string(),
        input.friction.trim().to_string(),
    ];

    push_optional_section(&mut lines, "Expected", input.expected.as_deref());
    push_optional_section(
        &mut lines,
        "Suggested Change",
        input.suggested_change.as_deref(),
    );
    push_optional_section(&mut lines, "Evidence", input.evidence.as_deref());

    lines.join("\n")
}

fn push_optional_section(lines: &mut Vec<String>, title: &str, value: Option<&str>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    lines.push(String::new());
    lines.push(format!("## {title}"));
    lines.push(value.to_string());
}
