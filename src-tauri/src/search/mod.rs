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
const ACTIVITY_TEXT_LIMIT: usize = 6_000;
const ACTIVITY_CHUNK_TARGET_CHARS: usize = 900;
const ACTIVITY_CHUNK_OVERLAP_CHARS: usize = 180;

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
    pub evidence_source: Option<String>,
    pub evidence_snippet: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActivitySearchHit {
    pub activity_id: String,
    pub task_id: String,
    pub kind: String,
    pub summary: String,
    pub score: Option<f64>,
    pub matched_fields: Vec<String>,
    pub evidence_source: Option<String>,
    pub evidence_snippet: Option<String>,
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
    pub prefix_fts_query: Option<String>,
    pub terms: Vec<String>,
    pub like_text: String,
    pub intent: SearchIntent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchIntent {
    General,
    Phrase,
    Identifier,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchEvidence {
    pub source: String,
    pub snippet: String,
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

pub fn build_activity_search_text(kind: TaskActivityKind, content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return kind.to_string();
    }

    let normalized = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    let body = sample_activity_search_text(&normalized, ACTIVITY_TEXT_LIMIT);
    truncate(format!("{kind}: {body}"), ACTIVITY_TEXT_LIMIT + 32)
}

pub fn build_activity_search_chunks(content: &str) -> Vec<String> {
    let normalized = content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return Vec::new();
    }

    let chars = normalized.chars().collect::<Vec<_>>();
    if chars.len() <= ACTIVITY_CHUNK_TARGET_CHARS {
        return vec![normalized];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + ACTIVITY_CHUNK_TARGET_CHARS).min(chars.len());
        let chunk = chars[start..end]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(ACTIVITY_CHUNK_OVERLAP_CHARS);
    }

    chunks
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

    let terms = tokenize_search_terms(raw_text)
        .into_iter()
        .map(|term| term.to_lowercase())
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return None;
    }

    let fts_query = build_fts_query(&terms, false)?;
    let prefix_fts_query = build_fts_query(&terms, true).filter(|query| query != &fts_query);
    let like_text = terms.join(" ");
    let intent = detect_search_intent(&terms);

    Some(NormalizedSearchQuery {
        raw_text: raw_text.to_string(),
        fts_query,
        prefix_fts_query,
        terms,
        like_text,
        intent,
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
    let normalized = value.to_lowercase();
    terms.iter().any(|term| normalized.contains(term))
}

pub fn weighted_rrf_score(rank_index: usize, weight: f64) -> f64 {
    weight / (DEFAULT_RRF_K as f64 + rank_index as f64 + 1.0)
}

pub fn build_search_evidence<'a>(
    terms: &[String],
    fields: impl IntoIterator<Item = (&'a str, Option<&'a str>)>,
) -> Option<SearchEvidence> {
    fields.into_iter().find_map(|(field_name, value)| {
        value
            .filter(|candidate| contains_any_term(candidate, terms))
            .and_then(|candidate| {
                build_evidence_snippet(candidate, terms).map(|snippet| SearchEvidence {
                    source: field_name.to_string(),
                    snippet,
                })
            })
    })
}

fn tokenize_search_terms(raw_text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut buffer = String::new();
    let mut in_quotes = false;

    for ch in raw_text.chars() {
        match ch {
            '"' => {
                flush_search_term_buffer(&mut buffer, in_quotes, &mut terms);
                in_quotes = !in_quotes;
            }
            _ if ch.is_whitespace() && !in_quotes => {
                flush_search_term_buffer(&mut buffer, false, &mut terms);
            }
            _ => buffer.push(ch),
        }
    }
    flush_search_term_buffer(&mut buffer, in_quotes, &mut terms);

    terms
}

fn flush_search_term_buffer(buffer: &mut String, quoted: bool, terms: &mut Vec<String>) {
    if quoted {
        let phrase = buffer.split_whitespace().collect::<Vec<_>>().join(" ");
        if !phrase.is_empty() {
            terms.push(phrase);
        }
    } else {
        terms.extend(
            buffer
                .split_whitespace()
                .filter(|term| !term.is_empty())
                .map(ToOwned::to_owned),
        );
    }
    buffer.clear();
}

fn build_fts_query(terms: &[String], prefix_match: bool) -> Option<String> {
    let clauses = terms
        .iter()
        .map(|term| {
            let escaped = term.replace('"', "\"\"");
            if prefix_match && should_use_prefix_clause(term) {
                format!("\"{escaped}\"*")
            } else {
                format!("\"{escaped}\"")
            }
        })
        .collect::<Vec<_>>();
    (!clauses.is_empty()).then(|| clauses.join(" AND "))
}

fn detect_search_intent(terms: &[String]) -> SearchIntent {
    if terms
        .iter()
        .any(|term| term.chars().any(|ch| ch.is_whitespace()))
    {
        SearchIntent::Phrase
    } else if terms.len() == 1 && looks_like_identifier_query(&terms[0]) {
        SearchIntent::Identifier
    } else {
        SearchIntent::General
    }
}

fn looks_like_identifier_query(term: &str) -> bool {
    let has_ascii_alpha = term.chars().any(|ch| ch.is_ascii_alphabetic());
    let has_ascii_digit = term.chars().any(|ch| ch.is_ascii_digit());
    let has_separator = term.chars().any(|ch| matches!(ch, '-' | '_'));
    has_ascii_alpha && (has_ascii_digit || has_separator)
}

fn should_use_prefix_clause(term: &str) -> bool {
    term.chars().count() >= 2
        && !term.chars().any(|ch| ch.is_whitespace())
        && !contains_cjk_like_char(term)
        && term.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
}

fn contains_cjk_like_char(value: &str) -> bool {
    value.chars().any(|ch| {
        matches!(
            ch,
            '\u{3400}'..='\u{4DBF}'
                | '\u{4E00}'..='\u{9FFF}'
                | '\u{3040}'..='\u{30FF}'
                | '\u{AC00}'..='\u{D7AF}'
        )
    })
}

fn sample_activity_search_text(value: &str, limit: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= limit {
        return value.to_string();
    }

    let window = (limit / 3).max(256);
    let start = chars.iter().take(window).collect::<String>();
    let middle_start = chars.len().saturating_sub(window) / 2;
    let middle = chars
        .iter()
        .skip(middle_start)
        .take(window)
        .collect::<String>();
    let end = chars
        .iter()
        .rev()
        .take(window)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();

    format!("{start}\n...\n{middle}\n...\n{end}")
}

fn build_evidence_snippet(value: &str, terms: &[String]) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_lowercase();
    let match_start = terms.iter().filter_map(|term| lower.find(term)).min();
    let Some(match_start) = match_start else {
        return Some(truncate(trimmed.to_string(), 160));
    };

    let match_char = lower[..match_start].chars().count();
    let total_chars = trimmed.chars().count();
    let window = 72usize;
    let start = match_char.saturating_sub(window);
    let end = (match_char + window).min(total_chars);
    let snippet = slice_by_char_range(trimmed, start, end).trim().to_string();

    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if end < total_chars { "..." } else { "" };
    Some(format!("{prefix}{snippet}{suffix}"))
}

fn slice_by_char_range(value: &str, start: usize, end: usize) -> String {
    value
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
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
        build_activity_search_chunks, build_activity_search_summary, build_activity_search_text,
        build_task_vector_document_text, normalize_search_query, weighted_rrf_score, SearchIntent,
    };
    use crate::domain::TaskActivityKind;

    #[test]
    fn trims_large_activity_content() {
        let summary = build_activity_search_summary(TaskActivityKind::Note, &"x".repeat(400));
        assert!(summary.len() < 300);
    }

    #[test]
    fn activity_search_text_samples_long_content() {
        let content = format!(
            "{} {} {}",
            "start".repeat(600),
            "middle".repeat(600),
            "tail".repeat(600)
        );
        let search_text = build_activity_search_text(TaskActivityKind::Note, &content);
        assert!(search_text.contains("start"));
        assert!(search_text.contains("middle"));
        assert!(search_text.contains("tail"));
        assert!(search_text.len() <= 6_100);
    }

    #[test]
    fn activity_search_chunks_split_long_content() {
        let content = "alpha ".repeat(600);
        let chunks = build_activity_search_chunks(&content);
        assert!(chunks.len() >= 3);
        assert!(chunks.iter().all(|chunk| !chunk.trim().is_empty()));
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
            normalized.prefix_fts_query,
            Some("\"initctx-1\" AND \"reusable\"* AND \"conclusion\"*".to_string())
        );
        assert_eq!(
            normalized.terms,
            vec!["initctx-1", "reusable", "conclusion"]
        );
        assert_eq!(normalized.like_text, "initctx-1 reusable conclusion");
        assert_eq!(normalized.intent, SearchIntent::General);
    }

    #[test]
    fn keeps_non_ascii_terms_during_normalization() {
        let normalized = normalize_search_query("搜索 中文").expect("normalized query");
        assert_eq!(normalized.fts_query, "\"搜索\" AND \"中文\"");
        assert_eq!(normalized.prefix_fts_query, None);
        assert_eq!(normalized.intent, SearchIntent::General);
    }

    #[test]
    fn keeps_quoted_phrases_together() {
        let normalized =
            normalize_search_query("\"runtime console\" SearchV2").expect("normalized query");
        assert_eq!(normalized.terms, vec!["runtime console", "searchv2"]);
        assert_eq!(normalized.fts_query, "\"runtime console\" AND \"searchv2\"");
        assert_eq!(
            normalized.prefix_fts_query,
            Some("\"runtime console\" AND \"searchv2\"*".to_string())
        );
        assert_eq!(normalized.intent, SearchIntent::Phrase);
    }

    #[test]
    fn classifies_identifier_queries() {
        let normalized = normalize_search_query("SearchV2-04").expect("normalized query");
        assert_eq!(normalized.intent, SearchIntent::Identifier);
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
