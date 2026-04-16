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

export interface Task {
  task_id: string;
  project_id: string;
  version_id: string | null;
  title: string;
  summary: string | null;
  description: string | null;
  task_search_summary: string;
  task_context_digest: string;
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

export interface RuntimeStatus {
  data_dir: string;
  database_path: string;
  attachments_dir: string;
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

export interface SearchTaskHit {
  task_id: string;
  title: string;
  status: TaskStatus;
  priority: TaskPriority;
  summary: string;
}

export interface SearchActivityHit {
  activity_id: string;
  task_id: string;
  kind: TaskActivity["kind"];
  summary: string;
}

export interface SearchResponse {
  query: string;
  tasks: SearchTaskHit[];
  activities: SearchActivityHit[];
}

export interface AppBridgeError {
  code: string;
  message: string;
  details: unknown;
}
