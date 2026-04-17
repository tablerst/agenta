mod runtime;

use serde::Serialize;

use crate::domain::{Task, TaskActivityKind};

pub use runtime::{
    SearchRuntime, SearchRuntimeStatus, SearchSidecarStatus, SearchVectorJob, VectorQueryHit,
};

pub const DEFAULT_SEARCH_LIMIT: usize = 10;
pub const MAX_SEARCH_LIMIT: usize = 50;
pub const DEFAULT_VECTOR_COLLECTION: &str = "agenta_tasks_v1";
pub const DEFAULT_VECTOR_ENDPOINT: &str = "http://127.0.0.1:8000";
pub const DEFAULT_VECTOR_TOP_K: usize = 40;
pub const DEFAULT_EMBEDDING_TIMEOUT_MS: u64 = 10_000;
pub const DEFAULT_RRF_K: usize = 60;
pub const LEXICAL_RRF_WEIGHT: f64 = 1.0;
pub const SEMANTIC_RRF_WEIGHT: f64 = 0.75;

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
    pub retrieval_source: String,
    pub score: Option<f64>,
    pub matched_fields: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActivitySearchHit {
    pub activity_id: String,
    pub task_id: String,
    pub kind: String,
    pub summary: String,
    pub score: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchIndexedFields {
    pub tasks: Vec<String>,
    pub activities: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchMeta {
    pub indexed_fields: SearchIndexedFields,
    pub task_sort: String,
    pub activity_sort: String,
    pub limit_applies_per_bucket: bool,
    pub task_limit_applied: usize,
    pub activity_limit_applied: usize,
    pub default_limit: usize,
    pub max_limit: usize,
    pub retrieval_mode: String,
    pub vector_backend: Option<String>,
    pub vector_status: String,
    pub pending_index_jobs: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResponse {
    pub query: Option<String>,
    pub tasks: Vec<TaskSearchHit>,
    pub activities: Vec<ActivitySearchHit>,
    pub meta: SearchMeta,
}

#[derive(Clone, Debug)]
pub struct TaskVectorDocument {
    pub task_id: String,
    pub project_id: String,
    pub project_slug: String,
    pub project_name: String,
    pub project_description: Option<String>,
    pub version_id: Option<String>,
    pub version_name: Option<String>,
    pub version_description: Option<String>,
    pub task_code: Option<String>,
    pub task_kind: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub knowledge_status: String,
    pub latest_note_summary: Option<String>,
    pub latest_attachment_summary: Option<String>,
    pub task_search_summary: String,
    pub task_context_digest: String,
    pub updated_at: String,
    pub document: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedSearchQuery {
    pub raw_text: String,
    pub fts_query: String,
    pub terms: Vec<String>,
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

pub fn build_task_vector_document_text(
    project_slug: &str,
    project_name: &str,
    project_description: Option<&str>,
    version_name: Option<&str>,
    version_description: Option<&str>,
    task_code: Option<&str>,
    title: &str,
    latest_note_summary: Option<&str>,
    latest_attachment_summary: Option<&str>,
    task_search_summary: &str,
    task_context_digest: &str,
) -> String {
    let mut parts = Vec::new();
    let project_label = [project_slug.trim(), project_name.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    if !project_label.is_empty() {
        parts.push(format!("project {project_label}"));
    }
    if let Some(project_description) = project_description.filter(|value| !value.trim().is_empty())
    {
        parts.push(format!(
            "project description {}",
            project_description.trim()
        ));
    }
    if let Some(version_name) = version_name.filter(|value| !value.trim().is_empty()) {
        parts.push(format!("version {}", version_name.trim()));
    }
    if let Some(version_description) = version_description.filter(|value| !value.trim().is_empty())
    {
        parts.push(format!(
            "version description {}",
            version_description.trim()
        ));
    }
    if let Some(task_code) = task_code.filter(|value| !value.trim().is_empty()) {
        parts.push(task_code.trim().to_owned());
    }
    parts.push(title.trim().to_owned());
    if let Some(latest_note_summary) = latest_note_summary.filter(|value| !value.trim().is_empty())
    {
        parts.push(latest_note_summary.trim().to_owned());
    }
    if let Some(latest_attachment_summary) =
        latest_attachment_summary.filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("attachment {}", latest_attachment_summary.trim()));
    }
    if !task_search_summary.trim().is_empty() {
        parts.push(task_search_summary.trim().to_owned());
    }
    if !task_context_digest.trim().is_empty() {
        parts.push(task_context_digest.trim().to_owned());
    }
    truncate(parts.join("\n"), 2_000)
}

pub fn normalize_search_query(value: &str) -> Option<NormalizedSearchQuery> {
    let raw_text = value.trim();
    if raw_text.is_empty() {
        return None;
    }

    let terms = raw_text
        .split_whitespace()
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(|term| term.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return None;
    }

    let fts_query = terms
        .iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ");

    Some(NormalizedSearchQuery {
        raw_text: raw_text.to_string(),
        fts_query,
        terms,
    })
}

pub fn matched_field_names<'a>(
    terms: &[String],
    fields: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) -> Vec<String> {
    fields
        .into_iter()
        .filter_map(|(field_name, value)| {
            value
                .filter(|candidate| contains_any_term(candidate, terms))
                .map(|_| field_name.to_string())
        })
        .collect()
}

pub fn contains_any_term(value: &str, terms: &[String]) -> bool {
    let normalized = value.to_ascii_lowercase();
    terms.iter().any(|term| normalized.contains(term))
}

pub fn weighted_rrf_score(rank_index: usize, weight: f64) -> f64 {
    weight / (DEFAULT_RRF_K as f64 + rank_index as f64 + 1.0)
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
    use super::{
        build_activity_search_summary, build_task_vector_document_text, normalize_search_query,
        weighted_rrf_score,
    };
    use crate::domain::TaskActivityKind;

    #[test]
    fn trims_large_activity_content() {
        let summary = build_activity_search_summary(TaskActivityKind::Note, &"x".repeat(400));
        assert!(summary.len() < 300);
    }

    #[test]
    fn normalizes_multi_term_queries_for_fts() {
        let normalized =
            normalize_search_query("InitCtx-1 reusable conclusion").expect("normalized query");
        assert_eq!(normalized.raw_text, "InitCtx-1 reusable conclusion");
        assert_eq!(
            normalized.fts_query,
            "\"initctx-1\" AND \"reusable\" AND \"conclusion\""
        );
        assert_eq!(
            normalized.terms,
            vec!["initctx-1", "reusable", "conclusion"]
        );
    }

    #[test]
    fn keeps_non_ascii_terms_during_normalization() {
        let normalized = normalize_search_query("搜索 中文").expect("normalized query");
        assert_eq!(normalized.fts_query, "\"搜索\" AND \"中文\"");
    }

    #[test]
    fn vector_document_rollup_stays_compact() {
        let document = build_task_vector_document_text(
            "workspace-alpha",
            "Workspace Alpha",
            Some("Primary workspace"),
            Some("Alpha v1"),
            Some("Initial release lane"),
            Some("InitCtx-1"),
            "Reusable task",
            Some("Conclusion note"),
            Some("System architecture.md"),
            &"summary ".repeat(50),
            &"digest ".repeat(50),
        );
        assert!(document.len() < 2_500);
    }

    #[test]
    fn weighted_rrf_prefers_better_ranks() {
        assert!(weighted_rrf_score(0, 1.0) > weighted_rrf_score(4, 1.0));
        assert!(weighted_rrf_score(0, 1.0) > weighted_rrf_score(0, 0.75));
    }
}
