use super::*;

pub(super) fn structured_task_hit_from_detail(detail: TaskDetail) -> TaskSearchHit {
    TaskSearchHit {
        task_id: detail.task.task_id.to_string(),
        project_id: detail.task.project_id.to_string(),
        version_id: detail.task.version_id.map(|value| value.to_string()),
        task_code: detail.task.task_code.clone(),
        task_kind: detail.task.task_kind.to_string(),
        title: detail.task.title.clone(),
        status: detail.task.status.to_string(),
        priority: detail.task.priority.to_string(),
        knowledge_status: detail.task.knowledge_status.to_string(),
        summary: task_summary(
            detail.task.latest_note_summary.as_deref(),
            detail.task.task_search_summary.as_str(),
        ),
        retrieval_source: "structured_filter".to_string(),
        score: None,
        matched_fields: Vec::new(),
        evidence_source: None,
        evidence_snippet: None,
        evidence_activity_id: None,
        evidence_chunk_id: None,
        evidence_attachment_id: None,
    }
}

pub(super) fn combine_task_search_results(
    lexical_rows: Vec<crate::storage::TaskLexicalSearchRow>,
    semantic_rows: Vec<crate::search::VectorQueryHit>,
    terms: &[String],
    activity_evidence_by_task: &HashMap<String, SearchEvidence>,
    semantic_evidence_by_task: &HashMap<String, SearchEvidence>,
    limit: usize,
) -> Vec<TaskSearchHit> {
    #[derive(Default)]
    struct CombinedTaskRow {
        lexical: Option<crate::storage::TaskLexicalSearchRow>,
        semantic_distance: Option<f64>,
        combined_score: f64,
    }

    let mut combined = HashMap::<String, CombinedTaskRow>::new();
    for (index, row) in lexical_rows.into_iter().enumerate() {
        let entry = combined.entry(row.task_id.clone()).or_default();
        entry.combined_score += weighted_rrf_score(index, LEXICAL_RRF_WEIGHT);
        entry.lexical = Some(row);
    }

    for (index, row) in semantic_rows.into_iter().enumerate() {
        let entry = combined.entry(row.task_id.clone()).or_default();
        entry.combined_score += weighted_rrf_score(index, SEMANTIC_RRF_WEIGHT);
        entry.semantic_distance = row.distance;
    }

    let mut rows = combined
        .into_values()
        .filter_map(|row| {
            row.lexical
                .map(|lexical| (lexical, row.semantic_distance, row.combined_score))
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .2
            .partial_cmp(&left.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.lexical_rank.cmp(&right.0.lexical_rank))
            .then_with(|| right.0.latest_activity_at.cmp(&left.0.latest_activity_at))
            .then_with(|| left.0.task_id.cmp(&right.0.task_id))
    });

    rows.into_iter()
        .take(limit)
        .map(|(row, semantic_distance, combined_score)| {
            let mut matched_fields = matched_field_names(
                terms,
                [
                    ("task_code", row.task_code.as_deref()),
                    ("title", Some(row.title.as_str())),
                    ("latest_note_summary", row.latest_note_summary.as_deref()),
                    (
                        "task_search_summary",
                        Some(row.task_search_summary.as_str()),
                    ),
                    (
                        "task_context_digest",
                        Some(row.task_context_digest.as_str()),
                    ),
                ],
            );
            let task_evidence = build_search_evidence(
                terms,
                [
                    ("task_code", row.task_code.as_deref()),
                    ("title", Some(row.title.as_str())),
                    ("latest_note_summary", row.latest_note_summary.as_deref()),
                    (
                        "task_search_summary",
                        Some(row.task_search_summary.as_str()),
                    ),
                    (
                        "task_context_digest",
                        Some(row.task_context_digest.as_str()),
                    ),
                ],
            );
            let fallback_evidence = activity_evidence_by_task.get(&row.task_id).cloned();
            let semantic_evidence = semantic_evidence_by_task.get(&row.task_id).cloned();
            let had_lexical_signal = !matched_fields.is_empty()
                || task_evidence.is_some()
                || fallback_evidence.is_some();
            let evidence = task_evidence.or(fallback_evidence).or(semantic_evidence);
            if let Some(evidence) = evidence.as_ref() {
                if !matched_fields.contains(&evidence.source) {
                    matched_fields.push(evidence.source.clone());
                }
            }
            let retrieval_source = match (semantic_distance.is_some(), had_lexical_signal) {
                (true, true) => "hybrid",
                (true, false) => "semantic",
                _ => "lexical",
            };
            TaskSearchHit {
                task_id: row.task_id,
                project_id: row.project_id,
                version_id: row.version_id,
                task_code: row.task_code,
                task_kind: row.task_kind,
                title: row.title,
                status: row.status,
                priority: row.priority,
                knowledge_status: row.knowledge_status,
                summary: task_summary(
                    row.latest_note_summary.as_deref(),
                    row.task_search_summary.as_str(),
                ),
                retrieval_source: retrieval_source.to_string(),
                score: Some(combined_score),
                matched_fields,
                evidence_source: evidence.as_ref().map(|item| item.source.clone()),
                evidence_snippet: evidence.as_ref().map(|item| item.snippet.clone()),
                evidence_activity_id: evidence.as_ref().and_then(|item| item.activity_id.clone()),
                evidence_chunk_id: evidence.as_ref().and_then(|item| item.chunk_id.clone()),
                evidence_attachment_id: evidence.and_then(|item| item.attachment_id),
            }
        })
        .collect()
}

pub(super) fn rerank_task_hits(hits: &mut [TaskSearchHit]) {
    hits.sort_by(|left, right| {
        rerank_score(right)
            .partial_cmp(&rerank_score(left))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.task_id.cmp(&right.task_id))
    });
}

fn rerank_score(hit: &TaskSearchHit) -> f64 {
    let base = hit.score.unwrap_or_default();
    let retrieval_bonus = match hit.retrieval_source.as_str() {
        "hybrid" => 0.003,
        "semantic" => 0.002,
        _ => 0.0,
    };
    let evidence_bonus = match hit.evidence_source.as_deref() {
        Some("semantic_activity_chunk") => 0.002,
        Some("activity_search_text") => 0.0015,
        Some("semantic_task_document") => 0.001,
        Some("latest_note_summary") => 0.0008,
        Some("task_search_summary") | Some("task_context_digest") => 0.0005,
        _ => 0.0,
    };
    base + retrieval_bonus + evidence_bonus
}

pub(super) fn build_activity_evidence_map(
    activity_rows: &[crate::storage::ActivityLexicalSearchRow],
    terms: &[String],
) -> HashMap<String, SearchEvidence> {
    let mut output = HashMap::new();

    for activity in activity_rows {
        let evidence = build_search_evidence(
            terms,
            [
                ("activity_search_text", Some(activity.search_text.as_str())),
                ("activity_search_summary", Some(activity.summary.as_str())),
            ],
        );
        if let Some(mut evidence) = evidence {
            evidence.activity_id = Some(activity.activity_id.clone());
            evidence.chunk_id = Some(activity.chunk_id.clone());
            evidence.attachment_id = activity.attachment_id.clone();
            output.entry(activity.task_id.clone()).or_insert(evidence);
        }
    }

    output
}

pub(super) fn build_semantic_evidence_map(
    semantic_rows: &[crate::search::VectorQueryHit],
) -> HashMap<String, SearchEvidence> {
    let mut output = HashMap::new();

    for hit in semantic_rows {
        let Some(document) = hit
            .document
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let source = if hit.source_kind == "activity_chunk" {
            "semantic_activity_chunk"
        } else {
            "semantic_task_document"
        };
        output.entry(hit.task_id.clone()).or_insert(SearchEvidence {
            source: source.to_string(),
            snippet: truncate_evidence(document, 180),
            activity_id: hit.activity_id.clone(),
            chunk_id: hit.chunk_id.clone(),
            attachment_id: hit.attachment_id.clone(),
        });
    }

    output
}

pub(super) fn merge_lexical_task_rows(
    groups: Vec<Vec<crate::storage::TaskLexicalSearchRow>>,
) -> Vec<crate::storage::TaskLexicalSearchRow> {
    let mut seen = HashSet::<String>::new();
    let mut merged = Vec::new();

    for group in groups {
        for row in group {
            if seen.insert(row.task_id.clone()) {
                merged.push(row);
            }
        }
    }

    for (index, row) in merged.iter_mut().enumerate() {
        row.lexical_rank = index;
    }

    merged
}

pub(super) fn task_summary(latest_note_summary: Option<&str>, task_search_summary: &str) -> String {
    latest_note_summary
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(task_search_summary)
        .to_string()
}

pub(super) fn default_indexed_fields() -> SearchIndexedFields {
    SearchIndexedFields {
        tasks: vec![
            "title".to_string(),
            "task_code".to_string(),
            "task_kind".to_string(),
            "task_search_summary".to_string(),
            "task_context_digest".to_string(),
            "latest_note_summary".to_string(),
        ],
        activities: vec![
            "activity_search_summary".to_string(),
            "activity_search_text".to_string(),
            "activity_chunk".to_string(),
        ],
    }
}

fn truncate_evidence(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut output = value
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>();
    output.push_str("...");
    output
}

pub(super) fn vector_status_label(
    vector_enabled: bool,
    used_hybrid: bool,
    pending_index_jobs: usize,
) -> String {
    if !vector_enabled {
        "disabled".to_string()
    } else if pending_index_jobs > 0 {
        "indexing".to_string()
    } else if used_hybrid {
        "ready".to_string()
    } else {
        "lexical_fallback".to_string()
    }
}

pub(super) fn matches_prefix_filters(
    row: &crate::storage::TaskLexicalSearchRow,
    filter: &TaskListFilter,
) -> bool {
    if let Some(task_code_prefix) = filter.task_code_prefix.as_deref() {
        if !row
            .task_code
            .as_deref()
            .is_some_and(|value| value.starts_with(task_code_prefix))
        {
            return false;
        }
    }
    if let Some(title_prefix) = filter.title_prefix.as_deref() {
        if !row.title.starts_with(title_prefix) {
            return false;
        }
    }
    true
}

pub(super) fn search_index_job_summary(record: SearchIndexJobRecord) -> SearchIndexJobSummary {
    SearchIndexJobSummary {
        task_id: record.task_id,
        title: record.title,
        status: record.status,
        attempt_count: record.attempt_count,
        last_error: record.last_error,
        next_attempt_at: record.next_attempt_at,
        locked_at: record.locked_at,
        lease_until: record.lease_until,
        updated_at: record.updated_at,
        run_id: record.run_id,
    }
}

pub(super) fn search_sidecar_status_label(status: SearchSidecarStatus) -> &'static str {
    match status {
        SearchSidecarStatus::Disabled => "disabled",
        SearchSidecarStatus::Running => "running",
        SearchSidecarStatus::External => "external",
    }
}

pub(super) fn search_index_operation_kind(trigger_kind: &str) -> &'static str {
    match trigger_kind {
        "manual_backfill" => "manual_rebuild",
        "retry_failed" => "retry_failed",
        "recover_stale" => "recover_stale",
        _ => "incremental_upsert",
    }
}

pub(super) fn search_index_operation_description(trigger_kind: &str) -> &'static str {
    match search_index_operation_kind(trigger_kind) {
        "manual_rebuild" => "Scans local tasks and re-upserts their Chroma vectors.",
        "retry_failed" => "Requeues failed vector-index jobs and processes them again.",
        "recover_stale" => "Reclaims expired processing jobs and resumes indexing.",
        _ => "Indexes tasks changed by local or remote mutations.",
    }
}
