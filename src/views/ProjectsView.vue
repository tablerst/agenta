<script setup lang="ts">
import { BadgeCheck, Folder, Plus } from "@lucide/vue";
import { computed, onMounted, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { coerceString, formatDateTime } from "../lib/format";
import { formatDesktopError } from "../lib/errorMessage";
import { projectStatusOptions, versionStatusOptions } from "../lib/options";
import { DesktopBridgeError } from "../lib/desktop";
import type { ProjectStatus, VersionStatus } from "../lib/types";
import { useProjectsStore } from "../stores/projects";
import { useShellStore } from "../stores/shell";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const projectsStore = useProjectsStore();
const { t } = useI18n({ useScope: "global" });

const createProjectForm = reactive({
  slug: "",
  name: "",
  description: "",
});

const projectForm = reactive({
  slug: "",
  name: "",
  description: "",
  status: "active" as ProjectStatus,
});

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

const selectedProjectKey = computed(() => coerceString(route.query.project as string | undefined));
const selectedVersionId = computed(() => coerceString(route.query.version as string | undefined));

const selectedProject = computed(() =>
  projectsStore.projects.find(
    (item) => item.project_id === selectedProjectKey.value || item.slug === selectedProjectKey.value,
  ) ?? null,
);

const selectedVersion = computed(() =>
  projectsStore.versions.find((item) => item.version_id === selectedVersionId.value) ?? null,
);

watch(
  selectedProject,
  async (project) => {
    if (!project) {
      return;
    }

    projectForm.slug = project.slug;
    projectForm.name = project.name;
    projectForm.description = project.description ?? "";
    projectForm.status = project.status;
    await projectsStore.loadVersions(project.slug);
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

watch(
  () => projectsStore.projects.map((item) => item.project_id).join("|"),
  async () => {
    if (projectsStore.loadingProjects) {
      return;
    }

    const firstProject = projectsStore.projects[0];
    const hasSelection = firstProject
      ? projectsStore.projects.some(
          (item) => item.project_id === selectedProjectKey.value || item.slug === selectedProjectKey.value,
        )
      : false;

    if (!firstProject) {
      return;
    }

    if (!hasSelection) {
      await router.replace({
        path: "/projects",
        query: {
          ...route.query,
          project: firstProject.slug,
        },
      });
    }
  },
  { immediate: true },
);

onMounted(async () => {
  await projectsStore.loadProjects();
  const fallbackProject = selectedProject.value ?? projectsStore.projects[0] ?? null;
  if (!selectedProject.value && fallbackProject) {
    await router.replace({
      path: "/projects",
      query: {
        ...route.query,
        project: fallbackProject.slug,
      },
    });
  }
  if (fallbackProject) {
    await projectsStore.loadVersions(fallbackProject.slug);
  }
});

async function selectProject(project: string) {
  await router.push({ path: "/projects", query: { project } });
}

async function selectVersion(version: string) {
  await router.push({
    path: "/projects",
    query: {
      project: selectedProject.value?.slug,
      version,
    },
  });
}

async function submitCreateProject() {
  try {
    const created = await projectsStore.createProject({
      slug: createProjectForm.slug,
      name: createProjectForm.name,
      description: createProjectForm.description || null,
    });
    shell.pushNotice("success", t("notices.projectCreated"));
    createProjectForm.slug = "";
    createProjectForm.name = "";
    createProjectForm.description = "";
    await router.push({ path: "/projects", query: { project: created.slug } });
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function submitProjectUpdate() {
  if (!selectedProject.value) {
    return;
  }

  try {
    const updated = await projectsStore.updateProject(selectedProject.value.slug, {
      slug: projectForm.slug,
      name: projectForm.name,
      description: projectForm.description || null,
      status: projectForm.status,
    });
    shell.pushNotice("success", t("notices.projectUpdated"));
    await router.push({
      path: "/projects",
      query: {
        project: updated.slug,
        version: selectedVersion.value?.version_id,
      },
    });
    await projectsStore.loadVersions(updated.slug);
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
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
    shell.pushNotice("success", t("notices.versionCreated"));
    createVersionForm.name = "";
    createVersionForm.description = "";
    createVersionForm.status = "planning";
    await selectVersion(created.version_id);
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

async function jumpToQueuedApproval(error: unknown) {
  if (
    error instanceof DesktopBridgeError &&
    error.code === "requires_human_review" &&
    typeof error.details === "object" &&
    error.details &&
    "approval_request_id" in error.details
  ) {
    const requestId = (error.details as Record<string, unknown>).approval_request_id;
    shell.pushNotice("info", t("notices.requestQueued"));
    await router.push({
      path: "/approvals",
      query: { approvalState: "pending", request: requestId as string },
    });
    return true;
  }

  return false;
}
</script>

<template>
  <section class="page-grid">
    <aside class="page-list px-4 py-5">
      <div class="mb-4 flex items-center justify-between">
        <div>
          <p class="section-label">{{ t("projects.listKicker") }}</p>
          <h1 class="text-lg font-semibold text-[var(--text-main)]">{{ t("projects.listTitle") }}</h1>
        </div>
        <span class="status-pill">{{ projectsStore.projects.length }}</span>
      </div>

      <section class="panel-section mb-4">
        <p class="section-label">{{ t("projects.createProject") }}</p>
        <div class="space-y-3">
          <label class="form-field">
            <span class="field-label">{{ t("projects.fields.slug") }}</span>
            <input
              v-model="createProjectForm.slug"
              class="control-input"
              :placeholder="t('projects.placeholders.slug')"
            />
          </label>
          <label class="form-field">
            <span class="field-label">{{ t("projects.fields.name") }}</span>
            <input
              v-model="createProjectForm.name"
              class="control-input"
              :placeholder="t('projects.placeholders.projectName')"
            />
          </label>
          <label class="form-field">
            <span class="field-label">{{ t("projects.fields.description") }}</span>
            <textarea
              v-model="createProjectForm.description"
              class="control-textarea"
              :placeholder="t('projects.placeholders.projectDescription')"
            />
          </label>
          <button class="primary-action spotlight-surface" @click="submitCreateProject">
            <Plus :size="15" />
            {{ t("projects.createProjectAction") }}
          </button>
        </div>
      </section>

      <div>
        <button
          v-for="project in projectsStore.projects"
          :key="project.project_id"
          v-spotlight
          class="list-row spotlight-surface"
          :class="{ 'list-row-active': selectedProject?.project_id === project.project_id }"
          @click="selectProject(project.slug)"
        >
          <Folder :size="16" />
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium text-[var(--text-main)]">{{ project.name }}</p>
            <p class="truncate text-xs text-[var(--text-muted)]">{{ project.slug }}</p>
          </div>
          <span class="status-pill">{{ t(`status.project.${project.status}`) }}</span>
        </button>
      </div>
    </aside>

    <div class="page-detail">
      <div v-if="selectedProject" class="space-y-5">
        <section class="glass-panel p-5">
          <div class="mb-4 flex items-start justify-between gap-3">
            <div>
              <p class="section-label">{{ t("projects.projectDetail") }}</p>
              <h2 class="text-2xl font-semibold text-[var(--text-main)]">
                {{ selectedProject.name }}
              </h2>
              <p class="mt-2 max-w-3xl text-sm leading-6 text-[var(--text-muted)]">
                {{ selectedProject.description || t("projects.emptyDescription") }}
              </p>
            </div>
            <span class="status-pill status-pill-success" v-if="selectedProject.default_version_id">
              {{ t("projects.defaultVersionSet") }}
            </span>
          </div>

          <div class="grid gap-3 md:grid-cols-2">
            <div class="panel-section">
              <p class="section-label">{{ t("projects.editProject") }}</p>
              <div class="space-y-3">
                <label class="form-field">
                  <span class="field-label">{{ t("projects.fields.slug") }}</span>
                  <input
                    v-model="projectForm.slug"
                    class="control-input"
                    :placeholder="t('projects.placeholders.slug')"
                  />
                </label>
                <label class="form-field">
                  <span class="field-label">{{ t("projects.fields.name") }}</span>
                  <input
                    v-model="projectForm.name"
                    class="control-input"
                    :placeholder="t('projects.placeholders.projectName')"
                  />
                </label>
                <label class="form-field">
                  <span class="field-label">{{ t("projects.fields.description") }}</span>
                  <textarea
                    v-model="projectForm.description"
                    class="control-textarea"
                    :placeholder="t('projects.placeholders.projectDescription')"
                  />
                </label>
                <label class="form-field">
                  <span class="field-label">{{ t("common.status") }}</span>
                  <select v-model="projectForm.status" class="control-select">
                    <option v-for="status in projectStatusOptions" :key="status" :value="status">
                      {{ t(`status.project.${status}`) }}
                    </option>
                  </select>
                </label>
                <button class="primary-action spotlight-surface" @click="submitProjectUpdate">
                  <BadgeCheck :size="15" />
                  {{ t("projects.saveProject") }}
                </button>
              </div>
            </div>

            <div class="panel-section">
              <p class="section-label">{{ t("projects.metadata") }}</p>
              <dl class="space-y-3 text-sm">
                <div>
                  <dt class="text-[var(--text-muted)]">{{ t("projects.created") }}</dt>
                  <dd>{{ formatDateTime(selectedProject.created_at) }}</dd>
                </div>
                <div>
                  <dt class="text-[var(--text-muted)]">{{ t("projects.updated") }}</dt>
                  <dd>{{ formatDateTime(selectedProject.updated_at) }}</dd>
                </div>
                <div>
                  <dt class="text-[var(--text-muted)]">{{ t("projects.defaultVersion") }}</dt>
                  <dd>{{ selectedProject.default_version_id || t("projects.notAssigned") }}</dd>
                </div>
              </dl>
            </div>
          </div>
        </section>

        <section class="grid gap-5 xl:grid-cols-[minmax(0,0.72fr)_minmax(320px,0.28fr)]">
          <div class="glass-panel p-5">
            <div class="mb-4 flex items-center justify-between">
              <div>
                <p class="section-label">{{ t("projects.versions") }}</p>
                <h3 class="text-lg font-semibold text-[var(--text-main)]">{{ t("projects.versionsTitle") }}</h3>
              </div>
              <span class="status-pill">{{ projectsStore.versions.length }}</span>
            </div>

            <div v-if="projectsStore.versions.length === 0" class="empty-state">
              {{ t("projects.versionsEmpty") }}
            </div>
            <div v-else>
              <button
                v-for="version in projectsStore.versions"
                :key="version.version_id"
                v-spotlight
                class="list-row spotlight-surface"
                :class="{ 'list-row-active': selectedVersion?.version_id === version.version_id }"
                @click="selectVersion(version.version_id)"
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
          </div>

          <div class="space-y-5">
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

            <section class="panel-section">
              <p class="section-label">{{ t("projects.selectedVersion") }}</p>
              <div v-if="selectedVersion" class="space-y-3">
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
              <div v-else class="empty-state">{{ t("projects.selectedVersionEmpty") }}</div>
            </section>
          </div>
        </section>
      </div>

      <div v-else class="empty-state">
        {{ t("projects.emptySelection") }}
      </div>
    </div>
  </section>
</template>
