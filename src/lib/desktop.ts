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
  SearchResponse,
  SuccessEnvelope,
  SyncBackfillSummary,
  SyncOutboxListItem,
  SyncPullSummary,
  SyncPushSummary,
  SyncStatusSummary,
  Task,
  TaskActivity,
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

async function callDesktop<TResult>(
  command: string,
  input?: Record<string, unknown>,
): Promise<SuccessEnvelope<TResult>> {
  try {
    return await invoke<SuccessEnvelope<TResult>>(command, input ? { input } : undefined);
  } catch (error) {
    throw normalizeError(error);
  }
}

async function callPreview<TResult>(
  loader: () => Promise<SuccessEnvelope<TResult>>,
): Promise<SuccessEnvelope<TResult>> {
  try {
    return await loader();
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

async function subscribeDesktopEvent<TPayload>(
  event: string,
  listener: (payload: TPayload) => void,
): Promise<UnlistenFn> {
  if (resolveBridgeMode() !== "desktop") {
    return async () => undefined;
  }

  return listen<TPayload>(event, (eventPayload) => {
    listener(eventPayload.payload);
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
      ? callDesktop<Task | Task[] | TaskActivity[]>("desktop_task", input)
      : callPreview<Task | Task[] | TaskActivity[]>(() => mockDesktopBridge.task(input));
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
