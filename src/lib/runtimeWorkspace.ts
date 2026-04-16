import type { RouteLocationNormalizedLoaded } from "vue-router";

export const runtimeWorkspaceSectionOptions = ["host", "sync"] as const;

export type RuntimeWorkspaceSection = (typeof runtimeWorkspaceSectionOptions)[number];

export function buildRuntimeWorkspacePath(section: RuntimeWorkspaceSection = "host") {
  return `/runtime/${section}`;
}

export function getRuntimeWorkspaceSection(
  route: RouteLocationNormalizedLoaded,
): RuntimeWorkspaceSection {
  const section = String(route.meta.runtimeSection ?? "");

  if (
    runtimeWorkspaceSectionOptions.includes(
      section as RuntimeWorkspaceSection,
    )
  ) {
    return section as RuntimeWorkspaceSection;
  }

  return "host";
}
