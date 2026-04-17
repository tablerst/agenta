import type {
  AppBridgeError,
  ApprovalRequest,
  ApprovalStatus,
  Attachment,
  AttachmentKind,
  McpLaunchOverrides,
  McpLogEntry,
  McpLogSnapshot,
  McpLogLevel,
  McpRuntimeStatus,
  NoteKind,
  Project,
  RuntimeStatus,
  SearchBackfillSummary,
  SearchResponse,
  SuccessEnvelope,
  SyncBackfillSummary,
  SyncEntityKind,
  SyncOutboxListItem,
  SyncPullSummary,
  SyncPushSummary,
  SyncStatusSummary,
  Task,
  TaskActivity,
  TaskContextPayload,
  TaskKind,
  TaskLink,
  TaskListPayload,
  TaskPriority,
  TaskRelation,
  TaskRelationStatus,
  TaskStatus,
  Version,
  VersionStatus,
  KnowledgeStatus,
} from "./types";

type JsonMap = Record<string, unknown>;

interface MockState {
  approvals: ApprovalRequest[];
  attachments: Attachment[];
  projects: Project[];
  tasks: Task[];
  taskActivities: TaskActivity[];
  taskRelations: TaskRelation[];
  versions: Version[];
}

const PREVIEW_WARNING = "Running in browser preview mode with seeded local data.";

const previewNow = Date.now();
const PREVIEW_MCP_LOG_FILE = "D:/preview/agenta/data/logs/mcp.jsonl";
const PREVIEW_SYNC_REMOTE_ID = "preview-primary";

let mcpRuntime = createPreviewMcpRuntime();
let mcpLogs: McpLogEntry[] = [];
let syncOutbox: SyncOutboxListItem[] = [];
let syncPullCheckpoint: string | null = null;
let syncPushAckCheckpoint: string | null = null;
let previewRemoteMutationCursor = 12;

function iso(hoursAgo: number) {
  return new Date(previewNow - hoursAgo * 60 * 60 * 1000).toISOString();
}

function createPreviewMcpRuntime(overrides: Partial<McpRuntimeStatus> = {}): McpRuntimeStatus {
  return {
    state: "stopped",
    session_id: null,
    bind: "127.0.0.1:8787",
    actual_bind: null,
    path: "/mcp",
    autostart: false,
    log_level: "info",
    log_destinations: ["ui", "file"],
    log_file_path: PREVIEW_MCP_LOG_FILE,
    log_ui_buffer_lines: 1000,
    last_error: null,
    ...overrides,
  };
}

function normalizeMcpPath(path?: string | null) {
  const trimmed = path?.trim() ?? "";
  if (!trimmed) {
    return "/mcp";
  }
  if (trimmed === "/") {
    return "/mcp";
  }
  return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
}

function resolveActualBind(bind: string) {
  if (bind.endsWith(":0")) {
    return "127.0.0.1:9450";
  }
  return bind;
}

function pushMcpLog(
  level: McpLogLevel,
  component: string,
  message: string,
  fields: Record<string, unknown> = {},
) {
  if (!mcpRuntime.session_id) {
    return;
  }
  const entry: McpLogEntry = {
    session_id: mcpRuntime.session_id,
    timestamp: new Date().toISOString(),
    level,
    component,
    message,
    fields,
  };
  mcpLogs = [...mcpLogs, entry].slice(-mcpRuntime.log_ui_buffer_lines);
}

function startPreviewMcp(input: McpLaunchOverrides = {}) {
  const bind = input.bind?.trim() || mcpRuntime.bind;
  const path = normalizeMcpPath(input.path);
  const nextDestinations =
    input.log_destinations && input.log_destinations.length > 0
      ? [...input.log_destinations]
      : [...mcpRuntime.log_destinations];
  mcpLogs = [];
  mcpRuntime = createPreviewMcpRuntime({
    ...mcpRuntime,
    state: "running",
    session_id: crypto.randomUUID(),
    bind,
    actual_bind: resolveActualBind(bind),
    path,
    autostart: input.autostart ?? mcpRuntime.autostart,
    log_level: input.log_level ?? mcpRuntime.log_level,
    log_destinations: nextDestinations,
    log_file_path: input.log_file_path?.trim() || mcpRuntime.log_file_path,
    log_ui_buffer_lines:
      typeof input.log_ui_buffer_lines === "number" && input.log_ui_buffer_lines > 0
        ? input.log_ui_buffer_lines
        : mcpRuntime.log_ui_buffer_lines,
    last_error: null,
  });
  pushMcpLog("info", "mcp_supervisor", "Starting preview desktop-managed MCP host", {
    bind: mcpRuntime.bind,
    path: mcpRuntime.path,
  });
  pushMcpLog("info", "mcp_supervisor", "Preview desktop-managed MCP host is running", {
    actual_bind: mcpRuntime.actual_bind,
    path: mcpRuntime.path,
  });
}

function stopPreviewMcp() {
  if (mcpRuntime.state === "running") {
    pushMcpLog("info", "mcp_supervisor", "Stopping preview desktop-managed MCP host");
  }
  mcpRuntime = createPreviewMcpRuntime({
    ...mcpRuntime,
    state: "stopped",
    session_id: null,
    actual_bind: null,
    last_error: null,
  });
}

function createPreviewSyncStatus(): SyncStatusSummary {
  const pending = syncOutbox.filter((item) => item.status === "pending");
  return {
    enabled: true,
    mode: "manual_bidirectional",
    remote: {
      id: PREVIEW_SYNC_REMOTE_ID,
      kind: "postgres",
      postgres: {
        host: "preview.db.local",
        port: 5432,
        database: "agenta_preview",
        max_conns: 30,
        min_conns: 5,
        max_conn_lifetime: "1h",
      },
    },
    pending_outbox_count: pending.length,
    oldest_pending_at: pending.length > 0 ? pending[pending.length - 1]?.created_at ?? null : null,
    checkpoints: {
      pull: syncPullCheckpoint,
      push_ack: syncPushAckCheckpoint,
    },
  };
}

function syncEntityKindFor(
  value: Project | Version | Task | TaskActivity | Attachment | TaskRelation,
): SyncEntityKind {
  if ("project_id" in value && "slug" in value) {
    return "project";
  }
  if ("project_id" in value && "version_id" in value && "name" in value) {
    return "version";
  }
  if ("task_id" in value && "title" in value) {
    return "task";
  }
  if ("relation_id" in value) {
    return "task_relation";
  }
  if ("attachment_id" in value) {
    return "attachment";
  }
  return "note";
}

function syncLocalIdFor(
  value: Project | Version | Task | TaskActivity | Attachment | TaskRelation,
): string {
  if ("project_id" in value && "slug" in value) {
    return value.project_id;
  }
  if ("project_id" in value && "version_id" in value && "name" in value) {
    return value.version_id;
  }
  if ("task_id" in value && "title" in value) {
    return value.task_id;
  }
  if ("relation_id" in value) {
    return value.relation_id;
  }
  if ("attachment_id" in value) {
    return value.attachment_id;
  }
  return value.activity_id;
}

function enqueuePreviewMutation(
  value: Project | Version | Task | TaskActivity | Attachment | TaskRelation,
  operation: "create" | "update" = "create",
) {
  const entity_kind = syncEntityKindFor(value);
  const local_id = syncLocalIdFor(value);
  const existing = syncOutbox.find((item) => item.entity_kind === entity_kind && item.local_id === local_id);
  if (existing) {
    if (operation === "update") {
      existing.operation = "update";
      existing.status = "pending";
      existing.last_error = null;
    }
    return false;
  }
  const entry: SyncOutboxListItem = {
    mutation_id: crypto.randomUUID(),
    entity_kind,
    local_id,
    operation,
    local_version: 1,
    status: "pending",
    created_at: new Date().toISOString(),
    attempt_count: 0,
    last_error: null,
  };
  syncOutbox = [entry, ...syncOutbox];
  return true;
}

function runPreviewBackfill(limit?: number): SyncBackfillSummary {
  const maxToQueue = typeof limit === "number" ? Math.max(1, limit) : 1000;
  const summary: SyncBackfillSummary = {
    scanned: 0,
    queued: 0,
    skipped: 0,
    queued_projects: 0,
    queued_versions: 0,
    queued_tasks: 0,
    queued_notes: 0,
    queued_attachments: 0,
  };

  const tryQueue = (value: Project | Version | Task | TaskActivity | Attachment) => {
    if (summary.queued >= maxToQueue) {
      return;
    }
    summary.scanned += 1;
    if (enqueuePreviewMutation(value, "create")) {
      summary.queued += 1;
      switch (syncEntityKindFor(value)) {
        case "project":
          summary.queued_projects += 1;
          break;
        case "version":
          summary.queued_versions += 1;
          break;
        case "task":
          summary.queued_tasks += 1;
          break;
        case "note":
          summary.queued_notes += 1;
          break;
        case "attachment":
          summary.queued_attachments += 1;
          break;
      }
    } else {
      summary.skipped += 1;
    }
  };

  listProjects().forEach(tryQueue);
  listVersions().forEach(tryQueue);
  state.tasks.forEach(tryQueue);
  state.taskActivities.filter((item) => item.kind === "note").forEach(tryQueue);
  state.attachments.forEach(tryQueue);

  return summary;
}

function runPreviewPush(limit?: number): SyncPushSummary {
  const maxToPush = typeof limit === "number" ? Math.max(1, limit) : 50;
  const pending = syncOutbox.filter((item) => item.status === "pending").slice(0, maxToPush);
  for (const item of pending) {
    previewRemoteMutationCursor += 1;
    item.status = "acked";
    item.attempt_count += 1;
    item.last_error = null;
    syncPushAckCheckpoint = String(previewRemoteMutationCursor);
  }
  return {
    attempted: pending.length,
    pushed: pending.length,
    failed: 0,
    last_remote_mutation_id: pending.length > 0 ? previewRemoteMutationCursor : null,
  };
}

function runPreviewPull(limit?: number): SyncPullSummary {
  const fetched = typeof limit === "number" ? Math.max(0, Math.min(limit, 1)) : 1;
  if (fetched > 0) {
    syncPullCheckpoint = String(previewRemoteMutationCursor);
  }
  return {
    fetched,
    applied: 0,
    skipped: fetched,
    last_remote_mutation_id: fetched > 0 ? previewRemoteMutationCursor : null,
  };
}

function runPreviewSearchBackfill(options: { limit?: number; batchSize?: number } = {}): SearchBackfillSummary {
  const maxToQueue = typeof options.limit === "number" ? Math.max(1, options.limit) : 1000;
  const scanned = state.tasks.length;
  const queued = Math.min(scanned, maxToQueue);
  return {
    scanned,
    queued,
    skipped: Math.max(0, scanned - queued),
    pending_after: 0,
    processing_error: null,
  };
}

function createSeedState(): MockState {
  const projectAlpha: Project = {
    project_id: "project-alpha",
    slug: "agenta-console",
    name: "Agenta Console",
    description: "Desktop orchestration surface for projects, approvals, and task execution lanes.",
    status: "active",
    default_version_id: "version-alpha-v2",
    created_at: iso(240),
    updated_at: iso(8),
  };
  const projectBeta: Project = {
    project_id: "project-beta",
    slug: "ops-lab",
    name: "Ops Lab",
    description: "Internal sandbox for MCP policy experiments and runtime validation.",
    status: "active",
    default_version_id: "version-beta-v1",
    created_at: iso(300),
    updated_at: iso(16),
  };

  const versions: Version[] = [
    {
      version_id: "version-alpha-v1",
      project_id: projectAlpha.project_id,
      name: "v1 Foundation",
      description: "Initial shell, storage boot, and CLI alignment.",
      status: "closed",
      created_at: iso(220),
      updated_at: iso(180),
    },
    {
      version_id: "version-alpha-v2",
      project_id: projectAlpha.project_id,
      name: "v2 Desktop UX",
      description: "Tighten shell navigation, preview mode, and interaction polish.",
      status: "active",
      created_at: iso(48),
      updated_at: iso(6),
    },
    {
      version_id: "version-beta-v1",
      project_id: projectBeta.project_id,
      name: "Policy Sandbox",
      description: "Review workflows, replay outcomes, and attachment safety rails.",
      status: "planning",
      created_at: iso(72),
      updated_at: iso(24),
    },
  ];

  const tasks: Task[] = [
    createTaskRecord({
      task_id: "task-shell-polish",
      project_id: projectAlpha.project_id,
      version_id: "version-alpha-v2",
      title: "Refine shell navigation",
      summary: "Collapse state, keyboard search, and compact affordances need a calmer desktop rhythm.",
      description: "Audit the shell chrome, smooth the sidebar transition, and reduce dead space on wider screens.",
      status: "in_progress",
      priority: "high",
      created_by: "desktop",
      updated_by: "desktop",
      created_at: iso(18),
      updated_at: iso(2),
      closed_at: null,
    }),
    createTaskRecord({
      task_id: "task-preview-mode",
      project_id: projectAlpha.project_id,
      version_id: "version-alpha-v2",
      title: "Ship browser preview mode",
      summary: "Provide seeded local data so frontend work stays explorable without the Tauri bridge.",
      description: "Mock projects, versions, approvals, and task detail in browser dev mode.",
      status: "ready",
      priority: "critical",
      created_by: "desktop",
      updated_by: "desktop",
      created_at: iso(14),
      updated_at: iso(5),
      closed_at: null,
    }),
    createTaskRecord({
      task_id: "task-approval-queue",
      project_id: projectBeta.project_id,
      version_id: "version-beta-v1",
      title: "Tighten approval queue review copy",
      summary: "Approval inspector should explain request intent, replay result, and next action in one pass.",
      description: "Condense review actions and surface the underlying resource reference more clearly.",
      status: "blocked",
      priority: "normal",
      created_by: "desktop",
      updated_by: "desktop",
      created_at: iso(30),
      updated_at: iso(9),
      closed_at: null,
    }),
    createTaskRecord({
      task_id: "task-runtime-empty-state",
      project_id: projectBeta.project_id,
      version_id: "version-beta-v1",
      title: "Rework runtime empty state",
      summary: "Current runtime view feels like a scaffold instead of an operational surface.",
      description: "Use status tiles, path groups, and payload framing that feels intentional on desktop.",
      status: "draft",
      priority: "low",
      created_by: "desktop",
      updated_by: "desktop",
      created_at: iso(52),
      updated_at: iso(52),
      closed_at: null,
    }),
  ];

  const attachments: Attachment[] = [
    {
      attachment_id: "attachment-shell-audit",
      task_id: "task-shell-polish",
      kind: "report",
      mime: "text/markdown",
      original_filename: "shell-audit.md",
      original_path: "D:/preview/reports/shell-audit.md",
      storage_path: "attachments/task-shell-polish/shell-audit.md",
      sha256: "preview-shell-audit",
      size_bytes: 18240,
      summary: "Shell audit notes",
      created_by: "desktop",
      created_at: iso(3),
    },
    {
      attachment_id: "attachment-queue-flow",
      task_id: "task-approval-queue",
      kind: "image",
      mime: "image/png",
      original_filename: "approval-queue.png",
      original_path: "D:/preview/screens/approval-queue.png",
      storage_path: "attachments/task-approval-queue/approval-queue.png",
      sha256: "preview-approval-queue",
      size_bytes: 845312,
      summary: "Approval queue capture",
      created_by: "desktop",
      created_at: iso(10),
    },
  ];

  const taskActivities: TaskActivity[] = [
    {
      activity_id: "activity-shell-note",
      task_id: "task-shell-polish",
      kind: "note",
      content: "Collapsed navigation still loses orientation after expanding back into the workbench.",
      activity_search_summary: "note: collapsed navigation loses orientation after expanding back into the workbench",
      created_by: "desktop",
      created_at: iso(7),
      metadata_json: {},
    },
    {
      activity_id: "activity-shell-attachment",
      task_id: "task-shell-polish",
      kind: "attachment_ref",
      content: "Shell audit notes",
      activity_search_summary: "attachment_ref: shell audit notes",
      created_by: "desktop",
      created_at: iso(3),
      metadata_json: {
        attachment_id: "attachment-shell-audit",
        storage_path: "attachments/task-shell-polish/shell-audit.md",
      },
    },
    {
      activity_id: "activity-preview-system",
      task_id: "task-preview-mode",
      kind: "system",
      content: "Browser preview seed loaded with local projects, versions, tasks, and approvals.",
      activity_search_summary: "system: browser preview seed loaded with local projects versions tasks and approvals",
      created_by: "system",
      created_at: iso(5),
      metadata_json: {},
    },
    {
      activity_id: "activity-approval-note",
      task_id: "task-approval-queue",
      kind: "note",
      content: "Replay failed once because the original attachment path disappeared before approval review.",
      activity_search_summary: "note: replay failed because attachment path disappeared before approval review",
      created_by: "desktop",
      created_at: iso(11),
      metadata_json: {},
    },
    {
      activity_id: "activity-queue-attachment",
      task_id: "task-approval-queue",
      kind: "attachment_ref",
      content: "Approval queue capture",
      activity_search_summary: "attachment_ref: approval queue capture",
      created_by: "desktop",
      created_at: iso(10),
      metadata_json: {
        attachment_id: "attachment-queue-flow",
        storage_path: "attachments/task-approval-queue/approval-queue.png",
      },
    },
  ];

  const approvals: ApprovalRequest[] = [
    {
      request_id: "approval-project-refresh",
      action: "project.create",
      requested_via: "desktop",
      resource_ref: "agenta-console",
      project_ref: "agenta-console",
      project_name: "Agenta Console",
      task_ref: null,
      payload_json: {
        slug: "agenta-console",
        name: "Agenta Console",
        description: "Desktop orchestration surface for projects, approvals, and task execution lanes.",
      },
      request_summary: "Create project agenta-console",
      requested_at: iso(28),
      requested_by: "desktop",
      reviewed_at: null,
      reviewed_by: null,
      review_note: null,
      result_json: null,
      error_json: null,
      status: "pending",
    },
    {
      request_id: "approval-attachment-replay",
      action: "attachment.create",
      requested_via: "desktop",
      resource_ref: "task-approval-queue",
      project_ref: "ops-lab",
      project_name: "Ops Lab",
      task_ref: "task-approval-queue",
      payload_json: {
        task: "task-approval-queue",
        path: "D:/preview/screens/approval-queue.png",
        summary: "Approval queue capture",
      },
      request_summary: "Add attachment Approval queue capture to task task-approval-queue",
      requested_at: iso(13),
      requested_by: "desktop",
      reviewed_at: iso(9),
      reviewed_by: "reviewer",
      review_note: "Original file vanished before replay.",
      result_json: null,
      error_json: {
        code: "not_found",
        message: "Attachment source file no longer exists.",
      },
      status: "failed",
    },
    {
      request_id: "approval-task-copy",
      action: "task.update",
      requested_via: "mcp",
      resource_ref: "task-shell-polish",
      project_ref: "agenta-console",
      project_name: "Agenta Console",
      task_ref: "task-shell-polish",
      payload_json: {
        task: "task-shell-polish",
        title: "Refine shell navigation",
        summary: "Collapse state, keyboard search, and compact affordances need a calmer desktop rhythm.",
      },
      request_summary: "Update task task-shell-polish",
      requested_at: iso(9),
      requested_by: "mcp",
      reviewed_at: iso(6),
      reviewed_by: "ops-lead",
      review_note: "Copy tightened for release notes.",
      result_json: {
        task_id: "task-shell-polish",
        status: "approved",
      },
      error_json: null,
      status: "approved",
    },
  ];

  return {
    approvals,
    attachments,
    projects: [projectAlpha, projectBeta],
    tasks,
    taskActivities,
    taskRelations: [],
    versions,
  };
}

let state = createSeedState();
refreshTasksDerivedFields(state.tasks.map((item) => item.task_id));

function createTaskRecord(
  task: Omit<
    Task,
    | "task_code"
    | "task_kind"
    | "task_context_digest"
    | "task_search_summary"
    | "latest_note_summary"
    | "knowledge_status"
    | "note_count"
    | "attachment_count"
    | "latest_activity_at"
    | "parent_task_id"
    | "child_count"
    | "open_blocker_count"
    | "blocking_count"
    | "ready_to_start"
  > & {
    task_code?: string | null;
    task_kind?: TaskKind;
  },
): Task {
  return {
    ...task,
    task_code: task.task_code ?? null,
    task_kind: task.task_kind ?? "standard",
    attachment_count: 0,
    blocking_count: 0,
    child_count: 0,
    latest_activity_at: task.updated_at,
    latest_note_summary: null,
    knowledge_status: "empty",
    note_count: 0,
    open_blocker_count: 0,
    parent_task_id: null,
    ready_to_start: !matchesClosedTask(task.status),
    task_context_digest: buildTaskContextDigest(task),
    task_search_summary: buildTaskSearchSummary(
      task.task_code ?? null,
      task.task_kind ?? "standard",
      task.title,
      task.summary,
      task.description,
    ),
  };
}

function buildTaskSearchSummary(
  taskCode: string | null,
  taskKind: TaskKind,
  title: string,
  summary: string | null,
  description: string | null,
) {
  return [taskCode, taskKind, title, summary, description].filter(Boolean).join(" | ");
}

function buildTaskContextDigest(task: {
  description: string | null;
  task_code?: string | null;
  task_kind?: TaskKind;
  latest_note_summary?: string | null;
  knowledge_status?: KnowledgeStatus;
  open_blocker_count?: number;
  parent_task_id?: string | null;
  ready_to_start?: boolean;
  blocking_count?: number;
  child_count?: number;
  priority: TaskPriority;
  status: TaskStatus;
  summary: string | null;
  title: string;
}) {
  return `status=${task.status} priority=${task.priority} task_code=${task.task_code ?? ""} task_kind=${task.task_kind ?? "standard"} knowledge_status=${task.knowledge_status ?? "empty"} latest_note_summary=${task.latest_note_summary ?? ""} ready_to_start=${task.ready_to_start ?? true} parent_task_id=${task.parent_task_id ?? ""} child_count=${task.child_count ?? 0} open_blocker_count=${task.open_blocker_count ?? 0} blocking_count=${task.blocking_count ?? 0} title=${task.title} summary=${task.summary ?? ""} description=${task.description ?? ""}`.trim();
}

function matchesClosedTask(status: TaskStatus) {
  return status === "done" || status === "cancelled";
}

function activeRelationStatus(status: TaskRelationStatus) {
  return status === "active";
}

function activeParentRelation(taskId: string) {
  return (
    state.taskRelations.find(
      (relation) =>
        relation.kind === "parent_child" &&
        activeRelationStatus(relation.status) &&
        relation.target_task_id === taskId,
    ) ?? null
  );
}

function activeChildRelations(taskId: string) {
  return state.taskRelations.filter(
    (relation) =>
      relation.kind === "parent_child" &&
      activeRelationStatus(relation.status) &&
      relation.source_task_id === taskId,
  );
}

function activeBlockerRelations(taskId: string) {
  return state.taskRelations.filter(
    (relation) =>
      relation.kind === "blocks" &&
      activeRelationStatus(relation.status) &&
      relation.target_task_id === taskId,
  );
}

function activeBlockingRelations(taskId: string) {
  return state.taskRelations.filter(
    (relation) =>
      relation.kind === "blocks" &&
      activeRelationStatus(relation.status) &&
      relation.source_task_id === taskId,
  );
}

function openBlockerCount(taskId: string) {
  return activeBlockerRelations(taskId).filter((relation) => {
    const blocker = state.tasks.find((item) => item.task_id === relation.source_task_id);
    return blocker ? !matchesClosedTask(blocker.status) : false;
  }).length;
}

function noteKindForActivity(activity: TaskActivity): NoteKind {
  const noteKind =
    activity.metadata_json && typeof activity.metadata_json.note_kind === "string"
      ? activity.metadata_json.note_kind
      : undefined;
  return (noteKind ?? "finding") as NoteKind;
}

function refreshTaskDerivedFields(taskId: string) {
  const task = state.tasks.find((item) => item.task_id === taskId);
  if (!task) {
    return;
  }
  const parentRelation = activeParentRelation(taskId);
  task.parent_task_id = parentRelation?.source_task_id ?? null;
  task.child_count = activeChildRelations(taskId).length;
  task.open_blocker_count = openBlockerCount(taskId);
  task.blocking_count = activeBlockingRelations(taskId).length;
  task.note_count = state.taskActivities.filter((item) => item.task_id === taskId && item.kind === "note").length;
  task.attachment_count = state.attachments.filter((item) => item.task_id === taskId).length;
  const latestActivity = taskActivitiesFor(taskId)[0]?.created_at ?? task.updated_at;
  const latestNote =
    taskActivitiesFor(taskId).find((item) => item.kind === "note") ?? null;
  const hasConclusion = state.taskActivities.some(
    (item) => item.task_id === taskId && item.kind === "note" && noteKindForActivity(item) === "conclusion",
  );
  task.latest_activity_at = latestActivity > task.updated_at ? latestActivity : task.updated_at;
  task.latest_note_summary = latestNote?.activity_search_summary ?? null;
  task.knowledge_status = hasConclusion ? "reusable" : task.note_count > 0 ? "working" : "empty";
  task.ready_to_start = !matchesClosedTask(task.status) && task.open_blocker_count === 0;
  task.task_context_digest = buildTaskContextDigest(task);
  task.task_search_summary = buildTaskSearchSummary(
    task.task_code,
    task.task_kind,
    task.title,
    task.summary,
    task.description,
  );
}

function refreshTasksDerivedFields(taskIds: string[]) {
  const unique = [...new Set(taskIds)];
  for (const taskId of unique) {
    refreshTaskDerivedFields(taskId);
  }
}

function buildTaskLink(relation: TaskRelation, taskId: string): TaskLink {
  const task = findTask(taskId);
  return {
    relation_id: relation.relation_id,
    task_id: task.task_id,
    title: task.title,
    status: task.status,
    priority: task.priority,
    ready_to_start: task.ready_to_start,
  };
}

function getTaskContext(taskId: string): TaskContextPayload {
  const task = findTask(taskId);
  refreshTaskDerivedFields(taskId);
  const parentRelation = activeParentRelation(taskId);
  return {
    task,
    notes: listNotes(taskId),
    attachments: listAttachments(taskId),
    recent_activities: taskActivitiesFor(taskId),
    parent: parentRelation ? buildTaskLink(parentRelation, parentRelation.source_task_id) : null,
    children: activeChildRelations(taskId).map((relation) => buildTaskLink(relation, relation.target_task_id)),
    blocked_by: activeBlockerRelations(taskId).map((relation) => buildTaskLink(relation, relation.source_task_id)),
    blocking: activeBlockingRelations(taskId).map((relation) => buildTaskLink(relation, relation.target_task_id)),
  };
}

function envelope<T>(action: string, result: T, summary: string): SuccessEnvelope<T> {
  return {
    ok: true,
    action,
    result,
    summary,
    warnings: [PREVIEW_WARNING],
  };
}

function bridgeError(code: string, message: string, details: unknown = null): never {
  throw {
    error: {
      code,
      message,
      details,
    } satisfies AppBridgeError,
  };
}

function requireString(value: unknown, field: string) {
  if (typeof value === "string" && value.trim()) {
    return value.trim();
  }
  bridgeError("invalid_arguments", `${field} must not be empty`, { field });
}

function findProject(reference: string) {
  const project = state.projects.find((item) => item.project_id === reference || item.slug === reference);
  if (!project) {
    bridgeError("not_found", `Project not found: ${reference}`, { entity: "project", reference });
  }
  return project;
}

function findVersion(reference: string) {
  const version = state.versions.find((item) => item.version_id === reference);
  if (!version) {
    bridgeError("not_found", `Version not found: ${reference}`, { entity: "version", reference });
  }
  return version;
}

function findTask(reference: string) {
  const task = state.tasks.find((item) => item.task_id === reference);
  if (!task) {
    bridgeError("not_found", `Task not found: ${reference}`, { entity: "task", reference });
  }
  return task;
}

function findApproval(requestId: string) {
  const approval = state.approvals.find((item) => item.request_id === requestId);
  if (!approval) {
    bridgeError("not_found", `Approval request not found: ${requestId}`, {
      entity: "approval_request",
      reference: requestId,
    });
  }
  return approval;
}

function sortByDateDesc<T extends { updated_at?: string; created_at?: string; requested_at?: string }>(items: T[]) {
  return [...items].sort((left, right) => {
    const leftValue = left.updated_at ?? left.requested_at ?? left.created_at ?? "";
    const rightValue = right.updated_at ?? right.requested_at ?? right.created_at ?? "";
    return rightValue.localeCompare(leftValue);
  });
}

function listProjects() {
  return sortByDateDesc(state.projects);
}

function listVersions(projectReference?: unknown) {
  if (typeof projectReference === "string" && projectReference.trim()) {
    const project = findProject(projectReference.trim());
    return sortByDateDesc(state.versions.filter((item) => item.project_id === project.project_id));
  }
  return sortByDateDesc(state.versions);
}

function taskCodeParts(value: string | null) {
  if (!value) {
    return { prefix: "", number: Number.MAX_SAFE_INTEGER, raw: "" };
  }
  const normalized = value.trim().toLowerCase();
  const parts = normalized.match(/^(.*)-(\d+)$/);
  if (!parts) {
    return { prefix: normalized, number: Number.MAX_SAFE_INTEGER, raw: normalized };
  }
  return { prefix: parts[1], number: Number(parts[2]), raw: normalized };
}

function compareTasks(left: Task, right: Task, sortBy: string, sortOrder: "asc" | "desc") {
  let ordering = 0;
  switch (sortBy) {
    case "updated_at":
      ordering = left.updated_at.localeCompare(right.updated_at);
      break;
    case "latest_activity_at":
      ordering = left.latest_activity_at.localeCompare(right.latest_activity_at);
      break;
    case "task_code": {
      const leftParts = taskCodeParts(left.task_code);
      const rightParts = taskCodeParts(right.task_code);
      ordering =
        left.task_code && !right.task_code
          ? -1
          : !left.task_code && right.task_code
            ? 1
            : leftParts.prefix.localeCompare(rightParts.prefix) ||
              leftParts.number - rightParts.number ||
              leftParts.raw.localeCompare(rightParts.raw) ||
              left.title.localeCompare(right.title) ||
              left.task_id.localeCompare(right.task_id);
      break;
    }
    case "title":
      ordering = left.title.localeCompare(right.title) || left.task_id.localeCompare(right.task_id);
      break;
    case "created_at":
    default:
      ordering = left.created_at.localeCompare(right.created_at);
      break;
  }
  return sortOrder === "asc" ? ordering : -ordering;
}

function buildTaskSummary(tasks: Task[]) {
  return tasks.reduce<TaskListPayload["summary"]>(
    (summary, task) => {
      summary.total += 1;
      summary.status_counts[task.status] += 1;
      summary.knowledge_counts[task.knowledge_status] += 1;
      summary.kind_counts[task.task_kind] += 1;
      if (task.ready_to_start) {
        summary.ready_to_start_count += 1;
      }
      return summary;
    },
    {
      total: 0,
      status_counts: {
        draft: 0,
        ready: 0,
        in_progress: 0,
        blocked: 0,
        done: 0,
        cancelled: 0,
      },
      knowledge_counts: {
        empty: 0,
        working: 0,
        reusable: 0,
      },
      kind_counts: {
        standard: 0,
        context: 0,
        index: 0,
      },
      ready_to_start_count: 0,
    },
  );
}

function listTasks(filters: JsonMap): TaskListPayload {
  refreshTasksDerivedFields(state.tasks.map((item) => item.task_id));
  const projectReference = typeof filters.project === "string" ? filters.project.trim() : "";
  const versionReference = typeof filters.version === "string" ? filters.version.trim() : "";
  const status = typeof filters.status === "string" ? (filters.status.trim() as TaskStatus) : undefined;
  const kind = typeof filters.kind === "string" ? (filters.kind.trim() as TaskKind) : undefined;
  const taskCodePrefix =
    typeof filters.task_code_prefix === "string" && filters.task_code_prefix.trim()
      ? filters.task_code_prefix.trim()
      : "";
  const titlePrefix =
    typeof filters.title_prefix === "string" && filters.title_prefix.trim() ? filters.title_prefix.trim() : "";

  let nextTasks = [...state.tasks];
  if (projectReference) {
    const project = findProject(projectReference);
    nextTasks = nextTasks.filter((item) => item.project_id === project.project_id);
  }
  if (versionReference) {
    nextTasks = nextTasks.filter((item) => item.version_id === versionReference);
  }
  if (status) {
    nextTasks = nextTasks.filter((item) => item.status === status);
  }
  if (kind) {
    nextTasks = nextTasks.filter((item) => item.task_kind === kind);
  }
  if (taskCodePrefix) {
    nextTasks = nextTasks.filter((item) => (item.task_code ?? "").startsWith(taskCodePrefix));
  }
  if (titlePrefix) {
    nextTasks = nextTasks.filter((item) => item.title.startsWith(titlePrefix));
  }

  const sortBy =
    typeof filters.sort_by === "string" && filters.sort_by.trim()
      ? filters.sort_by.trim()
      : versionReference && nextTasks.some((item) => item.task_code)
        ? "task_code"
        : "created_at";
  const sortOrder =
    typeof filters.sort_order === "string" && filters.sort_order.trim()
      ? (filters.sort_order.trim() as "asc" | "desc")
      : sortBy === "task_code" || sortBy === "title"
        ? "asc"
        : "desc";

  nextTasks.sort((left, right) => compareTasks(left, right, sortBy, sortOrder));

  return {
    tasks: nextTasks,
    summary: buildTaskSummary(nextTasks),
    page: {
      limit: null,
      next_cursor: null,
      has_more: false,
      sort_by: sortBy,
      sort_order: sortOrder,
    },
  };
}

function taskActivitiesFor(taskId: string) {
  return sortByDateDesc(state.taskActivities.filter((item) => item.task_id === taskId));
}

function listNotes(taskId: string) {
  return taskActivitiesFor(taskId).filter((item) => item.kind === "note");
}

function listAttachments(taskId: string) {
  return sortByDateDesc(state.attachments.filter((item) => item.task_id === taskId));
}

function createProject(input: JsonMap) {
  const slug = requireString(input.slug, "slug");
  const name = requireString(input.name, "name");

  if (state.projects.some((item) => item.slug === slug)) {
    bridgeError("conflict", `Project slug already exists: ${slug}`, { slug });
  }

  const project: Project = {
    project_id: crypto.randomUUID(),
    slug,
    name,
    description: typeof input.description === "string" && input.description.trim() ? input.description.trim() : null,
    status: "active",
    default_version_id: null,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  };
  state.projects = [project, ...state.projects];
  return project;
}

function updateProject(input: JsonMap) {
  const projectReference = requireString(input.project, "project");
  const project = findProject(projectReference);

  if (typeof input.slug === "string" && input.slug.trim()) {
    const nextSlug = input.slug.trim();
    const conflict = state.projects.find((item) => item.slug === nextSlug && item.project_id !== project.project_id);
    if (conflict) {
      bridgeError("conflict", `Project slug already exists: ${nextSlug}`, { slug: nextSlug });
    }
    project.slug = nextSlug;
  }
  if (typeof input.name === "string" && input.name.trim()) {
    project.name = input.name.trim();
  }
  if ("description" in input) {
    project.description =
      typeof input.description === "string" && input.description.trim() ? input.description.trim() : null;
  }
  if (typeof input.status === "string") {
    project.status = input.status as Project["status"];
  }
  project.updated_at = new Date().toISOString();
  enqueuePreviewMutation(project, "update");
  return project;
}

function createVersion(input: JsonMap) {
  const project = findProject(requireString(input.project, "project"));
  const version: Version = {
    version_id: crypto.randomUUID(),
    project_id: project.project_id,
    name: requireString(input.name, "name"),
    description: typeof input.description === "string" && input.description.trim() ? input.description.trim() : null,
    status: (typeof input.status === "string" ? input.status : "planning") as VersionStatus,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  };
  state.versions = [version, ...state.versions];
  if (!project.default_version_id) {
    project.default_version_id = version.version_id;
    project.updated_at = new Date().toISOString();
  }
  enqueuePreviewMutation(version, "create");
  return version;
}

function updateVersion(input: JsonMap) {
  const version = findVersion(requireString(input.version, "version"));
  if (typeof input.name === "string" && input.name.trim()) {
    version.name = input.name.trim();
  }
  if ("description" in input) {
    version.description =
      typeof input.description === "string" && input.description.trim() ? input.description.trim() : null;
  }
  if (typeof input.status === "string") {
    version.status = input.status as VersionStatus;
  }
  version.updated_at = new Date().toISOString();
  enqueuePreviewMutation(version, "update");
  return version;
}

function resolveTaskVersion(projectId: string, versionReference: unknown) {
  if (typeof versionReference !== "string" || !versionReference.trim()) {
    return null;
  }
  const version = findVersion(versionReference.trim());
  if (version.project_id !== projectId) {
    bridgeError("conflict", "Version does not belong to the selected project", {
      project_id: projectId,
      version_id: version.version_id,
    });
  }
  return version.version_id;
}

function createTask(input: JsonMap) {
  const project = findProject(requireString(input.project, "project"));
  const now = new Date().toISOString();
  const task = createTaskRecord({
    task_id: crypto.randomUUID(),
    project_id: project.project_id,
    version_id: resolveTaskVersion(project.project_id, input.version),
    task_code: typeof input.task_code === "string" && input.task_code.trim() ? input.task_code.trim() : null,
    task_kind: typeof input.kind === "string" && input.kind.trim() ? (input.kind.trim() as TaskKind) : "standard",
    title: requireString(input.title, "title"),
    summary: typeof input.summary === "string" && input.summary.trim() ? input.summary.trim() : null,
    description: typeof input.description === "string" && input.description.trim() ? input.description.trim() : null,
    status: (typeof input.status === "string" ? input.status : "ready") as TaskStatus,
    priority: (typeof input.priority === "string" ? input.priority : "normal") as TaskPriority,
    created_by: typeof input.created_by === "string" && input.created_by.trim() ? input.created_by.trim() : "desktop",
    updated_by: typeof input.created_by === "string" && input.created_by.trim() ? input.created_by.trim() : "desktop",
    created_at: now,
    updated_at: now,
    closed_at: null,
  });
  state.tasks = [task, ...state.tasks];
  refreshTaskDerivedFields(task.task_id);
  enqueuePreviewMutation(task, "create");
  return task;
}

function createChildTask(input: JsonMap) {
  const parent = findTask(requireString(input.parent, "parent"));
  const task = createTask({
    ...input,
    project: parent.project_id,
    version: typeof input.version === "string" && input.version.trim() ? input.version.trim() : parent.version_id,
  });
  const relation = attachChild({
    child: task.task_id,
    parent: parent.task_id,
    updated_by:
      typeof input.created_by === "string" && input.created_by.trim() ? input.created_by.trim() : "desktop",
  });
  enqueuePreviewMutation(relation, "create");
  refreshTasksDerivedFields([parent.task_id, task.task_id]);
  return task;
}

function updateTask(input: JsonMap) {
  const task = findTask(requireString(input.task, "task"));
  if (typeof input.title === "string" && input.title.trim()) {
    task.title = input.title.trim();
  }
  if ("task_code" in input) {
    task.task_code = typeof input.task_code === "string" && input.task_code.trim() ? input.task_code.trim() : null;
  }
  if (typeof input.kind === "string" && input.kind.trim()) {
    task.task_kind = input.kind.trim() as TaskKind;
  }
  if ("summary" in input) {
    task.summary = typeof input.summary === "string" && input.summary.trim() ? input.summary.trim() : null;
  }
  if ("description" in input) {
    task.description =
      typeof input.description === "string" && input.description.trim() ? input.description.trim() : null;
  }
  if (typeof input.status === "string") {
    task.status = input.status as TaskStatus;
    task.closed_at = task.status === "done" || task.status === "cancelled" ? new Date().toISOString() : null;
  }
  if (typeof input.priority === "string") {
    task.priority = input.priority as TaskPriority;
  }
  if ("version" in input) {
    task.version_id = resolveTaskVersion(task.project_id, input.version);
  }
  if (typeof input.updated_by === "string" && input.updated_by.trim()) {
    task.updated_by = input.updated_by.trim();
  }
  task.updated_at = new Date().toISOString();
  refreshTaskDerivedFields(task.task_id);
  enqueuePreviewMutation(task, "update");
  return task;
}

function ensureSameProject(leftTaskId: string, rightTaskId: string) {
  const left = findTask(leftTaskId);
  const right = findTask(rightTaskId);
  if (left.project_id !== right.project_id) {
    bridgeError("conflict", "Tasks must belong to the same project", {
      left_task_id: left.task_id,
      right_task_id: right.task_id,
    });
  }
  return { left, right };
}

function hasParentCycle(parentId: string, childId: string): boolean {
  const visited = new Set<string>();
  const queue = [childId];
  while (queue.length > 0) {
    const current = queue.shift() ?? "";
    if (!current || visited.has(current)) {
      continue;
    }
    visited.add(current);
    if (current === parentId) {
      return true;
    }
    for (const relation of activeChildRelations(current)) {
      queue.push(relation.target_task_id);
    }
  }
  return false;
}

function attachChild(input: JsonMap): TaskRelation {
  const parentId = requireString(input.parent, "parent");
  const childId = requireString(input.child, "child");
  if (parentId === childId) {
    bridgeError("conflict", "Parent and child task must be different", { parent: parentId, child: childId });
  }
  const { left: parent, right: child } = ensureSameProject(parentId, childId);
  if (activeParentRelation(child.task_id)) {
    bridgeError("conflict", "Child task already has an active parent", { child: child.task_id });
  }
  if (
    state.taskRelations.some(
      (relation) =>
        relation.kind === "parent_child" &&
        relation.source_task_id === parent.task_id &&
        relation.target_task_id === child.task_id &&
        activeRelationStatus(relation.status),
    )
  ) {
    bridgeError("conflict", "Child task is already attached to this parent", {
      parent: parent.task_id,
      child: child.task_id,
    });
  }
  if (hasParentCycle(parent.task_id, child.task_id)) {
    bridgeError("conflict", "Attaching this child would create a parent cycle", {
      parent: parent.task_id,
      child: child.task_id,
    });
  }
  const now = new Date().toISOString();
  const actor =
    typeof input.updated_by === "string" && input.updated_by.trim() ? input.updated_by.trim() : "desktop";
  const relation: TaskRelation = {
    relation_id: crypto.randomUUID(),
    kind: "parent_child",
    source_task_id: parent.task_id,
    target_task_id: child.task_id,
    status: "active",
    created_by: actor,
    updated_by: actor,
    created_at: now,
    updated_at: now,
    resolved_at: null,
  };
  state.taskRelations = [relation, ...state.taskRelations];
  parent.updated_at = now;
  parent.updated_by = actor;
  child.updated_at = now;
  child.updated_by = actor;
  state.taskActivities = [
    {
      activity_id: crypto.randomUUID(),
      task_id: parent.task_id,
      kind: "system",
      content: `Attached child task ${child.title}.`,
      activity_search_summary: `system: attached child task ${child.title.toLowerCase()}`,
      created_by: actor,
      created_at: now,
      metadata_json: { relation_id: relation.relation_id, child_task_id: child.task_id },
    },
    {
      activity_id: crypto.randomUUID(),
      task_id: child.task_id,
      kind: "system",
      content: `Attached to parent task ${parent.title}.`,
      activity_search_summary: `system: attached to parent task ${parent.title.toLowerCase()}`,
      created_by: actor,
      created_at: now,
      metadata_json: { relation_id: relation.relation_id, parent_task_id: parent.task_id },
    },
    ...state.taskActivities,
  ];
  refreshTasksDerivedFields([parent.task_id, child.task_id]);
  enqueuePreviewMutation(parent, "update");
  enqueuePreviewMutation(child, "update");
  enqueuePreviewMutation(relation, "create");
  return relation;
}

function detachChild(input: JsonMap): TaskRelation {
  const parentId = requireString(input.parent, "parent");
  const childId = requireString(input.child, "child");
  const parent = findTask(parentId);
  const child = findTask(childId);
  const relation = state.taskRelations.find(
    (item) =>
      item.kind === "parent_child" &&
      item.source_task_id === parent.task_id &&
      item.target_task_id === child.task_id &&
      activeRelationStatus(item.status),
  );
  if (!relation) {
    bridgeError("not_found", "Active parent-child relation not found", { parent: parentId, child: childId });
  }
  const now = new Date().toISOString();
  const actor =
    typeof input.updated_by === "string" && input.updated_by.trim() ? input.updated_by.trim() : "desktop";
  relation.status = "resolved";
  relation.updated_at = now;
  relation.updated_by = actor;
  relation.resolved_at = now;
  parent.updated_at = now;
  parent.updated_by = actor;
  child.updated_at = now;
  child.updated_by = actor;
  state.taskActivities = [
    {
      activity_id: crypto.randomUUID(),
      task_id: parent.task_id,
      kind: "system",
      content: `Detached child task ${child.title}.`,
      activity_search_summary: `system: detached child task ${child.title.toLowerCase()}`,
      created_by: actor,
      created_at: now,
      metadata_json: { relation_id: relation.relation_id, child_task_id: child.task_id, status: relation.status },
    },
    {
      activity_id: crypto.randomUUID(),
      task_id: child.task_id,
      kind: "system",
      content: `Detached from parent task ${parent.title}.`,
      activity_search_summary: `system: detached from parent task ${parent.title.toLowerCase()}`,
      created_by: actor,
      created_at: now,
      metadata_json: { relation_id: relation.relation_id, parent_task_id: parent.task_id, status: relation.status },
    },
    ...state.taskActivities,
  ];
  refreshTasksDerivedFields([parent.task_id, child.task_id]);
  enqueuePreviewMutation(parent, "update");
  enqueuePreviewMutation(child, "update");
  enqueuePreviewMutation(relation, "update");
  return relation;
}

function addBlocker(input: JsonMap): TaskRelation {
  const blockerId = requireString(input.blocker, "blocker");
  const blockedId = requireString(input.task ?? input.blocked, "task");
  if (blockerId === blockedId) {
    bridgeError("conflict", "Blocker and blocked task must be different", {
      blocker: blockerId,
      blocked: blockedId,
    });
  }
  const { left: blocker, right: blocked } = ensureSameProject(blockerId, blockedId);
  if (
    state.taskRelations.some(
      (relation) =>
        relation.kind === "blocks" &&
        relation.source_task_id === blocker.task_id &&
        relation.target_task_id === blocked.task_id &&
        activeRelationStatus(relation.status),
    )
  ) {
    bridgeError("conflict", "This blocker relation already exists", {
      blocker: blocker.task_id,
      blocked: blocked.task_id,
    });
  }
  const now = new Date().toISOString();
  const actor =
    typeof input.updated_by === "string" && input.updated_by.trim() ? input.updated_by.trim() : "desktop";
  const relation: TaskRelation = {
    relation_id: crypto.randomUUID(),
    kind: "blocks",
    source_task_id: blocker.task_id,
    target_task_id: blocked.task_id,
    status: "active",
    created_by: actor,
    updated_by: actor,
    created_at: now,
    updated_at: now,
    resolved_at: null,
  };
  state.taskRelations = [relation, ...state.taskRelations];
  blocker.updated_at = now;
  blocker.updated_by = actor;
  blocked.updated_at = now;
  blocked.updated_by = actor;
  if (!matchesClosedTask(blocked.status)) {
    const previousStatus = blocked.status;
    blocked.status = "blocked";
    blocked.closed_at = null;
    if (previousStatus !== blocked.status) {
      state.taskActivities = [
        {
          activity_id: crypto.randomUUID(),
          task_id: blocked.task_id,
          kind: "status_change",
          content: `Status changed from ${previousStatus} to ${blocked.status}.`,
          activity_search_summary: `status_change: status changed from ${previousStatus} to ${blocked.status}`,
          created_by: actor,
          created_at: now,
          metadata_json: { from_status: previousStatus, to_status: blocked.status },
        },
        ...state.taskActivities,
      ];
    }
  }
  state.taskActivities = [
    {
      activity_id: crypto.randomUUID(),
      task_id: blocker.task_id,
      kind: "system",
      content: `Task ${blocked.title} is now blocked by this task.`,
      activity_search_summary: `system: task ${blocked.title.toLowerCase()} is now blocked by this task`,
      created_by: actor,
      created_at: now,
      metadata_json: { relation_id: relation.relation_id, blocked_task_id: blocked.task_id },
    },
    {
      activity_id: crypto.randomUUID(),
      task_id: blocked.task_id,
      kind: "system",
      content: `Blocked by task ${blocker.title}.`,
      activity_search_summary: `system: blocked by task ${blocker.title.toLowerCase()}`,
      created_by: actor,
      created_at: now,
      metadata_json: { relation_id: relation.relation_id, blocker_task_id: blocker.task_id },
    },
    ...state.taskActivities,
  ];
  refreshTasksDerivedFields([blocker.task_id, blocked.task_id]);
  enqueuePreviewMutation(blocker, "update");
  enqueuePreviewMutation(blocked, "update");
  enqueuePreviewMutation(relation, "create");
  return relation;
}

function resolveBlocker(input: JsonMap): TaskRelation {
  const taskId = requireString(input.task, "task");
  const blocked = findTask(taskId);
  const relationId = typeof input.relation_id === "string" ? input.relation_id.trim() : "";
  const blockerRef = typeof input.blocker === "string" ? input.blocker.trim() : "";
  let relation =
    relationId
      ? state.taskRelations.find((item) => item.relation_id === relationId)
      : null;
  if (!relation && blockerRef) {
    relation =
      state.taskRelations.find(
        (item) =>
          item.kind === "blocks" &&
          item.source_task_id === blockerRef &&
          item.target_task_id === blocked.task_id &&
          activeRelationStatus(item.status),
      ) ?? null;
  }
  if (!relation || relation.kind !== "blocks" || relation.target_task_id !== blocked.task_id) {
    bridgeError("not_found", "Active blocker relation not found", {
      task: blocked.task_id,
      blocker: input.blocker,
      relation_id: input.relation_id,
    });
  }
  const blocker = findTask(relation.source_task_id);
  const now = new Date().toISOString();
  const actor =
    typeof input.updated_by === "string" && input.updated_by.trim() ? input.updated_by.trim() : "desktop";
  relation.status = "resolved";
  relation.updated_at = now;
  relation.updated_by = actor;
  relation.resolved_at = now;
  blocker.updated_at = now;
  blocker.updated_by = actor;
  blocked.updated_at = now;
  blocked.updated_by = actor;
  refreshTasksDerivedFields([blocker.task_id, blocked.task_id]);
  if (blocked.status === "blocked" && blocked.open_blocker_count === 0) {
    const previousStatus = blocked.status;
    blocked.status = "ready";
    blocked.closed_at = null;
    state.taskActivities = [
      {
        activity_id: crypto.randomUUID(),
        task_id: blocked.task_id,
        kind: "status_change",
        content: `Status changed from ${previousStatus} to ${blocked.status}.`,
        activity_search_summary: `status_change: status changed from ${previousStatus} to ${blocked.status}`,
        created_by: actor,
        created_at: now,
        metadata_json: { from_status: previousStatus, to_status: blocked.status },
      },
      ...state.taskActivities,
    ];
  }
  state.taskActivities = [
    {
      activity_id: crypto.randomUUID(),
      task_id: blocker.task_id,
      kind: "system",
      content: `Resolved blocker for task ${blocked.title}.`,
      activity_search_summary: `system: resolved blocker for task ${blocked.title.toLowerCase()}`,
      created_by: actor,
      created_at: now,
      metadata_json: { relation_id: relation.relation_id, blocked_task_id: blocked.task_id, status: relation.status },
    },
    {
      activity_id: crypto.randomUUID(),
      task_id: blocked.task_id,
      kind: "system",
      content: `Unblocked from task ${blocker.title}.`,
      activity_search_summary: `system: unblocked from task ${blocker.title.toLowerCase()}`,
      created_by: actor,
      created_at: now,
      metadata_json: { relation_id: relation.relation_id, blocker_task_id: blocker.task_id, status: relation.status },
    },
    ...state.taskActivities,
  ];
  refreshTasksDerivedFields([blocker.task_id, blocked.task_id]);
  enqueuePreviewMutation(blocker, "update");
  enqueuePreviewMutation(blocked, "update");
  enqueuePreviewMutation(relation, "update");
  return relation;
}

function createNote(input: JsonMap) {
  const task = findTask(requireString(input.task, "task"));
  const content = requireString(input.content, "content");
  const noteKind =
    typeof input.note_kind === "string" && input.note_kind.trim()
      ? (input.note_kind.trim() as NoteKind)
      : "finding";
  const note: TaskActivity = {
    activity_id: crypto.randomUUID(),
    task_id: task.task_id,
    kind: "note",
    content,
    activity_search_summary: `note: ${content.toLowerCase()}`,
    created_by: typeof input.created_by === "string" && input.created_by.trim() ? input.created_by.trim() : "desktop",
    created_at: new Date().toISOString(),
    metadata_json: { note_kind: noteKind },
  };
  state.taskActivities = [note, ...state.taskActivities];
  task.updated_at = note.created_at;
  enqueuePreviewMutation(note, "create");
  enqueuePreviewMutation(task, "update");
  refreshTaskDerivedFields(task.task_id);
  return note;
}

function createAttachment(input: JsonMap) {
  const task = findTask(requireString(input.task, "task"));
  const summary =
    typeof input.summary === "string" && input.summary.trim() ? input.summary.trim() : "Preview attachment";
  const originalPath =
    typeof input.path === "string" && input.path.trim() ? input.path.trim() : "D:/preview/files/attachment.txt";
  const kind = (typeof input.kind === "string" ? input.kind : "artifact") as AttachmentKind;
  const attachment: Attachment = {
    attachment_id: crypto.randomUUID(),
    task_id: task.task_id,
    kind,
    mime: "text/plain",
    original_filename: originalPath.split(/[\\/]/).pop() || "attachment.txt",
    original_path: originalPath,
    storage_path: `attachments/${task.task_id}/${crypto.randomUUID()}.txt`,
    sha256: crypto.randomUUID().replace(/-/g, ""),
    size_bytes: 2048,
    summary,
    created_by: typeof input.created_by === "string" && input.created_by.trim() ? input.created_by.trim() : "desktop",
    created_at: new Date().toISOString(),
  };
  const activity: TaskActivity = {
    activity_id: crypto.randomUUID(),
    task_id: task.task_id,
    kind: "attachment_ref",
    content: summary,
    activity_search_summary: `attachment_ref: ${summary.toLowerCase()}`,
    created_by: attachment.created_by,
    created_at: attachment.created_at,
    metadata_json: {
      attachment_id: attachment.attachment_id,
      storage_path: attachment.storage_path,
    },
  };
  state.attachments = [attachment, ...state.attachments];
  state.taskActivities = [activity, ...state.taskActivities];
  task.updated_at = attachment.created_at;
  refreshTaskDerivedFields(task.task_id);
  enqueuePreviewMutation(attachment, "create");
  enqueuePreviewMutation(task, "update");
  return attachment;
}

function listApprovals(status?: ApprovalStatus, projectReference?: string) {
  let nextApprovals = status ? state.approvals.filter((item) => item.status === status) : state.approvals;
  if (projectReference) {
    const project =
      state.projects.find((item) => item.project_id === projectReference || item.slug === projectReference) ?? null;
    nextApprovals = nextApprovals.filter(
      (item) =>
        item.project_ref === projectReference ||
        (project ? item.project_ref === project.project_id || item.project_ref === project.slug : false),
    );
  }
  return sortByDateDesc(nextApprovals);
}

function reviewApproval(input: JsonMap, nextStatus: Extract<ApprovalStatus, "approved" | "denied">) {
  const approval = findApproval(requireString(input.request_id, "request_id"));
  approval.status = nextStatus;
  approval.reviewed_at = new Date().toISOString();
  approval.reviewed_by =
    typeof input.reviewed_by === "string" && input.reviewed_by.trim() ? input.reviewed_by.trim() : "desktop";
  approval.review_note =
    typeof input.review_note === "string" && input.review_note.trim() ? input.review_note.trim() : null;
  if (nextStatus === "approved" && !approval.result_json) {
    approval.result_json = {
      resource_ref: approval.resource_ref,
      reviewed_at: approval.reviewed_at,
      status: "approved",
    };
    approval.error_json = null;
  }
  if (nextStatus === "denied") {
    approval.result_json = null;
    approval.error_json = null;
  }
  return approval;
}

function runSearch(input: JsonMap) {
  const query =
    typeof input.query === "string" && input.query.trim()
      ? input.query.trim().toLowerCase()
      : typeof input.text === "string" && input.text.trim()
        ? input.text.trim().toLowerCase()
        : null;
  const limit = typeof input.limit === "number" ? Math.max(1, Math.min(20, input.limit)) : 8;
  const filteredTasks = listTasks(input).tasks;
  const taskIds = new Set(filteredTasks.map((item) => item.task_id));

  const tasks = filteredTasks
    .filter((item) =>
      !query
        ? true
        : [
            item.task_code ?? "",
            item.title,
            item.summary ?? "",
            item.description ?? "",
            item.task_context_digest,
            item.latest_note_summary ?? "",
          ]
            .join(" ")
            .toLowerCase()
            .includes(query),
    )
    .slice(0, limit)
    .map((item) => ({
      task_code: item.task_code,
      task_kind: item.task_kind,
      knowledge_status: item.knowledge_status,
      matched_fields: query
        ? [
            item.task_code?.toLowerCase().includes(query) ? "task_code" : null,
            item.title.toLowerCase().includes(query) ? "title" : null,
            (item.latest_note_summary ?? "").toLowerCase().includes(query)
              ? "latest_note_summary"
              : null,
            item.task_search_summary.toLowerCase().includes(query) ? "task_search_summary" : null,
            item.task_context_digest.toLowerCase().includes(query) ? "task_context_digest" : null,
          ].filter((value): value is string => value !== null)
        : [],
      priority: item.priority,
      retrieval_source: query ? ("lexical" as const) : ("structured_filter" as const),
      score: query ? 1 : null,
      status: item.status,
      summary: item.latest_note_summary ?? item.task_search_summary,
      task_id: item.task_id,
      title: item.title,
    }));

  const activities = !query
    ? []
    : state.taskActivities
    .filter(
      (item) =>
        taskIds.has(item.task_id) &&
        `${item.activity_search_summary} ${item.content}`.toLowerCase().includes(query),
    )
    .slice(0, limit)
    .map((item) => ({
      activity_id: item.activity_id,
      kind: item.kind,
      score: query ? 1 : null,
      summary: item.activity_search_summary,
      task_id: item.task_id,
    }));

  const results: SearchResponse = {
    query,
    tasks,
    activities,
    meta: {
      indexed_fields: {
        tasks: [
          "title",
          "task_code",
          "task_kind",
          "task_search_summary",
          "task_context_digest",
          "latest_note_summary",
        ],
        activities: ["activity_search_summary"],
      },
      task_sort: query
        ? "sqlite fts5 bm25 with structured filters and recency tiebreaks"
        : "structured task filter order",
      activity_sort: "sqlite fts5 bm25 with structured task filters applied",
      limit_applies_per_bucket: true,
      task_limit_applied: limit,
      activity_limit_applied: limit,
      default_limit: 10,
      max_limit: 50,
      retrieval_mode: query ? "lexical_only" : "structured_only",
      vector_backend: null,
      vector_status: "disabled",
      pending_index_jobs: 0,
    },
  };
  return results;
}

function runtimeStatus(): RuntimeStatus {
  return {
    data_dir: "D:/preview/agenta/data",
    database_path: "D:/preview/agenta/data/agenta.sqlite3",
    attachments_dir: "D:/preview/agenta/data/attachments",
    loaded_config_path: "D:/preview/agenta/agenta.local.yaml",
    mcp_bind: "127.0.0.1:8787",
    mcp_path: "/mcp",
    project_count: state.projects.length,
    task_count: state.tasks.length,
    pending_approval_count: state.approvals.filter((item) => item.status === "pending").length,
  };
}

export const mockDesktopBridge = {
  status() {
    return Promise.resolve(envelope("desktop_status", runtimeStatus(), "Loaded preview runtime status."));
  },
  syncStatus() {
    return Promise.resolve(
      envelope("desktop_sync_status", createPreviewSyncStatus(), "Loaded preview sync status."),
    );
  },
  syncOutboxList(limit?: number) {
    const result =
      typeof limit === "number" && limit > 0 ? syncOutbox.slice(0, limit) : [...syncOutbox];
    return Promise.resolve(
      envelope("desktop_sync_outbox_list", result, "Loaded preview sync outbox."),
    );
  },
  syncBackfill(limit?: number) {
    return Promise.resolve(
      envelope("desktop_sync_backfill", runPreviewBackfill(limit), "Completed preview sync backfill."),
    );
  },
  syncPush(limit?: number) {
    return Promise.resolve(
      envelope("desktop_sync_push", runPreviewPush(limit), "Completed preview sync push."),
    );
  },
  syncPull(limit?: number) {
    return Promise.resolve(
      envelope("desktop_sync_pull", runPreviewPull(limit), "Completed preview sync pull."),
    );
  },
  mcpStatus() {
    return Promise.resolve(envelope("desktop_mcp_status", mcpRuntime, "Loaded preview MCP runtime status."));
  },
  mcpStart(input: McpLaunchOverrides = {}) {
    startPreviewMcp(input);
    return Promise.resolve(envelope("desktop_mcp_start", mcpRuntime, "Started preview MCP host."));
  },
  mcpStop() {
    stopPreviewMcp();
    return Promise.resolve(envelope("desktop_mcp_stop", mcpRuntime, "Stopped preview MCP host."));
  },
  mcpLogsSnapshot(limit?: number) {
    const entries =
      typeof limit === "number" && limit > 0 ? mcpLogs.slice(Math.max(0, mcpLogs.length - limit)) : mcpLogs;
    const snapshot: McpLogSnapshot = {
      session_id: mcpRuntime.session_id,
      entries,
    };
    return Promise.resolve(
      envelope("desktop_mcp_logs_snapshot", snapshot, "Loaded preview MCP log snapshot."),
    );
  },
  project(input: JsonMap = {}) {
    const action = typeof input.action === "string" ? input.action : "list";
    switch (action) {
      case "create":
        return Promise.resolve(envelope("desktop_project", createProject(input), "Created preview project."));
      case "update":
        return Promise.resolve(envelope("desktop_project", updateProject(input), "Updated preview project."));
      case "list":
      default:
        return Promise.resolve(envelope("desktop_project", listProjects(), "Loaded preview projects."));
    }
  },
  version(input: JsonMap = {}) {
    const action = typeof input.action === "string" ? input.action : "list";
    switch (action) {
      case "get":
        return Promise.resolve(
          envelope("desktop_version", findVersion(requireString(input.version, "version")), "Loaded preview version."),
        );
      case "create":
        return Promise.resolve(envelope("desktop_version", createVersion(input), "Created preview version."));
      case "update":
        return Promise.resolve(envelope("desktop_version", updateVersion(input), "Updated preview version."));
      case "list":
      default:
        return Promise.resolve(
          envelope("desktop_version", listVersions(input.project), "Loaded preview release lanes."),
        );
    }
  },
  task(input: JsonMap = {}) {
    const action = typeof input.action === "string" ? input.action : "list";
    switch (action) {
      case "get":
        return Promise.resolve(
          envelope("desktop_task", findTask(requireString(input.task, "task")), "Loaded preview task."),
        );
      case "get_context":
        return Promise.resolve(
          envelope(
            "desktop_task",
            getTaskContext(requireString(input.task, "task")),
            "Loaded preview task context.",
          ),
        );
      case "create":
        return Promise.resolve(envelope("desktop_task", createTask(input), "Created preview task."));
      case "create_child":
        return Promise.resolve(envelope("desktop_task", createChildTask(input), "Created preview child task."));
      case "update":
        return Promise.resolve(envelope("desktop_task", updateTask(input), "Updated preview task."));
      case "attach_child":
        return Promise.resolve(envelope("desktop_task", attachChild(input), "Attached preview child task."));
      case "detach_child":
        return Promise.resolve(envelope("desktop_task", detachChild(input), "Detached preview child task."));
      case "add_blocker":
        return Promise.resolve(envelope("desktop_task", addBlocker(input), "Added preview blocker."));
      case "resolve_blocker":
        return Promise.resolve(
          envelope("desktop_task", resolveBlocker(input), "Resolved preview blocker."),
        );
      case "activity_list":
        return Promise.resolve(
          envelope(
            "desktop_task",
            taskActivitiesFor(requireString(input.task, "task")),
            "Loaded preview activity timeline.",
          ),
        );
      case "list":
      default:
        return Promise.resolve(envelope("desktop_task", listTasks(input), "Loaded preview tasks."));
    }
  },
  note(input: JsonMap = {}) {
    const action = typeof input.action === "string" ? input.action : "list";
    switch (action) {
      case "create":
        return Promise.resolve(envelope("desktop_note", createNote(input), "Added preview note."));
      case "list":
      default:
        return Promise.resolve(
          envelope("desktop_note", listNotes(requireString(input.task, "task")), "Loaded preview notes."),
        );
    }
  },
  attachment(input: JsonMap = {}) {
    const action = typeof input.action === "string" ? input.action : "list";
    switch (action) {
      case "create":
        return Promise.resolve(
          envelope("desktop_attachment", createAttachment(input), "Added preview attachment."),
        );
      case "list":
      default:
        return Promise.resolve(
          envelope(
            "desktop_attachment",
            listAttachments(requireString(input.task, "task")),
            "Loaded preview attachments.",
          ),
        );
    }
  },
  approval(input: JsonMap = {}) {
    const action = typeof input.action === "string" ? input.action : "list";
    switch (action) {
      case "get":
        return Promise.resolve(
          envelope(
            "desktop_approval",
            findApproval(requireString(input.request_id, "request_id")),
            "Loaded preview approval request.",
          ),
        );
      case "approve":
        return Promise.resolve(
          envelope("desktop_approval", reviewApproval(input, "approved"), "Approved preview request."),
        );
      case "deny":
        return Promise.resolve(
          envelope("desktop_approval", reviewApproval(input, "denied"), "Denied preview request."),
        );
      case "list":
      default: {
        const status =
          typeof input.status === "string" && input.status.trim() ? (input.status.trim() as ApprovalStatus) : undefined;
        const project =
          typeof input.project === "string" && input.project.trim() ? input.project.trim() : undefined;
        return Promise.resolve(
          envelope("desktop_approval", listApprovals(status, project), "Loaded preview approval queue."),
        );
      }
    }
  },
  search(input: JsonMap = {}) {
    return Promise.resolve(envelope("desktop_search", runSearch(input), "Loaded preview search results."));
  },
  searchBackfill(options: { limit?: number; batchSize?: number } = {}) {
    return Promise.resolve(
      envelope(
        "desktop_search",
        runPreviewSearchBackfill(options),
        "Completed preview search backfill.",
      ),
    );
  },
  openPath(_path?: string) {
    return Promise.resolve();
  },
  revealAttachment(_path?: string) {
    return Promise.resolve();
  },
  reset() {
    state = createSeedState();
    mcpRuntime = createPreviewMcpRuntime();
    mcpLogs = [];
    syncOutbox = [];
    syncPullCheckpoint = null;
    syncPushAckCheckpoint = null;
    previewRemoteMutationCursor = 12;
  },
};
