<script setup lang="ts">
import { BadgeCheck, Folder, Plus } from "@lucide/vue";
import { computed, onMounted, reactive, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

import { coerceString, formatDateTime } from "../lib/format";
import { DesktopBridgeError } from "../lib/desktop";
import type { ProjectStatus, VersionStatus } from "../lib/types";
import { useProjectsStore } from "../stores/projects";
import { useShellStore } from "../stores/shell";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const projectsStore = useProjectsStore();

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

onMounted(async () => {
  await projectsStore.loadProjects();
  if (selectedProject.value) {
    await projectsStore.loadVersions(selectedProject.value.slug);
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
    shell.pushNotice("success", "Project created");
    createProjectForm.slug = "";
    createProjectForm.name = "";
    createProjectForm.description = "";
    await router.push({ path: "/projects", query: { project: created.slug } });
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", (error as Error).message);
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
    shell.pushNotice("success", "Project updated");
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
    shell.pushNotice("error", (error as Error).message);
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
    shell.pushNotice("success", "Version created");
    createVersionForm.name = "";
    createVersionForm.description = "";
    createVersionForm.status = "planning";
    await selectVersion(created.version_id);
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", (error as Error).message);
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
    shell.pushNotice("success", "Version updated");
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", (error as Error).message);
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
    shell.pushNotice("info", "Request queued for human review.");
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
          <p class="section-label">Projects</p>
          <h1 class="text-lg font-semibold text-[var(--text-main)]">Project registry</h1>
        </div>
        <span class="status-pill">{{ projectsStore.projects.length }}</span>
      </div>

      <section class="panel-section mb-4">
        <p class="section-label">Create Project</p>
        <div class="space-y-3">
          <input v-model="createProjectForm.slug" class="control-input" placeholder="slug" />
          <input v-model="createProjectForm.name" class="control-input" placeholder="Project name" />
          <textarea
            v-model="createProjectForm.description"
            class="control-textarea"
            placeholder="Short context for the workspace"
          />
          <button class="primary-action spotlight-surface" @click="submitCreateProject">
            <Plus :size="15" />
            Create project
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
          <span class="status-pill">{{ project.status }}</span>
        </button>
      </div>
    </aside>

    <div class="page-detail">
      <div v-if="selectedProject" class="space-y-5">
        <section class="glass-panel p-5">
          <div class="mb-4 flex items-start justify-between gap-3">
            <div>
              <p class="section-label">Project Detail</p>
              <h2 class="text-2xl font-semibold text-[var(--text-main)]">
                {{ selectedProject.name }}
              </h2>
              <p class="mt-2 max-w-3xl text-sm leading-6 text-[var(--text-muted)]">
                {{ selectedProject.description || "No project description yet." }}
              </p>
            </div>
            <span class="status-pill status-pill-success" v-if="selectedProject.default_version_id">
              default version set
            </span>
          </div>

          <div class="grid gap-3 md:grid-cols-2">
            <div class="panel-section">
              <p class="section-label">Edit Project</p>
              <div class="space-y-3">
                <input v-model="projectForm.slug" class="control-input" placeholder="slug" />
                <input v-model="projectForm.name" class="control-input" placeholder="Project name" />
                <textarea v-model="projectForm.description" class="control-textarea" />
                <select v-model="projectForm.status" class="control-select">
                  <option value="active">active</option>
                  <option value="archived">archived</option>
                </select>
                <button class="primary-action spotlight-surface" @click="submitProjectUpdate">
                  <BadgeCheck :size="15" />
                  Save project
                </button>
              </div>
            </div>

            <div class="panel-section">
              <p class="section-label">Metadata</p>
              <dl class="space-y-3 text-sm">
                <div>
                  <dt class="text-[var(--text-muted)]">Created</dt>
                  <dd>{{ formatDateTime(selectedProject.created_at) }}</dd>
                </div>
                <div>
                  <dt class="text-[var(--text-muted)]">Updated</dt>
                  <dd>{{ formatDateTime(selectedProject.updated_at) }}</dd>
                </div>
                <div>
                  <dt class="text-[var(--text-muted)]">Default Version</dt>
                  <dd>{{ selectedProject.default_version_id || "Not assigned" }}</dd>
                </div>
              </dl>
            </div>
          </div>
        </section>

        <section class="grid gap-5 xl:grid-cols-[minmax(0,0.72fr)_minmax(320px,0.28fr)]">
          <div class="glass-panel p-5">
            <div class="mb-4 flex items-center justify-between">
              <div>
                <p class="section-label">Versions</p>
                <h3 class="text-lg font-semibold text-[var(--text-main)]">Release lanes</h3>
              </div>
              <span class="status-pill">{{ projectsStore.versions.length }}</span>
            </div>

            <div v-if="projectsStore.versions.length === 0" class="empty-state">
              No versions yet for this project.
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
                    {{ version.description || "No version description." }}
                  </p>
                </div>
                <span class="status-pill">{{ version.status }}</span>
              </button>
            </div>
          </div>

          <div class="space-y-5">
            <section class="panel-section">
              <p class="section-label">Create Version</p>
              <div class="space-y-3">
                <input v-model="createVersionForm.name" class="control-input" placeholder="v1" />
                <textarea
                  v-model="createVersionForm.description"
                  class="control-textarea"
                  placeholder="Scope and release notes"
                />
                <select v-model="createVersionForm.status" class="control-select">
                  <option value="planning">planning</option>
                  <option value="active">active</option>
                  <option value="closed">closed</option>
                  <option value="archived">archived</option>
                </select>
                <button class="primary-action spotlight-surface" @click="submitCreateVersion">
                  <Plus :size="15" />
                  Add version
                </button>
              </div>
            </section>

            <section class="panel-section">
              <p class="section-label">Selected Version</p>
              <div v-if="selectedVersion" class="space-y-3">
                <input v-model="versionForm.name" class="control-input" />
                <textarea v-model="versionForm.description" class="control-textarea" />
                <select v-model="versionForm.status" class="control-select">
                  <option value="planning">planning</option>
                  <option value="active">active</option>
                  <option value="closed">closed</option>
                  <option value="archived">archived</option>
                </select>
                <button class="primary-action spotlight-surface" @click="submitVersionUpdate">
                  <BadgeCheck :size="15" />
                  Save version
                </button>
              </div>
              <div v-else class="empty-state">Select a version to edit its metadata.</div>
            </section>
          </div>
        </section>
      </div>

      <div v-else class="empty-state">
        Select a project from the list to inspect metadata and manage versions.
      </div>
    </div>
  </section>
</template>
