import type { LocationQuery, LocationQueryRaw, RouteLocationNormalizedLoaded } from "vue-router";

import type { Project } from "./types";
import { projectWorkspaceSectionOptions, type ProjectWorkspaceSection } from "./options";

type QueryValue = string | null | undefined;

function cleanQueryValue(value: QueryValue) {
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim();
  return normalized || undefined;
}

export function readRouteString(value: unknown): string | undefined {
  if (Array.isArray(value)) {
    return readRouteString(value[0]);
  }
  return typeof value === "string" ? cleanQueryValue(value) : undefined;
}

export function findProjectByReference(projects: Project[], reference?: string | null): Project | null {
  if (!reference) {
    return null;
  }
  return projects.find((item) => item.slug === reference || item.project_id === reference) ?? null;
}

export function resolveProjectSlug(projects: Project[], reference?: string | null): string | undefined {
  const project = findProjectByReference(projects, reference);
  return project?.slug ?? reference ?? undefined;
}

export function buildProjectWorkspacePath(projectSlug: string, section: ProjectWorkspaceSection) {
  return `/projects/${projectSlug}/${section}`;
}

export function getProjectWorkspaceSection(route: RouteLocationNormalizedLoaded): ProjectWorkspaceSection {
  const section = String(route.meta.workspaceSection ?? "");
  if (projectWorkspaceSectionOptions.includes(section as ProjectWorkspaceSection)) {
    return section as ProjectWorkspaceSection;
  }
  return "overview";
}

export function mergeWorkspaceQuery(values: Record<string, QueryValue>) {
  const nextQuery: LocationQueryRaw = {};
  Object.entries(values).forEach(([key, value]) => {
    const normalized = cleanQueryValue(value);
    if (normalized) {
      nextQuery[key] = normalized;
    }
  });
  return nextQuery;
}

export function sanitizeSectionQuery(
  section: ProjectWorkspaceSection,
  query: LocationQuery,
): LocationQueryRaw {
  switch (section) {
    case "versions":
      return mergeWorkspaceQuery({
        version: readRouteString(query.version),
      });
    case "tasks":
      return mergeWorkspaceQuery({
        q: readRouteString(query.q),
        version: readRouteString(query.version),
        task: readRouteString(query.task),
        status: readRouteString(query.status),
      });
    case "approvals":
      return mergeWorkspaceQuery({
        request: readRouteString(query.request),
        approvalState: readRouteString(query.approvalState),
        approvalScope: readRouteString(query.approvalScope),
      });
    case "overview":
    default:
      return {};
  }
}

export function sanitizeProjectSwitchQuery(
  section: ProjectWorkspaceSection,
  query: LocationQuery,
): LocationQueryRaw {
  switch (section) {
    case "tasks":
      return mergeWorkspaceQuery({
        status: readRouteString(query.status),
      });
    case "approvals":
      return mergeWorkspaceQuery({
        approvalState: readRouteString(query.approvalState),
        approvalScope: readRouteString(query.approvalScope),
      });
    case "versions":
    case "overview":
    default:
      return {};
  }
}
