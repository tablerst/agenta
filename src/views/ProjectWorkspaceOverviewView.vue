<script setup lang="ts">
import { BadgeCheck, Folder, FolderCog, FolderOpen } from "@lucide/vue";
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { DesktopBridgeError, desktopBridge } from "../lib/desktop";
import { formatDesktopError } from "../lib/errorMessage";
import { formatDateTime } from "../lib/format";
import { projectStatusOptions } from "../lib/options";
import { buildProjectWorkspacePath } from "../lib/projectWorkspace";
import type { ContextInitResult, ProjectStatus, Task } from "../lib/types";
import { useProjectsStore } from "../stores/projects";
import { useShellStore } from "../stores/shell";

const CONTEXT_STATE_STORAGE_KEY = "agenta.project_context_state";
const DEFAULT_CONTEXT_SOURCE_KEY = "__DEFAULT__";

interface PersistedProjectContextState {
  contextDir: string;
  workspaceRoot: string;
  contextResult: ContextInitResult | null;
  contextResultSourceKey: string | null;
}

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

const contextForm = reactive({
  contextDir: "",
  workspaceRoot: "",
  force: false,
});

const contextResult = ref<ContextInitResult | null>(null);
const contextResultSourceKey = ref<string | null>(null);

const counts = ref({
  approvals: 0,
  tasks: 0,
  versions: 0,
});

let hydratingContextState = false;

const selectedProjectSlug = computed(() => String(route.params.projectSlug ?? ""));
const selectedProject = computed(() =>
  projectsStore.projects.find((item) => item.slug === selectedProjectSlug.value) ?? null,
);
const canOpenContextFolder = computed(() => Boolean(contextResult.value?.context_dir));

function normalizeContextDir(path: string) {
  return path.trim().replace(/\\/g, "/").replace(/\/+$/, "");
}

function buildContextSourceKey(contextDir: string, workspaceRoot: string) {
  const normalizedContextDir = normalizeContextDir(contextDir);
  if (normalizedContextDir) {
    return `context:${normalizedContextDir}`;
  }

  const normalizedWorkspaceRoot = normalizeContextDir(workspaceRoot);
  if (normalizedWorkspaceRoot) {
    return `workspace:${normalizedWorkspaceRoot}`;
  }

  return DEFAULT_CONTEXT_SOURCE_KEY;
}

function readPersistedProjectContextMap(): Record<string, PersistedProjectContextState> {
  try {
    const raw = window.localStorage.getItem(CONTEXT_STATE_STORAGE_KEY);
    if (!raw) {
      return {};
    }
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") {
      return {};
    }
    return parsed as Record<string, PersistedProjectContextState>;
  } catch {
    return {};
  }
}

function writePersistedProjectContextMap(nextState: Record<string, PersistedProjectContextState>) {
  window.localStorage.setItem(CONTEXT_STATE_STORAGE_KEY, JSON.stringify(nextState));
}

function persistSelectedProjectContextState() {
  if (!selectedProject.value) {
    return;
  }

  const persistedState = readPersistedProjectContextMap();
  persistedState[selectedProject.value.slug] = {
    contextDir: contextForm.contextDir,
    workspaceRoot: contextForm.workspaceRoot,
    contextResult: contextResult.value,
    contextResultSourceKey: contextResultSourceKey.value,
  };
  writePersistedProjectContextMap(persistedState);
}

function restoreProjectContextState(projectSlug: string) {
  hydratingContextState = true;
  try {
    const persistedState = readPersistedProjectContextMap()[projectSlug];
    contextForm.contextDir = persistedState?.contextDir ?? "";
    contextForm.workspaceRoot = persistedState?.workspaceRoot ?? "";
    contextResult.value = persistedState?.contextResult ?? null;
    contextResultSourceKey.value = persistedState?.contextResultSourceKey ?? null;
  } finally {
    hydratingContextState = false;
  }
}

function resetProjectContextState() {
  hydratingContextState = true;
  try {
    contextForm.contextDir = "";
    contextForm.workspaceRoot = "";
    contextForm.force = false;
    contextResult.value = null;
    contextResultSourceKey.value = null;
  } finally {
    hydratingContextState = false;
  }
}

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
      resetProjectContextState();
      return;
    }

    projectForm.slug = project.slug;
    projectForm.name = project.name;
    projectForm.description = project.description ?? "";
    projectForm.status = project.status;
    restoreProjectContextState(project.slug);

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

watch(
  () => [contextForm.contextDir, contextForm.workspaceRoot],
  () => {
    if (hydratingContextState || !selectedProject.value) {
      return;
    }

    const nextSourceKey = buildContextSourceKey(contextForm.contextDir, contextForm.workspaceRoot);
    if (
      contextResult.value &&
      contextResultSourceKey.value &&
      nextSourceKey !== contextResultSourceKey.value
    ) {
      contextResult.value = null;
      contextResultSourceKey.value = null;
    }

    persistSelectedProjectContextState();
  },
);

watch([contextResult, contextResultSourceKey], () => {
  if (hydratingContextState || !selectedProject.value) {
    return;
  }
  persistSelectedProjectContextState();
});

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

async function chooseContextFolder() {
  try {
    const selectedDirectory = await desktopBridge.pickDirectory(
      contextForm.workspaceRoot.trim() || contextResult.value?.context_dir,
    );
    if (!selectedDirectory) {
      return;
    }
    contextForm.workspaceRoot = selectedDirectory;
    contextForm.contextDir = "";
  } catch (error) {
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
    await router.push(buildProjectWorkspacePath(updated.slug, "overview"));
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function initProjectContext() {
  if (!selectedProject.value) {
    return;
  }

  try {
    const envelope = await desktopBridge.context({
      action: "init",
      project: selectedProject.value.slug,
      workspace_root: contextForm.workspaceRoot || null,
      context_dir: contextForm.contextDir || null,
      force: contextForm.force,
    });
    const result = envelope.result as ContextInitResult;
    contextResult.value = result;
    contextResultSourceKey.value = buildContextSourceKey(contextForm.contextDir, contextForm.workspaceRoot);
    if (result.status === "updated") {
      shell.pushNotice("success", t("notices.projectContextUpdated"));
    } else if (result.status === "unchanged") {
      shell.pushNotice("info", t("notices.projectContextUnchanged"));
    } else {
      shell.pushNotice("success", t("notices.projectContextInitialized"));
    }
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function openContextFolder() {
  if (!contextResult.value) {
    return;
  }

  try {
    await desktopBridge.revealItemInDir(contextResult.value.context_dir);
  } catch (error) {
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

        <section class="overview-meta-block">
          <div class="overview-editor-copy">
            <p class="section-label">{{ t("projects.contextSummary") }}</p>
            <h3 class="overview-editor-title">{{ t("projects.contextInit") }}</h3>
            <p class="overview-editor-summary">{{ t("projects.contextHelp") }}</p>
          </div>

          <div class="overview-field-grid">
            <label class="form-field overview-field-wide">
              <span class="field-label">{{ t("projects.fields.contextDir") }}</span>
              <div class="flex items-center gap-2">
                <input
                  v-model="contextForm.contextDir"
                  class="quiet-control-input flex-1"
                  :placeholder="t('projects.placeholders.contextDir')"
                />
                <button class="secondary-action spotlight-surface shrink-0" type="button" @click="chooseContextFolder">
                  <Folder :size="15" />
                  {{ t("projects.contextSelectWorkspace") }}
                </button>
              </div>
            </label>
          </div>

          <p v-if="contextForm.workspaceRoot" class="mt-3 text-xs text-[var(--text-muted)]">
            {{ t("projects.selectedWorkspaceRoot") }}: {{ contextForm.workspaceRoot }}
          </p>

          <label class="inline-flex items-center gap-2 text-sm text-[var(--text-muted)]">
            <input v-model="contextForm.force" type="checkbox" />
            <span>{{ t("projects.contextForce") }}</span>
          </label>

          <p v-if="contextResult" class="mt-3 text-xs text-[var(--text-muted)]">
            {{ contextResult.manifest_path }}
          </p>

          <div class="overview-editor-actions gap-2">
            <button class="primary-action spotlight-surface" type="button" @click="initProjectContext">
              <FolderCog :size="15" />
              {{ t("projects.contextInit") }}
            </button>
            <button
              v-if="canOpenContextFolder"
              class="secondary-action spotlight-surface"
              type="button"
              @click="openContextFolder"
            >
              <FolderOpen :size="15" />
              {{ t("projects.contextOpenFolder") }}
            </button>
          </div>
        </section>
      </div>
    </div>
  </section>
</template>
