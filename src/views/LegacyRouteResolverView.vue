<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { desktopBridge } from "../lib/desktop";
import { formatDesktopError } from "../lib/errorMessage";
import { buildProjectWorkspacePath, resolveProjectSlug, readRouteString } from "../lib/projectWorkspace";
import type { ApprovalRequest, Task, Version } from "../lib/types";
import { useProjectsStore } from "../stores/projects";
import { useShellStore } from "../stores/shell";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const projectsStore = useProjectsStore();
const { t } = useI18n({ useScope: "global" });

const legacySection = computed(() => String(route.meta.legacySection ?? "tasks"));

onMounted(async () => {
  try {
    if (projectsStore.projects.length === 0) {
      await projectsStore.loadProjects();
    }

    if (legacySection.value === "approvals") {
      await redirectLegacyApprovals();
      return;
    }

    await redirectLegacyTasks();
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
    await router.replace("/projects");
  }
});

async function redirectLegacyTasks() {
  let targetProject = resolveProjectSlug(projectsStore.projects, readRouteString(route.query.project));
  const taskId = readRouteString(route.query.task);
  const versionId = readRouteString(route.query.version);

  if (!targetProject && taskId) {
    const envelope = await desktopBridge.task({ action: "get", task: taskId });
    const task = envelope.result as Task;
    targetProject = resolveProjectSlug(projectsStore.projects, task.project_id);
  }

  if (!targetProject && versionId) {
    const envelope = await desktopBridge.version({ action: "get", version: versionId });
    const version = envelope.result as Version;
    targetProject = resolveProjectSlug(projectsStore.projects, version.project_id);
  }

  targetProject ??= projectsStore.projects[0]?.slug;

  await router.replace({
    path: targetProject ? buildProjectWorkspacePath(targetProject, "tasks") : "/projects",
    query: {
      status: readRouteString(route.query.status),
      task: taskId,
      version: versionId,
    },
  });
}

async function redirectLegacyApprovals() {
  let targetProject = resolveProjectSlug(projectsStore.projects, readRouteString(route.query.project));
  const requestId = readRouteString(route.query.request);

  if (!targetProject && requestId) {
    const envelope = await desktopBridge.approval({ action: "get", request_id: requestId });
    const request = envelope.result as ApprovalRequest;
    targetProject = request.project_ref ?? undefined;
  }

  targetProject ??= projectsStore.projects[0]?.slug;

  await router.replace({
    path: targetProject ? buildProjectWorkspacePath(targetProject, "approvals") : "/projects",
    query: {
      approvalScope: readRouteString(route.query.approvalScope) ?? "all",
      approvalState: readRouteString(route.query.approvalState),
      request: requestId,
    },
  });
}
</script>

<template>
  <section class="workspace-empty-state">
    <div class="workspace-empty-copy">
      <p class="section-label">{{ t("projects.workspaceKicker") }}</p>
      <h1 class="text-xl font-semibold text-[var(--text-main)]">{{ t("common.redirecting") }}</h1>
      <p class="mt-2 text-sm leading-6 text-[var(--text-muted)]">
        {{ t("projects.workspaceLoadingSummary") }}
      </p>
    </div>
  </section>
</template>
