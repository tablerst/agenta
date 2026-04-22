use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use time::OffsetDateTime;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::app::{SearchConfig, SearchEmbeddingProvider, SearchVectorBackend};
use crate::error::{AppError, AppResult};
use crate::search::TaskVectorDocument;
use crate::storage::{SqliteStore, TaskListFilter};

const INDEX_JOB_BATCH_SIZE: usize = 10;
const MAX_INDEX_JOB_BATCH_SIZE: usize = 200;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug)]
pub struct SearchVectorJob {
    pub task_id: Uuid,
    pub attempt_count: i64,
}

#[derive(Clone, Debug)]
pub struct VectorQueryHit {
    pub task_id: String,
    pub distance: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchSidecarStatus {
    Disabled,
    Running,
    External,
}

#[derive(Clone, Debug)]
pub struct SearchRuntimeStatus {
    pub sidecar: SearchSidecarStatus,
    pub vector_available: bool,
}

#[derive(Clone)]
pub struct SearchRuntime {
    inner: Arc<SearchRuntimeInner>,
}

struct SearchRuntimeInner {
    config: SearchConfig,
    http: reqwest::Client,
    worker_lock: Mutex<()>,
    collection_id: Mutex<Option<String>>,
    sidecar: Mutex<Option<Child>>,
}

#[derive(Deserialize)]
struct ChromaCollectionRecord {
    id: String,
}

#[derive(Deserialize)]
struct ChromaQueryResponse {
    ids: Vec<Vec<String>>,
    distances: Option<Vec<Vec<f64>>>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingItem>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingItem {
    embedding: Vec<f32>,
}

fn configure_sidecar_command(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
}

impl SearchRuntime {
    pub fn new(config: SearchConfig) -> AppResult<Self> {
        let http = reqwest::Client::builder().build().map_err(|error| {
            AppError::Io(format!("failed to build search http client: {error}"))
        })?;
        Ok(Self {
            inner: Arc::new(SearchRuntimeInner {
                config,
                http,
                worker_lock: Mutex::new(()),
                collection_id: Mutex::new(None),
                sidecar: Mutex::new(None),
            }),
        })
    }

    pub fn config(&self) -> &SearchConfig {
        &self.inner.config
    }

    pub fn vector_enabled(&self) -> bool {
        self.inner.config.vector.enabled
    }

    pub fn vector_backend_name(&self) -> Option<String> {
        self.vector_enabled()
            .then(|| self.inner.config.vector.backend.as_str().to_string())
    }

    pub fn autostart_sidecar_enabled(&self) -> bool {
        self.vector_enabled() && self.inner.config.vector.autostart_sidecar
    }

    pub fn trigger_index_worker(&self, store: SqliteStore) {
        if !self.vector_enabled() {
            return;
        }

        let runtime = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let _ = runtime.process_pending_jobs(store).await;
        });
    }

    pub async fn process_pending_jobs(&self, store: SqliteStore) -> AppResult<()> {
        self.process_pending_jobs_with_batch(store, INDEX_JOB_BATCH_SIZE)
            .await
    }

    pub async fn process_pending_jobs_with_batch(
        &self,
        store: SqliteStore,
        batch_size: usize,
    ) -> AppResult<()> {
        if !self.vector_enabled() {
            return Ok(());
        }

        let batch_size = batch_size.clamp(1, MAX_INDEX_JOB_BATCH_SIZE);
        let _guard = self.inner.worker_lock.lock().await;
        let mut first_error: Option<String> = None;
        loop {
            let mut jobs = Vec::with_capacity(batch_size);
            for _ in 0..batch_size {
                let now = OffsetDateTime::now_utc();
                let Some(job) = store.claim_next_search_index_job(now).await? else {
                    break;
                };
                jobs.push(job);
            }
            if jobs.is_empty() {
                break;
            }

            let mut documents = Vec::with_capacity(jobs.len());
            let mut queued_task_ids = Vec::with_capacity(jobs.len());
            for job in &jobs {
                match store.get_task_vector_document(job.task_id).await? {
                    Some(document) => {
                        queued_task_ids.push(job.task_id);
                        documents.push(document);
                    }
                    None => {
                        store.complete_search_index_job(job.task_id).await?;
                    }
                }
            }

            if documents.is_empty() {
                continue;
            }

            match self.process_job_batch(&documents).await {
                Ok(()) => {
                    for task_id in queued_task_ids {
                        store.complete_search_index_job(task_id).await?;
                    }
                }
                Err(error) => {
                    let error_message = error.to_string();
                    if first_error.is_none() {
                        first_error = Some(error_message.clone());
                    }
                    let now = OffsetDateTime::now_utc();
                    for job in jobs
                        .iter()
                        .filter(|candidate| queued_task_ids.contains(&candidate.task_id))
                    {
                        let next_attempt_at =
                            now + time::Duration::seconds(backoff_seconds(job.attempt_count));
                        store
                            .fail_search_index_job(job.task_id, &error_message, next_attempt_at)
                            .await?;
                    }
                }
            }
        }

        if let Some(error_message) = first_error {
            Err(AppError::Io(error_message))
        } else {
            Ok(())
        }
    }

    pub async fn runtime_status(&self) -> SearchRuntimeStatus {
        let sidecar = if !self.autostart_sidecar_enabled() {
            SearchSidecarStatus::Disabled
        } else if self.heartbeat().await {
            let child = self.inner.sidecar.lock().await;
            if child.is_some() {
                SearchSidecarStatus::Running
            } else {
                SearchSidecarStatus::External
            }
        } else {
            SearchSidecarStatus::Disabled
        };
        SearchRuntimeStatus {
            sidecar,
            vector_available: self.vector_enabled() && self.heartbeat().await,
        }
    }

    pub async fn start_sidecar(&self) -> AppResult<SearchSidecarStatus> {
        if !self.autostart_sidecar_enabled() {
            return Ok(SearchSidecarStatus::Disabled);
        }

        if self.heartbeat().await {
            let child = self.inner.sidecar.lock().await;
            return Ok(if child.is_some() {
                SearchSidecarStatus::Running
            } else {
                SearchSidecarStatus::External
            });
        }

        let mut guard = self.inner.sidecar.lock().await;
        if guard.is_some() {
            return Ok(SearchSidecarStatus::Running);
        }

        tokio::fs::create_dir_all(&self.inner.config.vector.sidecar_data_dir).await?;

        let host = self
            .inner
            .config
            .vector
            .endpoint
            .host_str()
            .unwrap_or("127.0.0.1")
            .to_string();
        let port = self
            .inner
            .config
            .vector
            .endpoint
            .port_or_known_default()
            .unwrap_or(8000)
            .to_string();

        let mut command = Command::new("chroma");
        configure_sidecar_command(&mut command);

        let mut child = command
            .arg("run")
            .arg("--host")
            .arg(host)
            .arg("--port")
            .arg(port)
            .arg("--path")
            .arg(&self.inner.config.vector.sidecar_data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    AppError::Io(
                        "failed to start chroma sidecar: program not found; install the Chroma CLI or run a local Chroma server before backfill".to_string(),
                    )
                } else {
                    AppError::Io(format!("failed to start chroma sidecar: {error}"))
                }
            })?;

        if !self.wait_for_heartbeat().await {
            let _ = child.kill().await;
            return Err(AppError::Io(
                "timed out while waiting for chroma sidecar".to_string(),
            ));
        }

        *guard = Some(child);
        Ok(SearchSidecarStatus::Running)
    }

    pub async fn stop_sidecar(&self) -> AppResult<()> {
        let mut guard = self.inner.sidecar.lock().await;
        if let Some(child) = guard.as_mut() {
            let _ = child.kill().await;
        }
        *guard = None;
        Ok(())
    }

    pub async fn query_tasks(
        &self,
        query_text: &str,
        filter: &TaskListFilter,
        limit: usize,
    ) -> AppResult<Vec<VectorQueryHit>> {
        if !self.vector_enabled() {
            return Ok(Vec::new());
        }

        self.ensure_vector_ready().await?;
        let collection_id = self.ensure_collection_id().await?;
        let embedding = self.embed_text(query_text).await?;
        let mut payload = json!({
            "query_embeddings": [embedding],
            "n_results": limit.min(self.inner.config.vector.top_k),
            "include": ["distances"],
        });
        if let Some(where_clause) = chroma_where_clause(filter) {
            payload["where"] = Value::Object(where_clause);
        }

        let response = self
            .inner
            .http
            .post(self.collection_endpoint(&collection_id, "query")?)
            .json(&payload)
            .send()
            .await
            .map_err(|error| AppError::Io(format!("chroma query request failed: {error}")))?;
        if !response.status().is_success() {
            return Err(AppError::Io(format!(
                "chroma query failed with status {}",
                response.status()
            )));
        }

        let payload = response
            .json::<ChromaQueryResponse>()
            .await
            .map_err(|error| AppError::Io(format!("invalid chroma query payload: {error}")))?;
        let ids = payload.ids.into_iter().next().unwrap_or_default();
        let distances = payload
            .distances
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or_default();

        Ok(ids
            .into_iter()
            .enumerate()
            .map(|(index, task_id)| VectorQueryHit {
                task_id,
                distance: distances.get(index).copied(),
            })
            .collect())
    }

    async fn process_job_batch(&self, documents: &[TaskVectorDocument]) -> AppResult<()> {
        self.ensure_vector_ready().await?;
        self.upsert_documents(documents).await
    }

    async fn ensure_vector_ready(&self) -> AppResult<()> {
        if !self.vector_enabled() {
            return Err(AppError::InvalidArguments(
                "vector search is disabled".to_string(),
            ));
        }

        if self.heartbeat().await {
            return Ok(());
        }

        if self.autostart_sidecar_enabled() {
            self.start_sidecar().await?;
        }

        if self.heartbeat().await {
            Ok(())
        } else {
            Err(AppError::Io(
                "chroma endpoint is unavailable; lexical fallback required".to_string(),
            ))
        }
    }

    async fn upsert_documents(&self, documents: &[TaskVectorDocument]) -> AppResult<()> {
        if documents.is_empty() {
            return Ok(());
        }

        let collection_id = self.ensure_collection_id().await?;
        let inputs = documents
            .iter()
            .map(|document| document.document.clone())
            .collect::<Vec<_>>();
        let embeddings = self.embed_texts(&inputs).await?;
        if embeddings.len() != documents.len() {
            return Err(AppError::Io(format!(
                "embedding response count mismatch: expected {}, got {}",
                documents.len(),
                embeddings.len()
            )));
        }
        let payload = json!({
            "ids": documents
                .iter()
                .map(|document| document.task_id.clone())
                .collect::<Vec<_>>(),
            "embeddings": embeddings,
            "documents": inputs,
            "metadatas": documents
                .iter()
                .map(|document| {
                    json!({
                        "project_id": document.project_id,
                        "project_slug": document.project_slug,
                        "version_id": document.version_id,
                        "task_kind": document.task_kind,
                        "status": document.status,
                        "priority": document.priority,
                        "knowledge_status": document.knowledge_status,
                        "updated_at": document.updated_at,
                    })
                })
                .collect::<Vec<_>>(),
        });
        let response = self
            .inner
            .http
            .post(self.collection_endpoint(&collection_id, "upsert")?)
            .json(&payload)
            .send()
            .await
            .map_err(|error| AppError::Io(format!("chroma upsert request failed: {error}")))?;
        if response.status().is_success() {
            return Ok(());
        }

        Err(AppError::Io(format!(
            "chroma upsert failed with status {}",
            response.status()
        )))
    }

    async fn ensure_collection_id(&self) -> AppResult<String> {
        if let Some(collection_id) = self.inner.collection_id.lock().await.clone() {
            return Ok(collection_id);
        }

        let response = self
            .inner
            .http
            .post(self.collections_endpoint()?)
            .json(&json!({
                "name": self.inner.config.vector.collection,
                "get_or_create": true,
            }))
            .send()
            .await
            .map_err(|error| {
                AppError::Io(format!("failed to create chroma collection: {error}"))
            })?;
        if !response.status().is_success() {
            return Err(AppError::Io(format!(
                "failed to create chroma collection: {}",
                response.status()
            )));
        }

        let collection = response
            .json::<ChromaCollectionRecord>()
            .await
            .map_err(|error| AppError::Io(format!("invalid chroma collection payload: {error}")))?;
        let collection_id = collection.id;
        *self.inner.collection_id.lock().await = Some(collection_id.clone());
        Ok(collection_id)
    }

    async fn embed_text(&self, input: &str) -> AppResult<Vec<f32>> {
        let embeddings = self.embed_texts(&[input.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Io("embedding response missing vector".to_string()))
    }

    async fn embed_texts(&self, inputs: &[String]) -> AppResult<Vec<Vec<f32>>> {
        match self.inner.config.embedding.provider {
            SearchEmbeddingProvider::OpenAiCompatible => {}
        }

        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let response = self
            .inner
            .http
            .post(self.embedding_endpoint()?)
            .header(
                AUTHORIZATION,
                format!("Bearer {}", self.inner.config.embedding.api_key),
            )
            .header(CONTENT_TYPE, "application/json")
            .timeout(Duration::from_millis(
                self.inner.config.embedding.timeout_ms,
            ))
            .json(&json!({
                "model": self.inner.config.embedding.model,
                "input": inputs,
            }))
            .send()
            .await
            .map_err(|error| AppError::Io(format!("embedding request failed: {error}")))?;
        if !response.status().is_success() {
            return Err(AppError::Io(format!(
                "embedding request failed with status {}",
                response.status()
            )));
        }

        let payload = response
            .json::<OpenAiEmbeddingResponse>()
            .await
            .map_err(|error| AppError::Io(format!("invalid embedding response: {error}")))?;
        Ok(payload
            .data
            .into_iter()
            .map(|item| item.embedding)
            .collect())
    }

    async fn heartbeat(&self) -> bool {
        match self
            .inner
            .http
            .get(match self.heartbeat_endpoint() {
                Ok(url) => url,
                Err(_) => return false,
            })
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    async fn wait_for_heartbeat(&self) -> bool {
        for _ in 0..20 {
            if self.heartbeat().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        false
    }

    fn collections_endpoint(&self) -> AppResult<String> {
        Ok(format!(
            "{}/api/v2/tenants/default_tenant/databases/default_database/collections",
            self.base_endpoint()?
        ))
    }

    fn collection_endpoint(&self, collection_id: &str, action: &str) -> AppResult<String> {
        Ok(format!(
            "{}/api/v2/tenants/default_tenant/databases/default_database/collections/{collection_id}/{action}",
            self.base_endpoint()?
        ))
    }

    fn heartbeat_endpoint(&self) -> AppResult<String> {
        Ok(format!("{}/api/v2/heartbeat", self.base_endpoint()?))
    }

    fn embedding_endpoint(&self) -> AppResult<String> {
        let base_url = self.inner.config.embedding.base_url.trim_end_matches('/');
        let prefix = if base_url.ends_with("/v1") {
            base_url.to_string()
        } else {
            format!("{base_url}/v1")
        };
        Ok(format!("{prefix}/embeddings"))
    }

    fn base_endpoint(&self) -> AppResult<String> {
        if self.inner.config.vector.backend != SearchVectorBackend::Chroma {
            return Err(AppError::InvalidArguments(
                "unsupported vector backend".to_string(),
            ));
        }

        let mut base = self.inner.config.vector.endpoint.to_string();
        while base.ends_with('/') {
            base.pop();
        }
        Ok(base)
    }
}

fn chroma_where_clause(filter: &TaskListFilter) -> Option<Map<String, Value>> {
    let mut clauses = Vec::<Value>::new();
    if let Some(project_id) = filter.project_id {
        let mut clause = Map::new();
        clause.insert("project_id".to_string(), Value::String(project_id.to_string()));
        clauses.push(Value::Object(clause));
    }
    if let Some(version_id) = filter.version_id {
        let mut clause = Map::new();
        clause.insert("version_id".to_string(), Value::String(version_id.to_string()));
        clauses.push(Value::Object(clause));
    }
    if let Some(task_kind) = filter.task_kind {
        let mut clause = Map::new();
        clause.insert("task_kind".to_string(), Value::String(task_kind.to_string()));
        clauses.push(Value::Object(clause));
    }
    match clauses.len() {
        0 => None,
        1 => clauses
            .into_iter()
            .next()
            .and_then(|value| value.as_object().cloned()),
        _ => {
            let mut output = Map::new();
            output.insert("$and".to_string(), Value::Array(clauses));
            Some(output)
        }
    }
}

fn backoff_seconds(attempt_count: i64) -> i64 {
    match attempt_count {
        i64::MIN..=0 => 5,
        1 => 15,
        2 => 60,
        3 => 5 * 60,
        _ => 15 * 60,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use uuid::Uuid;

    use super::chroma_where_clause;
    use crate::domain::TaskKind;
    use crate::storage::TaskListFilter;

    #[test]
    fn chroma_where_clause_returns_single_field_directly() {
        let project_id = Uuid::nil();
        let clause = chroma_where_clause(&TaskListFilter {
            project_id: Some(project_id),
            ..Default::default()
        })
        .expect("single clause");

        assert_eq!(
            clause.get("project_id"),
            Some(&Value::String(project_id.to_string()))
        );
        assert!(!clause.contains_key("$and"));
    }

    #[test]
    fn chroma_where_clause_uses_and_for_multiple_filters() {
        let project_id = Uuid::nil();
        let version_id = Uuid::from_u128(1);
        let clause = chroma_where_clause(&TaskListFilter {
            project_id: Some(project_id),
            version_id: Some(version_id),
            task_kind: Some(TaskKind::Standard),
            ..Default::default()
        })
        .expect("compound clause");

        let and_clauses = clause
            .get("$and")
            .and_then(Value::as_array)
            .expect("$and array");
        assert_eq!(and_clauses.len(), 3);
        assert!(and_clauses.iter().any(|value| {
            value.as_object()
                .and_then(|item| item.get("project_id"))
                == Some(&Value::String(project_id.to_string()))
        }));
        assert!(and_clauses.iter().any(|value| {
            value.as_object()
                .and_then(|item| item.get("version_id"))
                == Some(&Value::String(version_id.to_string()))
        }));
        assert!(and_clauses.iter().any(|value| {
            value.as_object()
                .and_then(|item| item.get("task_kind"))
                == Some(&Value::String("standard".to_string()))
        }));
    }
}
