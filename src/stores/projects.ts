import { ref } from "vue";
import { defineStore } from "pinia";

import { desktopBridge } from "../lib/desktop";
import type { Project, Version } from "../lib/types";

export const useProjectsStore = defineStore("projects", () => {
  const projects = ref<Project[]>([]);
  const versions = ref<Version[]>([]);
  const loadingProjects = ref(false);
  const loadingVersions = ref(false);

  async function loadProjects() {
    loadingProjects.value = true;
    try {
      const envelope = await desktopBridge.project({ action: "list" });
      projects.value = envelope.result as Project[];
      return projects.value;
    } finally {
      loadingProjects.value = false;
    }
  }

  async function loadVersions(project?: string) {
    loadingVersions.value = true;
    try {
      const envelope = await desktopBridge.version({
        action: "list",
        project,
      });
      versions.value = envelope.result as Version[];
      return versions.value;
    } finally {
      loadingVersions.value = false;
    }
  }

  async function createProject(payload: Record<string, unknown>) {
    const envelope = await desktopBridge.project({ action: "create", ...payload });
    await loadProjects();
    return envelope.result as Project;
  }

  async function updateProject(project: string, payload: Record<string, unknown>) {
    const envelope = await desktopBridge.project({
      action: "update",
      project,
      ...payload,
    });
    await loadProjects();
    return envelope.result as Project;
  }

  async function createVersion(payload: Record<string, unknown>) {
    const envelope = await desktopBridge.version({ action: "create", ...payload });
    if (typeof payload.project === "string") {
      await loadVersions(payload.project);
    }
    await loadProjects();
    return envelope.result as Version;
  }

  async function updateVersion(version: string, payload: Record<string, unknown>) {
    const envelope = await desktopBridge.version({
      action: "update",
      version,
      ...payload,
    });
    return envelope.result as Version;
  }

  return {
    createProject,
    createVersion,
    loadProjects,
    loadVersions,
    loadingProjects,
    loadingVersions,
    projects,
    updateProject,
    updateVersion,
    versions,
  };
});
