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
  Project,
  RuntimeStatus,
  SearchResponse,
  SuccessEnvelope,
  Task,
  TaskActivity,
  TaskPriority,
  TaskStatus,
  Version,
  VersionStatus,
} from "./types";

type JsonMap = Record<string, unknown>;

interface MockState {
  approvals: ApprovalRequest[];
  attachments: Attachment[];
  projects: Project[];
  tasks: Task[];
  taskActivities: TaskActivity[];
  versions: Version[];
}

const PREVIEW_WARNING = "Running in browser preview mode with seeded local data.";

const previewNow = Date.now();
const PREVIEW_MCP_LOG_FILE = "D:/preview/agenta/data/logs/mcp.jsonl";

let mcpRuntime = createPreviewMcpRuntime();
let mcpLogs: McpLogEntry[] = [];

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
    versions,
  };
}

let state = createSeedState();

function createTaskRecord(task: Omit<Task, "task_context_digest" | "task_search_summary">): Task {
  return {
    ...task,
    task_context_digest: buildTaskContextDigest(task),
    task_search_summary: buildTaskSearchSummary(task.title, task.summary, task.description),
  };
}

function buildTaskSearchSummary(title: string, summary: string | null, description: string | null) {
  return [title, summary, description].filter(Boolean).join(" | ");
}

function buildTaskContextDigest(task: {
  description: string | null;
  priority: TaskPriority;
  status: TaskStatus;
  summary: string | null;
  title: string;
}) {
  return `status=${task.status} priority=${task.priority} title=${task.title} summary=${task.summary ?? ""} description=${task.description ?? ""}`.trim();
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

function listTasks(filters: JsonMap) {
  const projectReference = typeof filters.project === "string" ? filters.project.trim() : "";
  const versionReference = typeof filters.version === "string" ? filters.version.trim() : "";
  const status = typeof filters.status === "string" ? (filters.status.trim() as TaskStatus) : undefined;

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
  return sortByDateDesc(nextTasks);
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
  return task;
}

function updateTask(input: JsonMap) {
  const task = findTask(requireString(input.task, "task"));
  if (typeof input.title === "string" && input.title.trim()) {
    task.title = input.title.trim();
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
  task.task_search_summary = buildTaskSearchSummary(task.title, task.summary, task.description);
  task.task_context_digest = buildTaskContextDigest(task);
  return task;
}

function createNote(input: JsonMap) {
  const task = findTask(requireString(input.task, "task"));
  const content = requireString(input.content, "content");
  const note: TaskActivity = {
    activity_id: crypto.randomUUID(),
    task_id: task.task_id,
    kind: "note",
    content,
    activity_search_summary: `note: ${content.toLowerCase()}`,
    created_by: typeof input.created_by === "string" && input.created_by.trim() ? input.created_by.trim() : "desktop",
    created_at: new Date().toISOString(),
    metadata_json: {},
  };
  state.taskActivities = [note, ...state.taskActivities];
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
  return attachment;
}

function listApprovals(status?: ApprovalStatus) {
  const nextApprovals = status ? state.approvals.filter((item) => item.status === status) : state.approvals;
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
  const query = requireString(input.query, "query").toLowerCase();
  const limit = typeof input.limit === "number" ? Math.max(1, Math.min(20, input.limit)) : 8;

  const tasks = state.tasks
    .filter((item) =>
      [item.title, item.summary ?? "", item.description ?? "", item.task_context_digest]
        .join(" ")
        .toLowerCase()
        .includes(query),
    )
    .slice(0, limit)
    .map((item) => ({
      priority: item.priority,
      status: item.status,
      summary: item.task_search_summary,
      task_id: item.task_id,
      title: item.title,
    }));

  const activities = state.taskActivities
    .filter((item) => `${item.activity_search_summary} ${item.content}`.toLowerCase().includes(query))
    .slice(0, limit)
    .map((item) => ({
      activity_id: item.activity_id,
      kind: item.kind,
      summary: item.activity_search_summary,
      task_id: item.task_id,
    }));

  const results: SearchResponse = {
    query,
    tasks,
    activities,
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
      case "create":
        return Promise.resolve(envelope("desktop_task", createTask(input), "Created preview task."));
      case "update":
        return Promise.resolve(envelope("desktop_task", updateTask(input), "Updated preview task."));
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
        return Promise.resolve(
          envelope("desktop_approval", listApprovals(status), "Loaded preview approval queue."),
        );
      }
    }
  },
  search(input: JsonMap = {}) {
    return Promise.resolve(envelope("desktop_search", runSearch(input), "Loaded preview search results."));
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
  },
};
