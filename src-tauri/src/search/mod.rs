use serde::Serialize;

use crate::domain::{Task, TaskActivityKind};

const SUMMARY_LIMIT: usize = 240;
const DIGEST_LIMIT: usize = 320;

#[derive(Clone, Debug, Serialize)]
pub struct TaskSearchHit {
    pub task_id: String,
    pub task_code: Option<String>,
    pub task_kind: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub knowledge_status: String,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActivitySearchHit {
    pub activity_id: String,
    pub task_id: String,
    pub kind: String,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResponse {
    pub query: Option<String>,
    pub tasks: Vec<TaskSearchHit>,
    pub activities: Vec<ActivitySearchHit>,
}

pub fn build_task_search_summary(
    task_code: Option<&str>,
    task_kind: impl std::fmt::Display,
    title: &str,
    summary: Option<&str>,
    description: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    if let Some(task_code) = task_code.filter(|value| !value.trim().is_empty()) {
        parts.push(task_code.trim().to_owned());
    }
    parts.push(task_kind.to_string());
    parts.push(title.trim().to_owned());
    if let Some(summary) = summary.filter(|value| !value.trim().is_empty()) {
        parts.push(summary.trim().to_owned());
    }
    if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
        parts.push(description.trim().to_owned());
    }
    truncate(parts.join(" | "), SUMMARY_LIMIT)
}

pub fn build_task_context_digest(task: &Task) -> String {
    let digest = format!(
        "status={} priority={} task_code={} task_kind={} knowledge_status={} latest_note_summary={} title={} summary={} description={}",
        task.status,
        task.priority,
        task.task_code.as_deref().unwrap_or(""),
        task.task_kind,
        task.knowledge_status,
        task.latest_note_summary.as_deref().unwrap_or(""),
        task.title,
        task.summary.as_deref().unwrap_or(""),
        task.description.as_deref().unwrap_or("")
    );
    truncate(digest, DIGEST_LIMIT)
}

pub fn build_activity_search_summary(kind: TaskActivityKind, content: &str) -> String {
    truncate(format!("{kind}: {}", content.trim()), SUMMARY_LIMIT)
}

fn truncate(value: String, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value;
    }

    let mut output = value
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>();
    output.push_str("...");
    output
}

#[cfg(test)]
mod tests {
    use super::build_activity_search_summary;
    use crate::domain::TaskActivityKind;

    #[test]
    fn trims_large_activity_content() {
        let summary = build_activity_search_summary(TaskActivityKind::Note, &"x".repeat(400));
        assert!(summary.len() < 300);
    }
}
