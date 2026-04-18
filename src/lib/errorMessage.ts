import { DesktopBridgeError } from "./desktop";

type Translate = (key: string, named?: Record<string, unknown>) => string;

function asRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value === "object" && value) {
    return value as Record<string, unknown>;
  }

  return null;
}

function resolveFieldLabel(field: string, t: Translate): string {
  switch (field) {
    case "content":
      return t("tasks.fields.note");
    case "name":
      return t("projects.fields.name");
    case "path":
      return t("tasks.fields.attachmentPath");
    case "project":
      return t("routes.projects.title");
    case "query":
      return t("sidebar.search");
    case "request_id":
      return t("approvals.request");
    case "review_note":
      return t("approvals.fields.reviewNote");
    case "reviewed_by":
      return t("approvals.fields.reviewedBy");
    case "slug":
      return t("projects.fields.slug");
    case "summary":
      return t("tasks.fields.summary");
    case "task":
      return t("routes.tasks.title");
    case "title":
      return t("tasks.fields.title");
    case "version":
      return t("projects.fields.versionName");
    default:
      return field;
  }
}

function resolveEntityLabel(entity: string, t: Translate): string {
  switch (entity) {
    case "approval_request":
      return t("approvals.request");
    case "project":
      return t("routes.projects.title");
    case "task":
      return t("routes.tasks.title");
    case "version":
      return t("projects.versions");
    default:
      return t("common.resource");
  }
}

export function formatDesktopError(error: unknown, t: Translate): string {
  if (!(error instanceof DesktopBridgeError)) {
    if (error instanceof Error && error.message) {
      return error.message;
    }

    return t("notices.errors.unknown");
  }

  const details = asRecord(error.details);

  switch (error.code) {
    case "conflict":
      if (typeof details?.slug === "string") {
        return t("notices.errors.projectSlugConflict", { slug: details.slug });
      }

      if (typeof details?.project_id === "string" && typeof details?.version_id === "string") {
        return t("notices.errors.versionProjectConflict");
      }

      return t("notices.errors.conflict");

    case "desktop_bridge_error":
      return error.message || t("notices.errors.desktopBridgeError");

    case "invalid_action":
      return t("notices.errors.invalidAction");

    case "invalid_arguments": {
      const field =
        typeof details?.field === "string" ? resolveFieldLabel(details.field, t) : null;

      return field
        ? t("notices.errors.invalidArgumentsField", { field })
        : t("notices.errors.invalidArguments");
    }

    case "ambiguous_context":
      return t("notices.errors.ambiguousContext");

    case "not_found": {
      const entity =
        typeof details?.entity === "string"
          ? resolveEntityLabel(details.entity, t)
          : t("common.resource");

      return t("notices.errors.notFound", { entity });
    }

    case "storage_busy":
      return t("notices.errors.storageBusy");

    default:
      return t("notices.errors.unknown");
  }
}
