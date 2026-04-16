<script setup lang="ts">
import { BadgeCheck } from "@lucide/vue";
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { DesktopBridgeError, desktopBridge } from "../lib/desktop";
import { formatDesktopError } from "../lib/errorMessage";
import { formatDateTime } from "../lib/format";
import { projectStatusOptions } from "../lib/options";
import { buildProjectWorkspacePath } from "../lib/projectWorkspace";
import type { ProjectStatus, Task } from "../lib/types";
import { useProjectsStore } from "../stores/projects";
import { useShellStore } from "../stores/shell";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const projectsStore = useProjectsStore();
const { t } = useI18n({ useScope: "global" });

const projectForm = reactive({
  slug: "",
  name: "",
  description: "",
  status: "active" as ProjectStatus,
});

const counts = ref({
  approvals: 0,
  tasks: 0,
  versions: 0,
});

const selectedProjectSlug = computed(() => String(route.params.projectSlug ?? ""));
const selectedProject = computed(() =>
  projectsStore.projects.find((item) => item.slug === selectedProjectSlug.value) ?? null,
);

watch(
  selectedProject,
  async (project) => {
    if (!project) {
      counts.value = {
        approvals: 0,
        tasks: 0,
        versions: 0,
      };
      projectForm.slug = "";
      projectForm.name = "";
      projectForm.description = "";
      projectForm.status = "active";
      return;
    }

    projectForm.slug = project.slug;
    projectForm.name = project.name;
    projectForm.description = project.description ?? "";
    projectForm.status = project.status;

    await projectsStore.loadVersions(project.slug);
    const [taskEnvelope, approvalEnvelope] = await Promise.all([
      desktopBridge.task({ action: "list", project: project.slug }),
      desktopBridge.approval({ action: "list", project: project.slug, status: "pending" }),
    ]);

    counts.value = {
      approvals: (approvalEnvelope.result as unknown[]).length,
      tasks: (taskEnvelope.result as Task[]).length,
      versions: projectsStore.versions.length,
    };
  },
  { immediate: true },
);

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
    await router.push(buildProjectWorkspacePath(updated.slug, "overview"));
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

function openSection(section: "versions" | "tasks" | "approvals") {
  if (!selectedProject.value) {
    return;
  }
  void router.push(buildProjectWorkspacePath(selectedProject.value.slug, section));
}
</script>

<template>
  <section class="workspace-section-grid">
    <aside class="workspace-list-pane overview-pane">
      <div v-if="selectedProject" class="workspace-pane-stack">
        <section class="overview-metrics">
          <button class="overview-metric-link spotlight-surface" type="button" @click="openSection('versions')">
            <span class="field-label">{{ t("projects.metrics.versions") }}</span>
            <strong class="overview-metric-value">{{ counts.versions }}</strong>
            <span class="overview-metric-meta">{{ t("projects.workspaceNav.versions") }}</span>
          </button>
          <button class="overview-metric-link spotlight-surface" type="button" @click="openSection('tasks')">
            <span class="field-label">{{ t("projects.metrics.tasks") }}</span>
            <strong class="overview-metric-value">{{ counts.tasks }}</strong>
            <span class="overview-metric-meta">{{ t("projects.workspaceNav.tasks") }}</span>
          </button>
          <button class="overview-metric-link spotlight-surface" type="button" @click="openSection('approvals')">
            <span class="field-label">{{ t("projects.metrics.pendingApprovals") }}</span>
            <strong class="overview-metric-value">{{ counts.approvals }}</strong>
            <span class="overview-metric-meta">{{ t("projects.workspaceNav.approvals") }}</span>
          </button>
        </section>

        <section class="overview-meta-block">
          <dl class="overview-definition-list">
            <div>
              <dt>{{ t("projects.fields.slug") }}</dt>
              <dd>{{ selectedProject.slug }}</dd>
            </div>
            <div>
              <dt>{{ t("projects.created") }}</dt>
              <dd>{{ formatDateTime(selectedProject.created_at) }}</dd>
            </div>
            <div>
              <dt>{{ t("projects.updated") }}</dt>
              <dd>{{ formatDateTime(selectedProject.updated_at) }}</dd>
            </div>
            <div>
              <dt>{{ t("projects.defaultVersion") }}</dt>
              <dd>{{ selectedProject.default_version_id || t("projects.notAssigned") }}</dd>
            </div>
          </dl>
        </section>
      </div>
    </aside>

    <div class="workspace-inspector-pane overview-editor-pane">
      <div v-if="selectedProject" class="workspace-pane-stack">
        <section class="overview-editor">
          <div class="overview-editor-copy">
            <p class="section-label">{{ t("routes.projects.sections.overview") }}</p>
            <h2 class="overview-editor-title">{{ selectedProject.name }}</h2>
            <p class="overview-editor-summary">{{ selectedProject.slug }}</p>
          </div>

          <div class="overview-field-grid">
            <label class="form-field">
              <span class="field-label">{{ t("projects.fields.slug") }}</span>
              <input
                v-model="projectForm.slug"
                class="quiet-control-input"
                :placeholder="t('projects.placeholders.slug')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("projects.fields.name") }}</span>
              <input
                v-model="projectForm.name"
                class="quiet-control-input"
                :placeholder="t('projects.placeholders.projectName')"
              />
            </label>
            <label class="form-field overview-field-wide">
              <span class="field-label">{{ t("projects.fields.description") }}</span>
              <textarea
                v-model="projectForm.description"
                class="quiet-control-textarea"
                :placeholder="t('projects.placeholders.projectDescription')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("common.status") }}</span>
              <select v-model="projectForm.status" class="quiet-control-select">
                <option v-for="status in projectStatusOptions" :key="status" :value="status">
                  {{ t(`status.project.${status}`) }}
                </option>
              </select>
            </label>
          </div>

          <div class="overview-editor-actions">
            <button class="primary-action spotlight-surface" @click="submitProjectUpdate">
              <BadgeCheck :size="15" />
              {{ t("projects.saveProject") }}
            </button>
          </div>
        </section>
      </div>
    </div>
  </section>
</template>
