use super::*;

pub(super) fn normalize_slug(value: &str) -> String {
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

pub(super) fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(super) fn require_non_empty(value: String, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppError::InvalidArguments(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(trimmed.to_string())
    }
}

pub(super) fn closed_at_for_status(
    status: TaskStatus,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    match status {
        TaskStatus::Done | TaskStatus::Cancelled => Some(now),
        _ => None,
    }
}

pub(super) fn task_ready_to_start(task: &Task, open_blocker_count: i64) -> bool {
    !matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled) && open_blocker_count == 0
}

pub(super) fn actor_or_default(value: Option<&str>, origin: RequestOrigin) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(origin.fallback_actor())
        .to_string()
}

pub(super) fn json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
}

pub(super) fn parse_uuid(value: &str, field: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value)
        .map_err(|error| AppError::InvalidArguments(format!("invalid {field}: {error}")))
}
