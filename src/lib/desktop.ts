import { invoke } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";

import type {
  AppBridgeError,
  ApprovalRequest,
  Attachment,
  Project,
  RuntimeStatus,
  SearchResponse,
  SuccessEnvelope,
  Task,
  TaskActivity,
  Version,
} from "./types";
import { mockDesktopBridge } from "./mockDesktop";

export type BridgeMode = "desktop" | "preview";

function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const bridgeMode: BridgeMode = hasTauriRuntime() ? "desktop" : "preview";

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

  return new DesktopBridgeError({
    code: "desktop_bridge_error",
    message: "Unknown desktop bridge error",
    details: error,
  });
}

export const desktopBridge = {
  mode: bridgeMode,
  status() {
    return bridgeMode === "desktop"
      ? callDesktop<RuntimeStatus>("desktop_status")
      : callPreview(() => mockDesktopBridge.status());
  },
  project(input: Record<string, unknown>) {
    return bridgeMode === "desktop"
      ? callDesktop<Project | Project[]>("desktop_project", input)
      : callPreview<Project | Project[]>(() => mockDesktopBridge.project(input));
  },
  version(input: Record<string, unknown>) {
    return bridgeMode === "desktop"
      ? callDesktop<Version | Version[]>("desktop_version", input)
      : callPreview<Version | Version[]>(() => mockDesktopBridge.version(input));
  },
  task(input: Record<string, unknown>) {
    return bridgeMode === "desktop"
      ? callDesktop<Task | Task[] | TaskActivity[]>("desktop_task", input)
      : callPreview<Task | Task[] | TaskActivity[]>(() => mockDesktopBridge.task(input));
  },
  note(input: Record<string, unknown>) {
    return bridgeMode === "desktop"
      ? callDesktop<TaskActivity | TaskActivity[]>("desktop_note", input)
      : callPreview<TaskActivity | TaskActivity[]>(() => mockDesktopBridge.note(input));
  },
  attachment(input: Record<string, unknown>) {
    return bridgeMode === "desktop"
      ? callDesktop<Attachment | Attachment[]>("desktop_attachment", input)
      : callPreview<Attachment | Attachment[]>(() => mockDesktopBridge.attachment(input));
  },
  search(input: Record<string, unknown>) {
    return bridgeMode === "desktop"
      ? callDesktop<SearchResponse>("desktop_search", input)
      : callPreview<SearchResponse>(() => mockDesktopBridge.search(input));
  },
  approval(input: Record<string, unknown>) {
    return bridgeMode === "desktop"
      ? callDesktop<ApprovalRequest | ApprovalRequest[]>("desktop_approval", input)
      : callPreview<ApprovalRequest | ApprovalRequest[]>(() => mockDesktopBridge.approval(input));
  },
  async revealAttachment(path: string) {
    if (bridgeMode === "desktop") {
      await openPath(path);
      return;
    }
    await mockDesktopBridge.revealAttachment(path);
  },
};
