use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use agenta_lib::{
    app::{AppRuntime, BootstrapOptions},
    domain::{KnowledgeStatus, NoteKind, TaskKind, TaskPriority, TaskStatus, VersionStatus},
    service::{
        CreateAttachmentInput, CreateNoteInput, CreateProjectInput, CreateTaskInput,
        CreateVersionInput, SearchInput,
    },
};

const EVAL_FIXTURE: &str = include_str!("fixtures/search_eval_v011.json");

#[derive(Debug, Deserialize)]
struct SearchEvalFixture {
    project: EvalProject,
    version: EvalVersion,
    tasks: Vec<EvalTask>,
    queries: Vec<EvalQuery>,
}

#[derive(Debug, Deserialize)]
struct EvalProject {
    slug: String,
    name: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EvalVersion {
    name: String,
    description: Option<String>,
    status: String,
}

#[derive(Debug, Deserialize)]
struct EvalTask {
    code: String,
    kind: String,
    title: String,
    summary: Option<String>,
    description: Option<String>,
    status: String,
    priority: String,
    #[serde(default)]
    notes: Vec<EvalNote>,
    #[serde(default)]
    attachments: Vec<EvalAttachment>,
}

#[derive(Debug, Deserialize)]
struct EvalNote {
    kind: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct EvalAttachment {
    filename: String,
    summary: Option<String>,
    content: String,
}

#[derive(Clone, Debug, Deserialize)]
struct EvalQuery {
    id: String,
    text: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    knowledge_status: Option<String>,
    #[serde(default)]
    task_kind: Option<String>,
    #[serde(default)]
    task_code_prefix: Option<String>,
    #[serde(default)]
    title_prefix: Option<String>,
    expected_task_codes: Vec<String>,
    #[serde(default)]
    expected_evidence_sources: Vec<String>,
    #[serde(default)]
    require_evidence_id: bool,
}

#[derive(Clone, Debug, Serialize)]
struct SearchEvalReport {
    fixture: String,
    profiles: Vec<SearchEvalProfileReport>,
    comparison: SearchEvalComparison,
}

#[derive(Clone, Debug, Serialize)]
struct SearchEvalProfileReport {
    profile: String,
    backend: String,
    metrics: SearchEvalMetrics,
    queries: Vec<SearchEvalQueryReport>,
}

#[derive(Clone, Debug, Serialize)]
struct SearchEvalMetrics {
    query_count: usize,
    accuracy_at_1: f64,
    recall_at_5: f64,
    recall_at_10: f64,
    mrr: f64,
    relevance_correctness: f64,
    evidence_coverage: f64,
    semantic_attempt_rate: f64,
    semantic_use_rate: f64,
    semantic_error_count: usize,
    total_latency_ms: f64,
    average_latency_ms: f64,
    p95_latency_ms: f64,
    max_latency_ms: f64,
    performance_score: f64,
}

#[derive(Clone, Debug, Serialize)]
struct SearchEvalQueryReport {
    id: String,
    text: Option<String>,
    expected_task_codes: Vec<String>,
    returned_task_codes: Vec<Option<String>>,
    first_relevant_rank: Option<usize>,
    top1_correct: bool,
    recall_at_5: f64,
    recall_at_10: f64,
    reciprocal_rank: f64,
    relevance_correct: bool,
    evidence_correct: bool,
    latency_ms: f64,
    retrieval_mode: String,
    semantic_attempted: bool,
    semantic_used: bool,
    semantic_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct SearchEvalComparison {
    hybrid_accuracy_delta: f64,
    hybrid_recall_at_10_delta: f64,
    hybrid_mrr_delta: f64,
    hybrid_evidence_coverage_delta: f64,
    hybrid_average_latency_delta_ms: f64,
}

#[derive(Clone, Debug)]
struct MockVectorRecord {
    id: String,
    embedding: Vec<f32>,
    metadata: Value,
    document: String,
}

#[derive(Clone, Default)]
struct MockSearchState {
    records: Arc<Mutex<Vec<MockVectorRecord>>>,
}

#[tokio::test]
async fn search_eval_v011_reports_relevance_and_performance_metrics(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture: SearchEvalFixture = serde_json::from_str(EVAL_FIXTURE)?;
    let lexical = run_eval_profile(&fixture, EvalProfile::Lexical).await?;
    let hybrid = run_eval_profile(&fixture, EvalProfile::HybridMock).await?;
    let report = SearchEvalReport {
        fixture: "search_eval_v011".to_string(),
        comparison: SearchEvalComparison {
            hybrid_accuracy_delta: hybrid.metrics.accuracy_at_1 - lexical.metrics.accuracy_at_1,
            hybrid_recall_at_10_delta: hybrid.metrics.recall_at_10 - lexical.metrics.recall_at_10,
            hybrid_mrr_delta: hybrid.metrics.mrr - lexical.metrics.mrr,
            hybrid_evidence_coverage_delta: hybrid.metrics.evidence_coverage
                - lexical.metrics.evidence_coverage,
            hybrid_average_latency_delta_ms: hybrid.metrics.average_latency_ms
                - lexical.metrics.average_latency_ms,
        },
        profiles: vec![lexical, hybrid],
    };

    println!(
        "SEARCH_EVAL_REPORT={}",
        serde_json::to_string_pretty(&report)?
    );

    let lexical_metrics = &report.profiles[0].metrics;
    let hybrid_metrics = &report.profiles[1].metrics;
    assert!(lexical_metrics.accuracy_at_1 >= 0.75);
    assert!(lexical_metrics.evidence_coverage >= 0.70);
    assert!(hybrid_metrics.accuracy_at_1 >= lexical_metrics.accuracy_at_1);
    assert!(hybrid_metrics.recall_at_10 >= lexical_metrics.recall_at_10);
    assert!(hybrid_metrics.mrr >= lexical_metrics.mrr);
    assert!(hybrid_metrics.evidence_coverage >= lexical_metrics.evidence_coverage);
    assert!(hybrid_metrics.semantic_attempt_rate > 0.0);
    assert!(hybrid_metrics.semantic_use_rate > 0.0);
    assert_eq!(hybrid_metrics.semantic_error_count, 0);
    assert!(hybrid_metrics.average_latency_ms < 250.0);

    let semantic_query = report.profiles[1]
        .queries
        .iter()
        .find(|query| query.id == "semantic_paraphrase")
        .expect("semantic query report");
    assert!(semantic_query.top1_correct);
    assert!(semantic_query.evidence_correct);

    Ok(())
}

#[derive(Clone, Copy)]
enum EvalProfile {
    Lexical,
    HybridMock,
}

impl EvalProfile {
    fn name(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::HybridMock => "hybrid_mock",
        }
    }
}

async fn run_eval_profile(
    fixture: &SearchEvalFixture,
    profile: EvalProfile,
) -> Result<SearchEvalProfileReport, Box<dyn std::error::Error>> {
    let tempdir = TempDir::new()?;
    let disabled_config = write_eval_config(&tempdir, None)?;
    let runtime = AppRuntime::bootstrap(BootstrapOptions {
        config_path: Some(disabled_config),
    })
    .await?;
    let seeded = seed_fixture(&runtime, &tempdir, fixture).await?;
    drop(runtime);

    let (_server, _state, enabled_runtime, backend) = match profile {
        EvalProfile::Lexical => {
            let runtime = AppRuntime::bootstrap(BootstrapOptions {
                config_path: Some(write_eval_config(&tempdir, None)?),
            })
            .await?;
            (None, None, runtime, "sqlite_fts".to_string())
        }
        EvalProfile::HybridMock => {
            let (endpoint, state, server) = spawn_mock_search_server().await?;
            let runtime = AppRuntime::bootstrap(BootstrapOptions {
                config_path: Some(write_eval_config(&tempdir, Some(&endpoint))?),
            })
            .await?;
            let backfill = runtime.service.search_backfill(Some(100), Some(20)).await?;
            assert_eq!(backfill.failed, 0);
            (
                Some(server),
                Some(state),
                runtime,
                "mock_chroma_deterministic".to_string(),
            )
        }
    };

    let mut query_reports = Vec::new();
    for query in &fixture.queries {
        query_reports.push(run_query_eval(&enabled_runtime, &seeded, query).await?);
    }

    Ok(SearchEvalProfileReport {
        profile: profile.name().to_string(),
        backend,
        metrics: aggregate_metrics(&query_reports),
        queries: query_reports,
    })
}

struct SeededFixture {
    project_slug: String,
    version_id: String,
}

async fn seed_fixture(
    runtime: &AppRuntime,
    tempdir: &TempDir,
    fixture: &SearchEvalFixture,
) -> Result<SeededFixture, Box<dyn std::error::Error>> {
    let project = runtime
        .service
        .create_project(CreateProjectInput {
            slug: fixture.project.slug.clone(),
            name: fixture.project.name.clone(),
            description: fixture.project.description.clone(),
        })
        .await?;
    let version = runtime
        .service
        .create_version(CreateVersionInput {
            project: project.slug.clone(),
            name: fixture.version.name.clone(),
            description: fixture.version.description.clone(),
            status: Some(VersionStatus::from_str(&fixture.version.status)?),
        })
        .await?;

    for task in &fixture.tasks {
        let created = runtime
            .service
            .create_task(CreateTaskInput {
                project: project.slug.clone(),
                version: Some(version.version_id.to_string()),
                task_code: Some(task.code.clone()),
                task_kind: Some(TaskKind::from_str(&task.kind)?),
                title: task.title.clone(),
                summary: task.summary.clone(),
                description: task.description.clone(),
                status: Some(TaskStatus::from_str(&task.status)?),
                priority: Some(TaskPriority::from_str(&task.priority)?),
                created_by: Some("search-eval".to_string()),
            })
            .await?;
        for note in &task.notes {
            runtime
                .service
                .create_note(CreateNoteInput {
                    task: created.task_id.to_string(),
                    content: note.content.clone(),
                    note_kind: Some(NoteKind::from_str(&note.kind)?),
                    created_by: Some("search-eval".to_string()),
                })
                .await?;
        }
        for attachment in &task.attachments {
            let path = tempdir
                .path()
                .join("attachments-src")
                .join(&attachment.filename);
            std::fs::create_dir_all(path.parent().expect("attachment parent"))?;
            std::fs::write(&path, &attachment.content)?;
            runtime
                .service
                .create_attachment(CreateAttachmentInput {
                    task: created.task_id.to_string(),
                    path,
                    kind: None,
                    created_by: Some("search-eval".to_string()),
                    summary: attachment.summary.clone(),
                })
                .await?;
        }
    }

    Ok(SeededFixture {
        project_slug: project.slug,
        version_id: version.version_id.to_string(),
    })
}

async fn run_query_eval(
    runtime: &AppRuntime,
    seeded: &SeededFixture,
    query: &EvalQuery,
) -> Result<SearchEvalQueryReport, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let response = runtime
        .service
        .search(SearchInput {
            text: query.text.clone(),
            project: Some(seeded.project_slug.clone()),
            version: Some(seeded.version_id.clone()),
            status: parse_optional::<TaskStatus>(query.status.as_deref())?,
            priority: parse_optional::<TaskPriority>(query.priority.as_deref())?,
            knowledge_status: parse_optional::<KnowledgeStatus>(query.knowledge_status.as_deref())?,
            task_kind: parse_optional::<TaskKind>(query.task_kind.as_deref())?,
            task_code_prefix: query.task_code_prefix.clone(),
            title_prefix: query.title_prefix.clone(),
            limit: Some(10),
            all_projects: false,
        })
        .await?;
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

    let returned_task_codes = response
        .tasks
        .iter()
        .map(|hit| hit.task_code.clone())
        .collect::<Vec<_>>();
    let rank_by_code = response
        .tasks
        .iter()
        .enumerate()
        .filter_map(|(index, hit)| hit.task_code.as_ref().map(|code| (code.clone(), index + 1)))
        .collect::<HashMap<_, _>>();
    let first_relevant_rank = query
        .expected_task_codes
        .iter()
        .filter_map(|code| rank_by_code.get(code).copied())
        .min();
    let recall_at_5 = recall_at(&query.expected_task_codes, &rank_by_code, 5);
    let recall_at_10 = recall_at(&query.expected_task_codes, &rank_by_code, 10);
    let reciprocal_rank = first_relevant_rank
        .map(|rank| 1.0 / rank as f64)
        .unwrap_or_default();
    let evidence_correct = evidence_matches(query, &response.tasks);

    Ok(SearchEvalQueryReport {
        id: query.id.clone(),
        text: query.text.clone(),
        expected_task_codes: query.expected_task_codes.clone(),
        returned_task_codes,
        first_relevant_rank,
        top1_correct: first_relevant_rank == Some(1),
        recall_at_5,
        recall_at_10,
        reciprocal_rank,
        relevance_correct: recall_at_10 >= 1.0,
        evidence_correct,
        latency_ms,
        retrieval_mode: response.meta.retrieval_mode,
        semantic_attempted: response.meta.semantic_attempted,
        semantic_used: response.meta.semantic_used,
        semantic_error: response.meta.semantic_error,
    })
}

fn parse_optional<T>(value: Option<&str>) -> Result<Option<T>, T::Err>
where
    T: FromStr,
{
    value.map(T::from_str).transpose()
}

fn recall_at(expected_codes: &[String], rank_by_code: &HashMap<String, usize>, k: usize) -> f64 {
    if expected_codes.is_empty() {
        return 1.0;
    }
    let matched = expected_codes
        .iter()
        .filter(|code| rank_by_code.get(*code).is_some_and(|rank| *rank <= k))
        .count();
    matched as f64 / expected_codes.len() as f64
}

fn evidence_matches(query: &EvalQuery, hits: &[agenta_lib::search::TaskSearchHit]) -> bool {
    if query.expected_evidence_sources.is_empty() && !query.require_evidence_id {
        return true;
    }
    hits.iter().any(|hit| {
        let relevant = hit
            .task_code
            .as_ref()
            .is_some_and(|code| query.expected_task_codes.contains(code));
        let source_matches = query.expected_evidence_sources.is_empty()
            || hit.evidence_source.as_ref().is_some_and(|source| {
                query
                    .expected_evidence_sources
                    .iter()
                    .any(|expected| expected == source)
            });
        let evidence_id_matches = !query.require_evidence_id
            || hit.evidence_chunk_id.is_some()
            || hit.evidence_attachment_id.is_some()
            || hit.evidence_activity_id.is_some();
        relevant && source_matches && evidence_id_matches
    })
}

fn aggregate_metrics(query_reports: &[SearchEvalQueryReport]) -> SearchEvalMetrics {
    let query_count = query_reports.len();
    let query_count_f64 = query_count as f64;
    let top1_count = query_reports
        .iter()
        .filter(|report| report.top1_correct)
        .count();
    let relevance_count = query_reports
        .iter()
        .filter(|report| report.relevance_correct)
        .count();
    let evidence_count = query_reports
        .iter()
        .filter(|report| report.evidence_correct)
        .count();
    let semantic_attempt_count = query_reports
        .iter()
        .filter(|report| report.semantic_attempted)
        .count();
    let semantic_use_count = query_reports
        .iter()
        .filter(|report| report.semantic_used)
        .count();
    let semantic_error_count = query_reports
        .iter()
        .filter(|report| report.semantic_error.is_some())
        .count();
    let total_latency_ms = query_reports
        .iter()
        .map(|report| report.latency_ms)
        .sum::<f64>();
    let mut latencies = query_reports
        .iter()
        .map(|report| report.latency_ms)
        .collect::<Vec<_>>();
    latencies.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let p95_index = query_count
        .saturating_sub(1)
        .min(((query_count as f64) * 0.95).ceil() as usize);
    let average_latency_ms = if query_count == 0 {
        0.0
    } else {
        total_latency_ms / query_count_f64
    };

    SearchEvalMetrics {
        query_count,
        accuracy_at_1: ratio(top1_count, query_count),
        recall_at_5: average(query_reports.iter().map(|report| report.recall_at_5)),
        recall_at_10: average(query_reports.iter().map(|report| report.recall_at_10)),
        mrr: average(query_reports.iter().map(|report| report.reciprocal_rank)),
        relevance_correctness: ratio(relevance_count, query_count),
        evidence_coverage: ratio(evidence_count, query_count),
        semantic_attempt_rate: ratio(semantic_attempt_count, query_count),
        semantic_use_rate: ratio(semantic_use_count, query_count),
        semantic_error_count,
        total_latency_ms,
        average_latency_ms,
        p95_latency_ms: latencies.get(p95_index).copied().unwrap_or_default(),
        max_latency_ms: latencies.last().copied().unwrap_or_default(),
        performance_score: 1.0 / (1.0 + average_latency_ms / 100.0),
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn average(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn write_eval_config(
    tempdir: &TempDir,
    vector_endpoint: Option<&str>,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let config_path = tempdir.path().join(match vector_endpoint {
        Some(_) => "agenta.vector.local.yaml",
        None => "agenta.lexical.local.yaml",
    });
    let data_dir = tempdir.path().join("data");
    let yaml = match vector_endpoint {
        Some(endpoint) => format!(
            "paths:\n  data_dir: {}\nsearch:\n  vector:\n    enabled: true\n    endpoint: {}\n    autostart_sidecar: false\n    sidecar_data_dir: {}\n  embedding:\n    provider: openai_compatible\n    base_url: {}\n    api_key: inline-search-key\n    model: deterministic-eval\n",
            normalize_path_for_yaml(&data_dir),
            endpoint,
            normalize_path_for_yaml(&tempdir.path().join("search").join("chroma")),
            endpoint,
        ),
        None => format!(
            "paths:\n  data_dir: {}\nsearch:\n  vector:\n    enabled: false\n    autostart_sidecar: false\n",
            normalize_path_for_yaml(&data_dir),
        ),
    };
    std::fs::write(&config_path, yaml)?;
    Ok(config_path)
}

fn normalize_path_for_yaml(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

async fn mock_heartbeat() -> StatusCode {
    StatusCode::OK
}

async fn mock_collections() -> Json<Value> {
    Json(json!({ "id": "eval-collection" }))
}

async fn mock_embeddings(Json(payload): Json<Value>) -> Json<Value> {
    let inputs = payload["input"].as_array().cloned().unwrap_or_default();
    Json(json!({
        "data": inputs
            .iter()
            .enumerate()
            .map(|(index, value)| {
                json!({
                    "index": index,
                    "embedding": deterministic_embedding(value.as_str().unwrap_or_default()),
                })
            })
            .collect::<Vec<_>>()
    }))
}

async fn mock_upsert(
    State(state): State<MockSearchState>,
    Path(_collection_id): Path<String>,
    Json(payload): Json<Value>,
) -> StatusCode {
    let ids = payload["ids"].as_array().cloned().unwrap_or_default();
    let embeddings = payload["embeddings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let metadatas = payload["metadatas"].as_array().cloned().unwrap_or_default();
    let documents = payload["documents"].as_array().cloned().unwrap_or_default();
    let mut records = state.records.lock().await;
    for index in 0..ids.len() {
        let Some(id) = ids.get(index).and_then(Value::as_str) else {
            continue;
        };
        let embedding = embeddings
            .get(index)
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_f64)
                    .map(|value| value as f32)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        records.retain(|record| record.id != id);
        records.push(MockVectorRecord {
            id: id.to_string(),
            embedding,
            metadata: metadatas.get(index).cloned().unwrap_or_else(|| json!({})),
            document: documents
                .get(index)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    StatusCode::OK
}

async fn mock_query(
    State(state): State<MockSearchState>,
    Path(_collection_id): Path<String>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let query_embedding = payload["query_embeddings"]
        .as_array()
        .and_then(|queries| queries.first())
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_f64)
                .map(|value| value as f32)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let n_results = payload["n_results"].as_u64().unwrap_or(10) as usize;
    let mut ranked = state
        .records
        .lock()
        .await
        .clone()
        .into_iter()
        .filter(|record| similarity(&record.embedding, &query_embedding) > 0.0)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        similarity(&right.embedding, &query_embedding)
            .partial_cmp(&similarity(&left.embedding, &query_embedding))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| source_priority(&left.metadata).cmp(&source_priority(&right.metadata)))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.truncate(n_results);
    let ids = ranked
        .iter()
        .map(|record| record.id.clone())
        .collect::<Vec<_>>();
    let distances = ranked
        .iter()
        .map(|record| 1.0 - similarity(&record.embedding, &query_embedding))
        .collect::<Vec<_>>();
    let metadatas = ranked
        .iter()
        .map(|record| record.metadata.clone())
        .collect::<Vec<_>>();
    let documents = ranked
        .iter()
        .map(|record| record.document.clone())
        .collect::<Vec<_>>();
    Json(json!({
        "ids": [ids],
        "distances": [distances],
        "metadatas": [metadatas],
        "documents": [documents],
    }))
}

async fn spawn_mock_search_server(
) -> Result<(String, MockSearchState, tokio::task::JoinHandle<()>), Box<dyn std::error::Error>> {
    let state = MockSearchState::default();
    let app = Router::new()
        .route("/api/v2/heartbeat", get(mock_heartbeat))
        .route(
            "/api/v2/tenants/default_tenant/databases/default_database/collections",
            post(mock_collections),
        )
        .route(
            "/api/v2/tenants/default_tenant/databases/default_database/collections/{collection_id}/upsert",
            post(mock_upsert),
        )
        .route(
            "/api/v2/tenants/default_tenant/databases/default_database/collections/{collection_id}/query",
            post(mock_query),
        )
        .route("/v1/embeddings", post(mock_embeddings))
        .with_state(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = format!("http://{}", listener.local_addr()?);
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("search eval mock server");
    });
    Ok((address, state, server))
}

fn deterministic_embedding(text: &str) -> Vec<f32> {
    let lower = text.to_lowercase();
    let mut vector = vec![0.0; 10];
    add_feature(
        &lower,
        &mut vector,
        0,
        &["watchdog", "guardian", "supervision", "sidecar"],
    );
    add_feature(
        &lower,
        &mut vector,
        1,
        &["attachment", "architecture", "blueprint", "evidence"],
    );
    add_feature(
        &lower,
        &mut vector,
        2,
        &["archival", "historic", "history", "historical"],
    );
    add_feature(
        &lower,
        &mut vector,
        3,
        &["runtime", "console", "failure", "recovery"],
    );
    add_feature(&lower, &mut vector, 4, &["filter", "priority", "status"]);
    add_feature(&lower, &mut vector, 5, &["reusable", "conclusion"]);
    add_feature(&lower, &mut vector, 6, &["桌面搜索", "控制台"]);
    add_feature(&lower, &mut vector, 7, &["searchv2-04", "initctx-01"]);
    vector
}

fn add_feature(text: &str, vector: &mut [f32], index: usize, terms: &[&str]) {
    let matched = terms.iter().filter(|term| text.contains(*term)).count();
    vector[index] = matched as f32 / terms.len() as f32;
}

fn similarity(left: &[f32], right: &[f32]) -> f64 {
    let len = left.len().min(right.len());
    if len == 0 {
        return 0.0;
    }
    let dot = (0..len)
        .map(|index| left[index] as f64 * right[index] as f64)
        .sum::<f64>();
    let left_norm = left
        .iter()
        .map(|value| (*value as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    let right_norm = right
        .iter()
        .map(|value| (*value as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn source_priority(metadata: &Value) -> usize {
    if metadata["source_kind"] == "activity_chunk" {
        0
    } else {
        1
    }
}
