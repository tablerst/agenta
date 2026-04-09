<script setup lang="ts">
import { BadgeCheck, Plus } from "@lucide/vue";
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { DesktopBridgeError } from "../lib/desktop";
import { formatDesktopError } from "../lib/errorMessage";
import { versionStatusOptions } from "../lib/options";
import { buildProjectWorkspacePath, mergeWorkspaceQuery } from "../lib/projectWorkspace";
import type { VersionStatus } from "../lib/types";
import { useProjectsStore } from "../stores/projects";
import { useShellStore } from "../stores/shell";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const projectsStore = useProjectsStore();
const { t } = useI18n({ useScope: "global" });

const createVersionForm = reactive({
  name: "",
  description: "",
  status: "planning" as VersionStatus,
});

const versionForm = reactive({
  name: "",
  description: "",
  status: "planning" as VersionStatus,
});

const selectedProjectSlug = computed(() => String(route.params.projectSlug ?? ""));
const selectedVersionId = computed(() => String(route.query.version ?? ""));
const selectedProject = computed(() =>
  projectsStore.projects.find((item) => item.slug === selectedProjectSlug.value) ?? null,
);
const selectedVersion = computed(
  () => projectsStore.versions.find((item) => item.version_id === selectedVersionId.value) ?? null,
);

watch(
  selectedProject,
  async (project) => {
    if (!project) {
      projectsStore.versions = [];
      return;
    }

    await projectsStore.loadVersions(project.slug);
    if (selectedVersionId.value && !projectsStore.versions.some((item) => item.version_id === selectedVersionId.value)) {
      await router.replace({
        path: buildProjectWorkspacePath(project.slug, "versions"),
      });
    }
  },
  { immediate: true },
);

watch(
  () => projectsStore.versions.map((item) => item.version_id).join("|"),
  async () => {
    if (projectsStore.loadingVersions || !selectedProject.value) {
      return;
    }

    const firstVersion = projectsStore.versions[0] ?? null;
    if (!firstVersion) {
      return;
    }

    if (!selectedVersionId.value || !selectedVersion.value) {
      await router.replace({
        path: buildProjectWorkspacePath(selectedProject.value.slug, "versions"),
        query: mergeWorkspaceQuery({ version: firstVersion.version_id }),
      });
    }
  },
  { immediate: true },
);

watch(
  selectedVersion,
  (version) => {
    if (!version) {
      versionForm.name = "";
      versionForm.description = "";
      versionForm.status = "planning";
      return;
    }

    versionForm.name = version.name;
    versionForm.description = version.description ?? "";
    versionForm.status = version.status;
  },
  { immediate: true },
);

async function updateQuery(version?: string) {
  if (!selectedProject.value) {
    return;
  }

  await router.push({
    path: buildProjectWorkspacePath(selectedProject.value.slug, "versions"),
    query: mergeWorkspaceQuery({ version }),
  });
}

async function jumpToQueuedApproval(error: unknown) {
  if (
    error instanceof DesktopBridgeError &&
    error.code === "requires_human_review" &&
    typeof error.details === "object" &&
    error.details &&
    "approval_request_id" in error.details &&
    selectedProject.value
  ) {
    const requestId = (error.details as Record<string, unknown>).approval_request_id;
    shell.pushNotice("info", t("notices.requestQueued"));
    await router.push({
      path: buildProjectWorkspacePath(selectedProject.value.slug, "approvals"),
      query: {
        approvalState: "pending",
        request: requestId as string,
      },
    });
    return true;
  }

  return false;
}

async function submitCreateVersion() {
  if (!selectedProject.value) {
    return;
  }

  try {
    const created = await projectsStore.createVersion({
      project: selectedProject.value.slug,
      name: createVersionForm.name,
      description: createVersionForm.description || null,
      status: createVersionForm.status,
    });
    createVersionForm.name = "";
    createVersionForm.description = "";
    createVersionForm.status = "planning";
    shell.pushNotice("success", t("notices.versionCreated"));
    await updateQuery(created.version_id);
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function submitVersionUpdate() {
  if (!selectedVersion.value || !selectedProject.value) {
    return;
  }

  try {
    await projectsStore.updateVersion(selectedVersion.value.version_id, {
      name: versionForm.name,
      description: versionForm.description || null,
      status: versionForm.status,
    });
    await projectsStore.loadVersions(selectedProject.value.slug);
    shell.pushNotice("success", t("notices.versionUpdated"));
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}
</script>

<template>
  <section class="workspace-section-grid">
    <aside class="workspace-list-pane">
      <div class="workspace-pane-stack">
        <section class="panel-section">
          <p class="section-label">{{ t("projects.createVersion") }}</p>
          <div class="space-y-3">
            <label class="form-field">
              <span class="field-label">{{ t("projects.fields.versionName") }}</span>
              <input
                v-model="createVersionForm.name"
                class="control-input"
                :placeholder="t('projects.placeholders.versionName')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("projects.fields.versionDescription") }}</span>
              <textarea
                v-model="createVersionForm.description"
                class="control-textarea"
                :placeholder="t('projects.placeholders.releaseNotes')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("common.status") }}</span>
              <select v-model="createVersionForm.status" class="control-select">
                <option v-for="status in versionStatusOptions" :key="status" :value="status">
                  {{ t(`status.version.${status}`) }}
                </option>
              </select>
            </label>
            <button class="primary-action spotlight-surface" @click="submitCreateVersion">
              <Plus :size="15" />
              {{ t("projects.addVersion") }}
            </button>
          </div>
        </section>

        <section class="glass-panel p-5">
          <div class="mb-4 flex items-center justify-between">
            <div>
              <p class="section-label">{{ t("projects.versions") }}</p>
              <h2 class="text-lg font-semibold text-[var(--text-main)]">
                {{ t("projects.versionsTitle") }}
              </h2>
            </div>
            <span class="status-pill">{{ projectsStore.versions.length }}</span>
          </div>

          <div v-if="projectsStore.versions.length === 0" class="empty-state">
            {{ t("projects.versionsEmpty") }}
          </div>
          <div v-else class="workspace-list-stack">
            <button
              v-for="version in projectsStore.versions"
              :key="version.version_id"
              v-spotlight
              class="list-row spotlight-surface"
              :class="{ 'list-row-active': selectedVersion?.version_id === version.version_id }"
              @click="updateQuery(version.version_id)"
            >
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm font-medium text-[var(--text-main)]">
                  {{ version.name }}
                </p>
                <p class="truncate text-xs text-[var(--text-muted)]">
                  {{ version.description || t("projects.emptyVersionDescription") }}
                </p>
              </div>
              <span class="status-pill">{{ t(`status.version.${version.status}`) }}</span>
            </button>
          </div>
        </section>
      </div>
    </aside>

    <div class="workspace-inspector-pane">
      <div class="workspace-pane-stack">
        <section v-if="selectedVersion" class="glass-panel p-5">
          <div class="mb-4 flex items-center justify-between gap-3">
            <div>
              <p class="section-label">{{ t("projects.selectedVersion") }}</p>
              <h2 class="text-2xl font-semibold text-[var(--text-main)]">{{ selectedVersion.name }}</h2>
            </div>
            <span class="status-pill">{{ t(`status.version.${selectedVersion.status}`) }}</span>
          </div>

          <div class="space-y-3">
            <label class="form-field">
              <span class="field-label">{{ t("projects.fields.versionName") }}</span>
              <input
                v-model="versionForm.name"
                class="control-input"
                :placeholder="t('projects.placeholders.versionName')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("projects.fields.versionDescription") }}</span>
              <textarea
                v-model="versionForm.description"
                class="control-textarea"
                :placeholder="t('projects.placeholders.releaseNotes')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("common.status") }}</span>
              <select v-model="versionForm.status" class="control-select">
                <option v-for="status in versionStatusOptions" :key="status" :value="status">
                  {{ t(`status.version.${status}`) }}
                </option>
              </select>
            </label>
            <button class="primary-action spotlight-surface" @click="submitVersionUpdate">
              <BadgeCheck :size="15" />
              {{ t("projects.saveVersion") }}
            </button>
          </div>
        </section>

        <div v-else class="empty-state">{{ t("projects.selectedVersionEmpty") }}</div>
      </div>
    </div>
  </section>
</template>
