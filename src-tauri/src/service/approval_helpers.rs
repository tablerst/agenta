use super::*;

pub(super) fn matches_project_filter(
    request: &ApprovalRequest,
    project_slug: &str,
    project_id: Option<&str>,
) -> bool {
    request.project_ref.as_deref() == Some(project_slug)
        || project_id.is_some_and(|project_id| request.project_ref.as_deref() == Some(project_id))
}

pub(super) fn ensure_pending(request: &ApprovalRequest) -> AppResult<()> {
    if request.status == ApprovalStatus::Pending {
        Ok(())
    } else {
        Err(AppError::Conflict(format!(
            "approval request {} is already {}",
            request.request_id, request.status
        )))
    }
}

pub(super) fn error_value(app_error: &AppError) -> Value {
    json!({
        "code": app_error.code(),
        "message": app_error.message(),
        "details": app_error.details(),
    })
}
