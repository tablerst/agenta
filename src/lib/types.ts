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
  mcp_bind: string;
  mcp_path: string;
  project_count: number;
  task_count: number;
  pending_approval_count: number;
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
