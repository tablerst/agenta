use super::*;

impl AgentaService {
    pub async fn list_approval_requests(
        &self,
        query: ApprovalQuery,
    ) -> AppResult<Vec<ApprovalRequest>> {
        let project_scope = self
            .resolve_project_scope(query.project.as_deref(), None, query.all_projects)
            .await?;
        let (project_slug_filter, project_id_filter) = match project_scope.as_deref() {
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
        let _write_guard = self.write_queue.lock().await;
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
        let _write_guard = self.write_queue.lock().await;
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

    pub(super) async fn enforce(&self, action: &str, mode: ApprovalMode) -> AppResult<()> {
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

    pub(super) fn approval_seed(
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

    pub(super) async fn enrich_approval_request(
        &self,
        mut request: ApprovalRequest,
    ) -> ApprovalRequest {
        let context = self.resolve_approval_context(&request).await;
        request.project_ref = context.project_ref;
        request.project_name = context.project_name;
        request.task_ref = context.task_ref;
        request
    }

    pub(super) async fn resolve_approval_context(
        &self,
        request: &ApprovalRequest,
    ) -> ApprovalContext {
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
            "task.create_child" | "task.attach_child" | "task.detach_child" => {
                let task_ref = json_string(&request.payload_json, "parent")
                    .or_else(|| json_string(&request.payload_json, "child"))
                    .unwrap_or_else(|| request.resource_ref.clone());
                self.task_context_from_reference(&task_ref).await
            }
            "task.add_blocker" | "task.resolve_blocker" => {
                let task_ref = json_string(&request.payload_json, "blocked")
                    .or_else(|| json_string(&request.payload_json, "task"))
                    .unwrap_or_else(|| request.resource_ref.clone());
                self.task_context_from_reference(&task_ref).await
            }
            "note.create" | "attachment.create" => {
                let task_ref = json_string(&request.payload_json, "task")
                    .unwrap_or_else(|| request.resource_ref.clone());
                self.task_context_from_reference(&task_ref).await
            }
            _ => ApprovalContext::default(),
        }
    }

    pub(super) async fn project_context_from_reference(
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

    pub(super) async fn version_context_from_reference(&self, reference: &str) -> ApprovalContext {
        let Ok(version) = self.store.get_version_by_ref(reference).await else {
            return ApprovalContext::default();
        };

        self.project_context_from_reference(Some(version.project_id.to_string()), None)
            .await
    }

    pub(super) async fn task_context_from_reference(&self, reference: &str) -> ApprovalContext {
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

    pub(super) async fn replay_approval_request(
        &self,
        request: &ApprovalRequest,
    ) -> AppResult<Value> {
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
            "task.create_child" => {
                let input =
                    serde_json::from_value::<CreateChildTaskInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.create_child_task_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.attach_child" => {
                let input =
                    serde_json::from_value::<AttachChildTaskInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.attach_child_task_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.detach_child" => {
                let input =
                    serde_json::from_value::<DetachChildTaskInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.detach_child_task_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.add_blocker" => {
                let input =
                    serde_json::from_value::<AddTaskBlockerInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.add_task_blocker_internal(input, ApprovalMode::Replay)
                        .await?,
                )
                .map_err(|error| {
                    AppError::internal(format!("failed to serialize replay result: {error}"))
                })
            }
            "task.resolve_blocker" => {
                let input =
                    serde_json::from_value::<ResolveTaskBlockerInput>(request.payload_json.clone())
                        .map_err(|error| {
                            AppError::InvalidArguments(format!("invalid approval payload: {error}"))
                        })?;
                serde_json::to_value(
                    self.resolve_task_blocker_internal(input, ApprovalMode::Replay)
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
}
