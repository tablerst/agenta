export interface SuccessEnvelope<T = unknown> {
  ok: true;
  action: string;
  result: T;
  summary: string;
  warnings: string[];
}

export interface ErrorEnvelope {
  ok: false;
  error: {
    code: string;
    message: string;
    details: unknown;
  };
}

export type AppLocale = "zh-CN" | "en";
export type ThemeMode = "dark" | "light" | "system";

export type ProjectStatus = "active" | "archived";
export type VersionStatus = "planning" | "active" | "closed" | "archived";
export type TaskStatus =
  | "draft"
  | "ready"
  | "in_progress"
  | "blocked"
  | "done"
  | "cancelled";
export type TaskPriority = "low" | "normal" | "high" | "critical";
export type TaskKind = "standard" | "context" | "index";
export type KnowledgeStatus = "empty" | "working" | "reusable";
export type NoteKind = "scratch" | "finding" | "conclusion";
export type TaskRelationKind = "parent_child" | "blocks";
export type TaskRelationStatus = "active" | "resolved";
export type AttachmentKind =
  | "screenshot"
  | "image"
  | "log"
  | "report"
  | "patch"
  | "artifact"
  | "other";
export type ApprovalStatus = "pending" | "approved" | "denied" | "failed";
export type ApprovalRequestedVia = "cli" | "mcp" | "desktop";
export type ApprovalScope = "project" | "all";
export type ContextInitStatus =
  | "created"
  | "updated"
  | "unchanged"
  | "would_create"
  | "would_update";

export interface Project {
  project_id: string;
  slug: string;
  name: string;
  description: string | null;
  status: ProjectStatus;
  default_version_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface Version {
  version_id: string;
  project_id: string;
  name: string;
  description: string | null;
  status: VersionStatus;
  created_at: string;
  updated_at: string;
}

export interface ContextInitResult {
  project: string;
  context_dir: string;
  manifest_path: string;
  status: ContextInitStatus;
  used_defaults: boolean;
  entry_task_id: string | null;
  entry_task_code: string | null;
}

export interface Task {
  task_id: string;
  project_id: string;
  version_id: string | null;
  task_code: string | null;
  task_kind: TaskKind;
  title: string;
  summary: string | null;
  description: string | null;
  task_search_summary: string;
  task_context_digest: string;
  latest_note_summary: string | null;
  knowledge_status: KnowledgeStatus;
  status: TaskStatus;
  priority: TaskPriority;
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;
  closed_at: string | null;
  note_count: number;
  attachment_count: number;
  latest_activity_at: string;
  parent_task_id: string | null;
  child_count: number;
  open_blocker_count: number;
  blocking_count: number;
  ready_to_start: boolean;
}

export interface TaskActivity {
  activity_id: string;
  task_id: string;
  kind: "note" | "status_change" | "system" | "attachment_ref";
  note_kind?: NoteKind;
  content: string;
  activity_search_summary: string;
  created_by: string;
  created_at: string;
  metadata_json: Record<string, unknown>;
}

export interface TaskRelation {
  relation_id: string;
  kind: TaskRelationKind;
  source_task_id: string;
  target_task_id: string;
  status: TaskRelationStatus;
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;
  resolved_at: string | null;
}

export interface TaskLink {
  relation_id: string;
  task_id: string;
  title: string;
  status: TaskStatus;
  priority: TaskPriority;
  ready_to_start: boolean;
}

export interface TaskContextPayload {
  task: Task;
  notes: TaskActivity[];
  attachments: Attachment[];
  recent_activities: TaskActivity[];
  parent: TaskLink | null;
  children: TaskLink[];
  blocked_by: TaskLink[];
  blocking: TaskLink[];
}

export interface TaskStatusCounts {
  draft: number;
  ready: number;
  in_progress: number;
  blocked: number;
  done: number;
  cancelled: number;
}

export interface TaskKnowledgeCounts {
  empty: number;
  working: number;
  reusable: number;
}

export interface TaskKindCounts {
  standard: number;
  context: number;
  index: number;
}

export interface TaskListSummary {
  total: number;
  status_counts: TaskStatusCounts;
  knowledge_counts: TaskKnowledgeCounts;
  kind_counts: TaskKindCounts;
  ready_to_start_count: number;
}

export interface TaskListPageInfo {
  limit: number | null;
  next_cursor: string | null;
  has_more: boolean;
  sort_by: string;
  sort_order: "asc" | "desc";
}

export interface TaskListPayload {
  tasks: Task[];
  summary: TaskListSummary;
  page: TaskListPageInfo;
}

export interface Attachment {
  attachment_id: string;
  task_id: string;
  kind: AttachmentKind;
  mime: string;
  original_filename: string;
  original_path: string;
  storage_path: string;
  sha256: string;
  size_bytes: number;
  summary: string;
  created_by: string;
  created_at: string;
}

export interface ApprovalRequest {
  request_id: string;
  action: string;
  requested_via: ApprovalRequestedVia;
  resource_ref: string;
  project_ref: string | null;
  project_name: string | null;
  task_ref: string | null;
  payload_json: unknown;
  request_summary: string;
  requested_at: string;
  requested_by: string;
  reviewed_at: string | null;
  reviewed_by: string | null;
  review_note: string | null;
  result_json: unknown | null;
  error_json: unknown | null;
  status: ApprovalStatus;
}

export interface BuildInfo {
  version: string;
  display_version: string;
  git_commit: string | null;
  git_commit_short: string | null;
  git_describe: string | null;
  git_dirty: boolean;
}

export interface RuntimeStatus {
  build: BuildInfo;
  data_dir: string;
  database_path: string;
  attachments_dir: string;
  error_log_path: string;
  loaded_config_path: string | null;
  mcp_bind: string;
  mcp_path: string;
  project_count: number;
  task_count: number;
  pending_approval_count: number;
}

export type McpLifecycleState = "stopped" | "starting" | "running" | "stopping" | "failed";
export type McpLogLevel = "trace" | "debug" | "info" | "warn" | "error";
export type McpLogDestination = "ui" | "stdout" | "file";
export type SyncMode = "manual_bidirectional";
export type SyncRemoteKind = "postgres";
export type SyncOutboxStatus = "pending" | "acked" | "failed";
export type SyncEntityKind = "project" | "version" | "task" | "task_relation" | "note" | "attachment";
export type SyncOperation = "create" | "update";

export interface McpRuntimeStatus {
  state: McpLifecycleState;
  session_id: string | null;
  bind: string;
  actual_bind: string | null;
  path: string;
  autostart: boolean;
  log_level: McpLogLevel;
  log_destinations: McpLogDestination[];
  log_file_path: string;
  log_ui_buffer_lines: number;
  last_error: string | null;
}

export interface McpLaunchOverrides {
  bind?: string | null;
  path?: string | null;
  autostart?: boolean | null;
  log_level?: McpLogLevel | null;
  log_destinations?: McpLogDestination[] | null;
  log_file_path?: string | null;
  log_ui_buffer_lines?: number | null;
  save_as_default?: boolean | null;
}

export interface McpLogEntry {
  session_id: string;
  timestamp: string;
  level: McpLogLevel;
  component: string;
  message: string;
  fields: Record<string, unknown>;
}

export interface McpLogSnapshot {
  session_id: string | null;
  entries: McpLogEntry[];
}

export interface SyncCheckpointStatus {
  pull: string | null;
  push_ack: string | null;
}

export interface SyncPostgresRemoteStatus {
  host: string | null;
  port: number | null;
  database: string | null;
  max_conns: number;
  min_conns: number;
  max_conn_lifetime: string;
}

export interface SyncRemoteStatus {
  id: string;
  kind: SyncRemoteKind;
  postgres: SyncPostgresRemoteStatus | null;
}

export interface SyncStatusSummary {
  enabled: boolean;
  mode: SyncMode;
  remote: SyncRemoteStatus | null;
  pending_outbox_count: number;
  oldest_pending_at: string | null;
  checkpoints: SyncCheckpointStatus;
  auto: SyncAutoStatus;
  conflict_count: number;
}

export interface SyncAutoStatus {
  enabled: boolean;
  running: boolean;
  interval_seconds: number;
  batch_limit: number;
  startup_backfill: boolean;
  last_started_at: string | null;
  last_finished_at: string | null;
  last_error: string | null;
  paused_reason: string | null;
}

export interface SyncOutboxListItem {
  mutation_id: string;
  entity_kind: SyncEntityKind;
  local_id: string;
  operation: SyncOperation;
  local_version: number;
  status: SyncOutboxStatus;
  created_at: string;
  attempt_count: number;
  last_error: string | null;
}

export interface SyncBackfillSummary {
  scanned: number;
  queued: number;
  skipped: number;
  queued_projects: number;
  queued_versions: number;
  queued_tasks: number;
  queued_notes: number;
  queued_attachments: number;
}

export interface SyncPushSummary {
  attempted: number;
  pushed: number;
  failed: number;
  last_remote_mutation_id: number | null;
}

export interface SyncPullSummary {
  fetched: number;
  applied: number;
  skipped: number;
  last_remote_mutation_id: number | null;
}

export interface SearchBackfillSummary {
  run_id: string;
  status: string;
  operation_kind: string;
  operation_description: string;
  scanned: number;
  queued: number;
  skipped: number;
  processed: number;
  succeeded: number;
  failed: number;
  pending_after: number;
  processing_error: string | null;
}

export interface SearchIndexRunSummary {
  run_id: string;
  status: string;
  trigger_kind: string;
  operation_kind: string;
  operation_description: string;
  scanned: number;
  queued: number;
  skipped: number;
  processed: number;
  succeeded: number;
  failed: number;
  batch_size: number;
  pending_count: number;
  processing_count: number;
  retrying_count: number;
  remaining_count: number;
  started_at: string;
  finished_at: string | null;
  last_error: string | null;
  updated_at: string;
}

export interface SearchIndexJobSummary {
  task_id: string;
  title: string | null;
  status: string;
  attempt_count: number;
  last_error: string | null;
  next_attempt_at: string | null;
  locked_at: string | null;
  lease_until: string | null;
  updated_at: string;
  run_id: string | null;
}

export interface SearchIndexStatusSummary {
  enabled: boolean;
  vector_available: boolean;
  sidecar: string;
  total_count: number;
  pending_count: number;
  processing_count: number;
  failed_count: number;
  due_count: number;
  stale_processing_count: number;
  next_retry_at: string | null;
  last_error: string | null;
  active_run: SearchIndexRunSummary | null;
  latest_run: SearchIndexRunSummary | null;
  failed_jobs: SearchIndexJobSummary[];
}

export interface SearchQueueRecoverySummary {
  run_id: string;
  status: string;
  trigger_kind: string;
  operation_kind: string;
  operation_description: string;
  queued: number;
  processed: number;
  succeeded: number;
  failed: number;
  pending_after: number;
  processing_error: string | null;
}

export interface ProjectSearchFilters {
  project: string;
  query: string;
  version?: string;
  status?: TaskStatus;
  priority?: TaskPriority;
  knowledge_status?: KnowledgeStatus;
  task_kind?: TaskKind;
  task_code_prefix?: string;
  limit?: number;
}

export interface GlobalSearchFilters {
  priority?: TaskPriority;
  knowledge_status?: KnowledgeStatus;
  task_kind?: TaskKind;
}

export interface SearchTaskHit {
  task_id: string;
  project_id: string;
  version_id: string | null;
  task_code: string | null;
  task_kind: TaskKind;
  title: string;
  status: TaskStatus;
  priority: TaskPriority;
  knowledge_status: KnowledgeStatus;
  summary: string;
  retrieval_source: "structured_filter" | "lexical" | "semantic" | "hybrid";
  score: number | null;
  matched_fields: string[];
  evidence_source: string | null;
  evidence_snippet: string | null;
  evidence_activity_id: string | null;
  evidence_chunk_id: string | null;
  evidence_attachment_id: string | null;
}

export interface SearchActivityHit {
  activity_id: string;
  task_id: string;
  project_id: string;
  version_id: string | null;
  task_title: string;
  kind: TaskActivity["kind"];
  summary: string;
  retrieval_source: "lexical";
  score: number | null;
  matched_fields: string[];
  evidence_source: string | null;
  evidence_snippet: string | null;
  evidence_chunk_id: string | null;
  evidence_attachment_id: string | null;
}

export interface SearchMeta {
  indexed_fields: {
    tasks: string[];
    activities: string[];
  };
  task_sort: string;
  activity_sort: string;
  limit_applies_per_bucket: boolean;
  task_limit_applied: number;
  activity_limit_applied: number;
  default_limit: number;
  max_limit: number;
  retrieval_mode: "structured_only" | "lexical_only" | "hybrid";
  vector_backend: string | null;
  vector_status: "disabled" | "ready" | "indexing" | "lexical_fallback";
  pending_index_jobs: number;
  semantic_attempted: boolean;
  semantic_used: boolean;
  semantic_error: string | null;
  semantic_candidate_count: number;
}

export interface SearchResponse {
  query: string | null;
  tasks: SearchTaskHit[];
  activities: SearchActivityHit[];
  meta: SearchMeta;
}

export interface SearchEvidenceDetail {
  source_kind: string;
  task_id: string;
  project_id: string;
  version_id: string | null;
  task_title: string;
  activity_id: string | null;
  chunk_id: string | null;
  chunk_index: number | null;
  attachment_id: string | null;
  activity_kind: string | null;
  summary: string;
  text: string;
}

export interface AppBridgeError {
  code: string;
  message: string;
  details: unknown;
}
