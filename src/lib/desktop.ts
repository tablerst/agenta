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
  status() {
    return callDesktop<RuntimeStatus>("desktop_status");
  },
  project(input: Record<string, unknown>) {
    return callDesktop<Project | Project[]>("desktop_project", input);
  },
  version(input: Record<string, unknown>) {
    return callDesktop<Version | Version[]>("desktop_version", input);
  },
  task(input: Record<string, unknown>) {
    return callDesktop<Task | Task[] | TaskActivity[]>("desktop_task", input);
  },
  note(input: Record<string, unknown>) {
    return callDesktop<TaskActivity | TaskActivity[]>("desktop_note", input);
  },
  attachment(input: Record<string, unknown>) {
    return callDesktop<Attachment | Attachment[]>("desktop_attachment", input);
  },
  search(input: Record<string, unknown>) {
    return callDesktop<SearchResponse>("desktop_search", input);
  },
  approval(input: Record<string, unknown>) {
    return callDesktop<ApprovalRequest | ApprovalRequest[]>("desktop_approval", input);
  },
  async revealAttachment(path: string) {
    await openPath(path);
  },
};
