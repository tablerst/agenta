import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";

import type {
  AppBridgeError,
  ApprovalRequest,
  Attachment,
  McpLaunchOverrides,
  McpLogEntry,
  McpLogSnapshot,
  McpRuntimeStatus,
  Project,
  RuntimeStatus,
  SearchBackfillSummary,
  SearchResponse,
  SuccessEnvelope,
  SyncBackfillSummary,
  SyncOutboxListItem,
  SyncPullSummary,
  SyncPushSummary,
  SyncStatusSummary,
  Task,
  TaskActivity,
  TaskContextPayload,
  TaskListPayload,
  TaskRelation,
  Version,
} from "./types";
import { mockDesktopBridge } from "./mockDesktop";

export type BridgeMode = "desktop" | "preview";

function hasTauriRuntime() {
  return typeof window !== "undefined" && isTauri();
}

export function resolveBridgeMode(): BridgeMode {
  return hasTauriRuntime() ? "desktop" : "preview";
}

export class DesktopBridgeError extends Error implements AppBridgeError {
  code: string;
  details: unknown;

  constructor(payload: AppBridgeError) {
    super(payload.message);
    this.name = "DesktopBridgeError";
    this.code = payload.code;
    this.details = payload.details;
  }
}

type SerializedOffsetDateTime = readonly [
  year: number,
  ordinal: number,
  hour: number,
  minute: number,
  second: number,
  nanosecond: number,
  offsetHours: number,
  offsetMinutes: number,
  offsetSeconds: number,
];

function normalizeSuccessEnvelope<TResult>(
  envelope: SuccessEnvelope<TResult>,
): SuccessEnvelope<TResult> {
  return {
    ...envelope,
    result: normalizeBridgePayload(envelope.result) as TResult,
  };
}

function normalizeBridgePayload<T>(value: T): T {
  if (isSerializedOffsetDateTime(value)) {
    return offsetDateTimeTupleToIsoString(value) as T;
  }

  if (Array.isArray(value)) {
    return value.map((item) => normalizeBridgePayload(item)) as T;
  }

  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, normalizeBridgePayload(item)]),
    ) as T;
  }

  return value;
}

function isSerializedOffsetDateTime(value: unknown): value is SerializedOffsetDateTime {
  return (
    Array.isArray(value) &&
    value.length === 9 &&
    isIntegerInRange(value[0], 1, 9999) &&
    isIntegerInRange(value[1], 1, 366) &&
    isIntegerInRange(value[2], 0, 23) &&
    isIntegerInRange(value[3], 0, 59) &&
    isIntegerInRange(value[4], 0, 60) &&
    isIntegerInRange(value[5], 0, 999_999_999) &&
    isIntegerInRange(value[6], -23, 23) &&
    isIntegerInRange(value[7], -59, 59) &&
    isIntegerInRange(value[8], -59, 59)
  );
}

function isIntegerInRange(value: unknown, min: number, max: number): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= min && value <= max;
}

function offsetDateTimeTupleToIsoString(value: SerializedOffsetDateTime): string {
  const [year, ordinal, hour, minute, second, nanosecond, offsetHours, offsetMinutes, offsetSeconds] =
    value;
  const offsetInSeconds = offsetHours * 60 * 60 + offsetMinutes * 60 + offsetSeconds;
  const milliseconds = Math.floor(nanosecond / 1_000_000);
  const utcTimestamp =
    Date.UTC(year, 0, ordinal, hour, minute, second, milliseconds) - offsetInSeconds * 1_000;
  return new Date(utcTimestamp).toISOString();
}

async function callDesktop<TResult>(
  command: string,
  input?: Record<string, unknown>,
): Promise<SuccessEnvelope<TResult>> {
  try {
    const envelope = await invoke<SuccessEnvelope<TResult>>(command, input ? { input } : undefined);
    return normalizeSuccessEnvelope(envelope);
  } catch (error) {
    throw normalizeError(error);
  }
}

async function callPreview<TResult>(
  loader: () => Promise<SuccessEnvelope<TResult>>,
): Promise<SuccessEnvelope<TResult>> {
  try {
    return normalizeSuccessEnvelope(await loader());
  } catch (error) {
    throw normalizeError(error);
  }
}

function normalizeError(error: unknown): DesktopBridgeError {
  if (error instanceof Error) {
    return new DesktopBridgeError({
      code: "desktop_bridge_error",
      message: error.message,
      details: error,
    });
  }

  if (typeof error === "string") {
    try {
      const parsed = JSON.parse(error) as { error?: AppBridgeError };
      if (parsed && parsed.error) {
        return new DesktopBridgeError(parsed.error);
      }
    } catch {
      return new DesktopBridgeError({
        code: "desktop_bridge_error",
        message: error,
        details: null,
      });
    }
  }

  if (typeof error === "object" && error && "error" in error) {
    const payload = (error as { error: AppBridgeError }).error;
    return new DesktopBridgeError(payload);
  }

  if (typeof error === "object" && error && "message" in error) {
    const payload = error as { message?: unknown; code?: unknown; details?: unknown };
    return new DesktopBridgeError({
      code: typeof payload.code === "string" ? payload.code : "desktop_bridge_error",
      message:
        typeof payload.message === "string" ? payload.message : "Unknown desktop bridge error",
      details: "details" in payload ? payload.details : error,
    });
  }

  return new DesktopBridgeError({
    code: "desktop_bridge_error",
    message: "Unknown desktop bridge error",
    details: error,
  });
}

type RawTaskDetail = {
  task: Task;
  note_count: number;
  attachment_count: number;
  latest_activity_at: string;
  parent_task_id: string | null;
  child_count: number;
  open_blocker_count: number;
  blocking_count: number;
  ready_to_start: boolean;
};

type RawTaskContextPayload = Omit<TaskContextPayload, "task"> & {
  task: Task | RawTaskDetail;
};

type RawTaskListPayload = Omit<TaskListPayload, "tasks"> & {
  tasks: Array<Task | RawTaskDetail>;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function isTaskDetail(value: unknown): value is RawTaskDetail {
  return isRecord(value) && isRecord(value.task) && typeof value.task.task_id === "string";
}

function flattenTaskDetail(value: RawTaskDetail): Task {
  return {
    ...value.task,
    attachment_count: value.attachment_count,
    blocking_count: value.blocking_count,
    child_count: value.child_count,
    latest_activity_at: value.latest_activity_at,
    note_count: value.note_count,
    open_blocker_count: value.open_blocker_count,
    parent_task_id: value.parent_task_id,
    ready_to_start: value.ready_to_start,
  };
}

function normalizeTaskResult(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => (isTaskDetail(item) ? flattenTaskDetail(item) : item));
  }

  if (isTaskDetail(value)) {
    return flattenTaskDetail(value);
  }

  if (isRecord(value) && "notes" in value && "attachments" in value && isTaskDetail(value.task)) {
    const context = value as RawTaskContextPayload;
    return {
      ...context,
      task: flattenTaskDetail(context.task as RawTaskDetail),
    } satisfies TaskContextPayload;
  }

  if (isRecord(value) && Array.isArray(value.tasks) && "summary" in value && "page" in value) {
    const payload = value as RawTaskListPayload;
    return {
      ...payload,
      tasks: payload.tasks.map((item) => (isTaskDetail(item) ? flattenTaskDetail(item) : item)),
    } satisfies TaskListPayload;
  }

  return value;
}

function normalizeTaskEnvelope<TResult>(
  envelope: SuccessEnvelope<TResult>,
): SuccessEnvelope<TResult> {
  return {
    ...envelope,
    result: normalizeTaskResult(envelope.result) as TResult,
  };
}

async function subscribeDesktopEvent<TPayload>(
  event: string,
  listener: (payload: TPayload) => void,
): Promise<UnlistenFn> {
  if (resolveBridgeMode() !== "desktop") {
    return async () => undefined;
  }

  return listen<TPayload>(event, (eventPayload) => {
    listener(normalizeBridgePayload(eventPayload.payload) as TPayload);
  });
}

export const desktopBridge = {
  get mode() {
    return resolveBridgeMode();
  },
  status() {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<RuntimeStatus>("desktop_status")
      : callPreview(() => mockDesktopBridge.status());
  },
  mcpStatus() {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<McpRuntimeStatus>("desktop_mcp_status")
      : callPreview(() => mockDesktopBridge.mcpStatus());
  },
  mcpStart(input: McpLaunchOverrides) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<McpRuntimeStatus>("desktop_mcp_start", input as Record<string, unknown>)
      : callPreview(() => mockDesktopBridge.mcpStart(input));
  },
  mcpStop() {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<McpRuntimeStatus>("desktop_mcp_stop")
      : callPreview(() => mockDesktopBridge.mcpStop());
  },
  mcpLogsSnapshot(limit?: number) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<McpLogSnapshot>(
          "desktop_mcp_logs_snapshot",
          { limit: typeof limit === "number" ? limit : null },
        )
      : callPreview(() => mockDesktopBridge.mcpLogsSnapshot(limit));
  },
  syncStatus() {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<SyncStatusSummary>("desktop_sync_status")
      : callPreview(() => mockDesktopBridge.syncStatus());
  },
  syncOutboxList(limit?: number) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<SyncOutboxListItem[]>(
          "desktop_sync_outbox_list",
          { limit: typeof limit === "number" ? limit : null },
        )
      : callPreview(() => mockDesktopBridge.syncOutboxList(limit));
  },
  syncBackfill(limit?: number) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<SyncBackfillSummary>(
          "desktop_sync_backfill",
          { limit: typeof limit === "number" ? limit : null },
        )
      : callPreview(() => mockDesktopBridge.syncBackfill(limit));
  },
  syncPush(limit?: number) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<SyncPushSummary>(
          "desktop_sync_push",
          { limit: typeof limit === "number" ? limit : null },
        )
      : callPreview(() => mockDesktopBridge.syncPush(limit));
  },
  syncPull(limit?: number) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<SyncPullSummary>(
          "desktop_sync_pull",
          { limit: typeof limit === "number" ? limit : null },
        )
      : callPreview(() => mockDesktopBridge.syncPull(limit));
  },
  onMcpStatus(listener: (payload: McpRuntimeStatus) => void) {
    return subscribeDesktopEvent<McpRuntimeStatus>("desktop://mcp-status", listener);
  },
  onMcpLog(listener: (payload: McpLogEntry) => void) {
    return subscribeDesktopEvent<McpLogEntry>("desktop://mcp-log", listener);
  },
  project(input: Record<string, unknown>) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<Project | Project[]>("desktop_project", input)
      : callPreview<Project | Project[]>(() => mockDesktopBridge.project(input));
  },
  version(input: Record<string, unknown>) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<Version | Version[]>("desktop_version", input)
      : callPreview<Version | Version[]>(() => mockDesktopBridge.version(input));
  },
  task(input: Record<string, unknown>) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<Task | TaskListPayload | Task[] | TaskActivity[] | TaskContextPayload | TaskRelation>("desktop_task", input).then(
          normalizeTaskEnvelope,
        )
      : callPreview<Task | TaskListPayload | Task[] | TaskActivity[] | TaskContextPayload | TaskRelation>(() =>
          mockDesktopBridge.task(input),
        ).then(normalizeTaskEnvelope);
  },
  note(input: Record<string, unknown>) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<TaskActivity | TaskActivity[]>("desktop_note", input)
      : callPreview<TaskActivity | TaskActivity[]>(() => mockDesktopBridge.note(input));
  },
  attachment(input: Record<string, unknown>) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<Attachment | Attachment[]>("desktop_attachment", input)
      : callPreview<Attachment | Attachment[]>(() => mockDesktopBridge.attachment(input));
  },
  search(input: Record<string, unknown>) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<SearchResponse>("desktop_search", input)
      : callPreview<SearchResponse>(() => mockDesktopBridge.search(input));
  },
  searchBackfill(limit?: number) {
    const input = { action: "backfill", limit: typeof limit === "number" ? limit : null };
    return resolveBridgeMode() === "desktop"
      ? callDesktop<SearchBackfillSummary>("desktop_search", input)
      : callPreview<SearchBackfillSummary>(() => mockDesktopBridge.searchBackfill(limit));
  },
  approval(input: Record<string, unknown>) {
    return resolveBridgeMode() === "desktop"
      ? callDesktop<ApprovalRequest | ApprovalRequest[]>("desktop_approval", input)
      : callPreview<ApprovalRequest | ApprovalRequest[]>(() => mockDesktopBridge.approval(input));
  },
  async openPath(path: string) {
    if (resolveBridgeMode() === "desktop") {
      await openPath(path);
      return;
    }
    await mockDesktopBridge.openPath(path);
  },
  async revealItemInDir(path: string) {
    if (resolveBridgeMode() === "desktop") {
      await revealItemInDir(path);
      return;
    }
    await mockDesktopBridge.openPath(path);
  },
  async revealAttachment(path: string) {
    await this.revealItemInDir(path);
  },
};
