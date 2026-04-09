import type {
  ApprovalScope,
  ApprovalStatus,
  AppLocale,
  AttachmentKind,
  ProjectStatus,
  TaskPriority,
  TaskStatus,
  VersionStatus,
} from "./types";

export const localeOptions: AppLocale[] = ["zh-CN", "en"];
export const projectStatusOptions: ProjectStatus[] = ["active", "archived"];
export const versionStatusOptions: VersionStatus[] = ["planning", "active", "closed", "archived"];
export const taskStatusOptions: TaskStatus[] = ["draft", "ready", "in_progress", "blocked", "done", "cancelled"];
export const taskPriorityOptions: TaskPriority[] = ["low", "normal", "high", "critical"];
export const attachmentKindOptions: AttachmentKind[] = [
  "artifact",
  "log",
  "report",
  "image",
  "screenshot",
  "patch",
  "other",
];
export const approvalStatusOptions: ApprovalStatus[] = ["pending", "approved", "denied", "failed"];
export const approvalScopeOptions: ApprovalScope[] = ["project", "all"];
export const projectWorkspaceSectionOptions = ["overview", "versions", "tasks", "approvals"] as const;
export const taskDetailTabOptions = ["overview", "notes", "attachments", "activity"] as const;

export type ProjectWorkspaceSection = (typeof projectWorkspaceSectionOptions)[number];
export type TaskDetailTab = (typeof taskDetailTabOptions)[number];
