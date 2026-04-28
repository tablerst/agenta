use std::collections::{HashMap, HashSet};
use std::fs as std_fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, Sqlite, Transaction};
use time::OffsetDateTime;
use tokio::fs;
use tokio::sync::Mutex;
use url::Url;
use uuid::Uuid;

use crate::app::{
    ProjectContextConfig, SearchConfig, SyncConfig, SyncRemoteConfig, SyncRemoteKind,
};
use crate::domain::{
    ApprovalRequest, ApprovalRequestedVia, ApprovalStatus, Attachment, AttachmentKind,
    KnowledgeStatus, NoteKind, Project, ProjectStatus, SyncCheckpointKind, SyncEntityKind,
    SyncMode, SyncOperation, SyncOutboxStatus, Task, TaskActivity, TaskActivityKind, TaskKind,
    TaskPriority, TaskRelation, TaskRelationKind, TaskRelationStatus, TaskStatus, Version,
    VersionStatus,
};
use crate::error::{AppError, AppResult};
use crate::policy::{PolicyEngine, WriteDecision};
use crate::search::{
    build_activity_search_summary, build_activity_search_text, build_search_evidence,
    build_task_context_digest, build_task_search_summary, matched_field_names,
    normalize_search_query, weighted_rrf_score, ActivitySearchHit, SearchEvidence,
    SearchIndexedFields, SearchMeta, SearchResponse, SearchRuntime, SearchSidecarStatus,
    TaskSearchHit, DEFAULT_SEARCH_LIMIT, LEXICAL_RRF_WEIGHT, MAX_SEARCH_LIMIT, SEMANTIC_RRF_WEIGHT,
};
use crate::storage::{SearchIndexJobRecord, SearchIndexRunRecord, SqliteStore, TaskListFilter};
use crate::sync::{PostgresSyncRemote, RemoteMutation};

mod activities;
mod approval_helpers;
mod approvals;
mod attachments;
mod context;
mod context_helpers;
mod overview;
mod pagination;
mod projects;
mod relations;
mod search;
mod search_helpers;
mod shared;
mod sync;
mod sync_helpers;
mod tasks;
mod types;
mod versions;

use approval_helpers::*;
use context_helpers::*;
use pagination::*;
use search_helpers::*;
use shared::*;
use sync_helpers::*;
use types::{
    ApprovalContext, ApprovalMode, ApprovalSeed, ContextInitTarget, ProjectContextManifest,
    ReferencedUpdatePayload,
};

pub use types::*;

#[derive(Clone)]
pub struct AgentaService {
    store: SqliteStore,
    policy: PolicyEngine,
    sync: SyncConfig,
    search: SearchRuntime,
    project_context: ProjectContextConfig,
    write_queue: Arc<Mutex<()>>,
    sync_run_lock: Arc<Mutex<()>>,
}

impl AgentaService {
    pub fn new(
        store: SqliteStore,
        policy: PolicyEngine,
        sync: SyncConfig,
        search_config: SearchConfig,
        project_context: ProjectContextConfig,
        error_log_path: PathBuf,
    ) -> AppResult<Self> {
        let search = SearchRuntime::new(search_config, Some(error_log_path))?;
        search.trigger_index_worker(store.clone());
        Ok(Self {
            store,
            policy,
            sync,
            search,
            project_context,
            write_queue: Arc::new(Mutex::new(())),
            sync_run_lock: Arc::new(Mutex::new(())),
        })
    }
}
