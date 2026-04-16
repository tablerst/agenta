<script setup lang="ts">
import {
  BadgeCheck,
  Check,
  Clock3,
  FileText,
  ListFilter,
  Paperclip,
  Plus,
  SquareKanban,
  X,
} from "@lucide/vue";
import { computed, nextTick, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import JsonBlock from "../components/JsonBlock.vue";
import { DesktopBridgeError, desktopBridge } from "../lib/desktop";
import { formatDesktopError } from "../lib/errorMessage";
import { formatDateTime } from "../lib/format";
import { buildProjectWorkspacePath, mergeWorkspaceQuery, readRouteString } from "../lib/projectWorkspace";
import {
  attachmentKindOptions,
  taskDetailTabOptions,
  taskPriorityOptions,
  taskStatusOptions,
  type TaskDetailTab,
} from "../lib/options";
import type { TaskPriority, TaskStatus } from "../lib/types";
import { useApprovalsStore } from "../stores/approvals";
import { useProjectsStore } from "../stores/projects";
import { useShellStore } from "../stores/shell";
import { useTasksStore } from "../stores/tasks";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const projectsStore = useProjectsStore();
const tasksStore = useTasksStore();
const approvalsStore = useApprovalsStore();
const { t } = useI18n({ useScope: "global" });

const detailTab = ref<TaskDetailTab>("overview");
const tabRefs = new Map<TaskDetailTab, HTMLButtonElement>();
const isCreatingTask = ref(false);
const createTaskTitleInput = ref<HTMLInputElement | null>(null);
const taskCreateTrigger = ref<HTMLElement | null>(null);

const createTaskForm = reactive({
  title: "",
  summary: "",
  description: "",
  status: "ready" as TaskStatus,
  priority: "normal" as TaskPriority,
  created_by: "desktop",
});

const taskForm = reactive({
  title: "",
  summary: "",
  description: "",
  status: "ready" as TaskStatus,
  priority: "normal" as TaskPriority,
  updated_by: "desktop",
});

const noteForm = reactive({
  content: "",
});

const attachmentForm = reactive({
  kind: "artifact",
  path: "",
  summary: "",
});

const selectedProjectSlug = computed(() => String(route.params.projectSlug ?? ""));
const selectedVersionId = computed(() => readRouteString(route.query.version) ?? "");
const selectedTaskId = computed(() => readRouteString(route.query.task) ?? "");
const selectedStatus = computed(() => readRouteString(route.query.status) ?? "");
const selectedProject = computed(
  () => projectsStore.projects.find((item) => item.slug === selectedProjectSlug.value) ?? null,
);
const selectedTask = computed(() => {
  if (!selectedTaskId.value) {
    return null;
  }

  return (
    tasksStore.tasks.find((item) => item.task_id === selectedTaskId.value) ??
    (tasksStore.currentTask?.task_id === selectedTaskId.value ? tasksStore.currentTask : null)
  );
});
const selectedProjectLabel = computed(() => selectedProject.value?.name || selectedProjectSlug.value);

function queryValue(
  overrides: Record<"status" | "task" | "version", string | undefined>,
  key: "status" | "task" | "version",
  current: string,
) {
  return key in overrides ? overrides[key] : current || undefined;
}

function buildTaskQuery(overrides: Partial<Record<"status" | "task" | "version", string | undefined>> = {}) {
  return mergeWorkspaceQuery({
    status: queryValue(overrides as Record<"status" | "task" | "version", string | undefined>, "status", selectedStatus.value),
    task: queryValue(overrides as Record<"status" | "task" | "version", string | undefined>, "task", selectedTaskId.value),
    version: queryValue(overrides as Record<"status" | "task" | "version", string | undefined>, "version", selectedVersionId.value),
  });
}

async function updateQuery(
  overrides: Partial<Record<"status" | "task" | "version", string | undefined>>,
  replace = false,
) {
  if (!selectedProjectSlug.value) {
    return;
  }

  const location = {
    path: buildProjectWorkspacePath(selectedProjectSlug.value, "tasks"),
    query: buildTaskQuery(overrides),
  };

  if (replace) {
    await router.replace(location);
    return;
  }

  await router.push(location);
}

function registerTabRef(
  tab: TaskDetailTab,
  element:
    | Element
    | { $el?: Element | null }
    | null,
) {
  const resolvedElement =
    element instanceof Element
      ? element
      : typeof element === "object" && element && "$el" in element
        ? element.$el ?? null
        : null;

  if (resolvedElement instanceof HTMLButtonElement) {
    tabRefs.set(tab, resolvedElement);
    return;
  }
  tabRefs.delete(tab);
}

function focusTab(tab: TaskDetailTab) {
  tabRefs.get(tab)?.focus();
}

function resetCreateTaskForm() {
  createTaskForm.title = "";
  createTaskForm.summary = "";
  createTaskForm.description = "";
  createTaskForm.status = "ready";
  createTaskForm.priority = "normal";
}

function focusCreateTaskField() {
  void nextTick(() => {
    createTaskTitleInput.value?.focus();
  });
}

function restoreTaskCreateFocus() {
  void nextTick(() => {
    taskCreateTrigger.value?.focus();
  });
}

function openCreateTask(event?: Event) {
  if (event?.currentTarget instanceof HTMLElement) {
    taskCreateTrigger.value = event.currentTarget;
  }
  isCreatingTask.value = true;
  focusCreateTaskField();
}

function cancelCreateTask() {
  resetCreateTaskForm();
  isCreatingTask.value = false;
  restoreTaskCreateFocus();
}

async function selectTask(taskId: string) {
  isCreatingTask.value = false;
  await updateQuery({ task: taskId });
}

async function updateVersionFilter(version: string) {
  isCreatingTask.value = false;
  await updateQuery({ version: version || undefined, task: undefined });
}

async function updateStatusFilter(status: string) {
  isCreatingTask.value = false;
  await updateQuery({ status: status || undefined, task: undefined });
}

function handleTabKeydown(event: KeyboardEvent, tab: TaskDetailTab) {
  const index = taskDetailTabOptions.indexOf(tab);

  if (index === -1) {
    return;
  }

  let nextTab = tab;

  switch (event.key) {
    case "ArrowRight":
    case "ArrowDown":
      nextTab = taskDetailTabOptions[(index + 1) % taskDetailTabOptions.length];
      break;
    case "ArrowLeft":
    case "ArrowUp":
      nextTab = taskDetailTabOptions[(index - 1 + taskDetailTabOptions.length) % taskDetailTabOptions.length];
      break;
    case "Home":
      nextTab = taskDetailTabOptions[0];
      break;
    case "End":
      nextTab = taskDetailTabOptions[taskDetailTabOptions.length - 1];
      break;
    default:
      return;
  }

  event.preventDefault();
  detailTab.value = nextTab;
  focusTab(nextTab);
}

watch(
  selectedTask,
  (task) => {
    if (!task) {
      taskForm.title = "";
      taskForm.summary = "";
      taskForm.description = "";
      taskForm.status = "ready";
      taskForm.priority = "normal";
      return;
    }

    taskForm.title = task.title;
    taskForm.summary = task.summary ?? "";
    taskForm.description = task.description ?? "";
    taskForm.status = task.status;
    taskForm.priority = task.priority;
  },
  { immediate: true },
);

watch(
  selectedProjectSlug,
  () => {
    cancelCreateTask();
  },
);

watch(
  () => selectedProjectSlug.value,
  async (projectSlug) => {
    if (!projectSlug) {
      projectsStore.versions = [];
      return;
    }
    await projectsStore.loadVersions(projectSlug);
  },
  { immediate: true },
);

watch(
  () => [selectedProjectSlug.value, selectedVersionId.value, selectedStatus.value] as const,
  async ([projectSlug, version, status]) => {
    if (!projectSlug) {
      tasksStore.tasks = [];
      tasksStore.clearTaskDetail();
      return;
    }

    await tasksStore.loadTasks({
      project: projectSlug,
      status: status || undefined,
      version: version || undefined,
    });
  },
  { immediate: true },
);

watch(
  () => tasksStore.tasks.map((item) => item.task_id).join("|"),
  async () => {
    if (tasksStore.loadingTasks) {
      return;
    }

    const firstTask = tasksStore.tasks[0] ?? null;
    const hasSelection = Boolean(
      selectedTaskId.value &&
      tasksStore.tasks.some((item) => item.task_id === selectedTaskId.value),
    );

    if (!firstTask) {
      tasksStore.clearTaskDetail();
      if (selectedTaskId.value) {
        await updateQuery({ task: undefined }, true);
      }
      return;
    }

    if (!hasSelection) {
      await updateQuery({ task: firstTask.task_id }, true);
    }
  },
  { immediate: true },
);

watch(
  selectedTaskId,
  async (taskId) => {
    detailTab.value = "overview";

    if (!taskId) {
      tasksStore.clearTaskDetail();
      return;
    }

    await tasksStore.loadTaskDetail(taskId);
  },
  { immediate: true },
);

onMounted(async () => {
  await Promise.allSettled([projectsStore.loadProjects(), approvalsStore.refreshPendingCount()]);
});

async function submitCreateTask() {
  if (!selectedProject.value) {
    shell.pushNotice("error", t("notices.selectProjectBeforeTask"));
    return;
  }

  try {
    const created = await tasksStore.createTask({
      ...createTaskForm,
      description: createTaskForm.description || null,
      project: selectedProject.value.slug,
      summary: createTaskForm.summary || null,
      version: selectedVersionId.value || undefined,
    });
    await tasksStore.loadTasks({
      project: selectedProject.value.slug,
      status: selectedStatus.value || undefined,
      version: selectedVersionId.value || undefined,
    });
    await approvalsStore.refreshPendingCount();
    shell.pushNotice("success", t("notices.taskSubmitted"));
    resetCreateTaskForm();
    isCreatingTask.value = false;
    await updateQuery({ task: created.task_id });
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
    await approvalsStore.refreshPendingCount();
  }
}

async function submitTaskUpdate() {
  if (!selectedTask.value) {
    return;
  }

  try {
    await tasksStore.updateTask(selectedTask.value.task_id, {
      description: taskForm.description || null,
      priority: taskForm.priority,
      status: taskForm.status,
      summary: taskForm.summary || null,
      title: taskForm.title,
      updated_by: taskForm.updated_by,
      version: selectedVersionId.value || selectedTask.value.version_id || undefined,
    });
    await tasksStore.loadTasks({
      project: selectedProjectSlug.value || undefined,
      status: selectedStatus.value || undefined,
      version: selectedVersionId.value || undefined,
    });
    await approvalsStore.refreshPendingCount();
    shell.pushNotice("success", t("notices.taskUpdated"));
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
    await approvalsStore.refreshPendingCount();
  }
}

async function submitNote() {
  if (!selectedTask.value) {
    return;
  }

  try {
    await tasksStore.createNote(selectedTask.value.task_id, {
      content: noteForm.content,
      created_by: "desktop",
    });
    noteForm.content = "";
    shell.pushNotice("success", t("notices.noteAdded"));
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function submitAttachment() {
  if (!selectedTask.value) {
    return;
  }

  try {
    await tasksStore.createAttachment(selectedTask.value.task_id, {
      created_by: "desktop",
      kind: attachmentForm.kind,
      path: attachmentForm.path,
      summary: attachmentForm.summary || null,
    });
    attachmentForm.path = "";
    attachmentForm.summary = "";
    shell.pushNotice("success", t("notices.attachmentAdded"));
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function openAttachment(path: string) {
  try {
    await desktopBridge.revealAttachment(path);
  } catch (error) {
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
      path: buildProjectWorkspacePath(selectedProjectSlug.value, "approvals"),
      query: mergeWorkspaceQuery({
        approvalState: "pending",
        request: requestId as string,
      }),
    });
    return true;
  }

  return false;
}
</script>

<template>
  <section class="workspace-section-grid">
    <aside class="workspace-list-pane">
      <div class="workspace-pane-stack">
        <section class="workspace-list-toolbar">
          <div class="workspace-list-toolbar-head">
            <div>
              <p class="section-label">{{ t("tasks.listKicker") }}</p>
              <h2 class="text-lg font-semibold text-[var(--text-main)]">{{ t("tasks.listTitle") }}</h2>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <span v-if="selectedProjectLabel" class="status-pill">{{ selectedProjectLabel }}</span>
              <span class="status-pill">{{ tasksStore.tasks.length }}</span>
            </div>
          </div>
          <div class="workspace-filter-toolbar">
            <div class="workspace-filter-title">
              <ListFilter :size="14" />
              <span>{{ t("tasks.filters") }}</span>
            </div>
            <label class="compact-field">
              <span class="field-label">{{ t("routes.projects.sections.versions") }}</span>
              <select
                class="control-select compact-control"
                :value="selectedVersionId"
                @change="updateVersionFilter(($event.target as HTMLSelectElement).value)"
              >
                <option value="">{{ t("tasks.allVersions") }}</option>
                <option v-for="version in projectsStore.versions" :key="version.version_id" :value="version.version_id">
                  {{ version.name }}
                </option>
              </select>
            </label>
            <label class="compact-field">
              <span class="field-label">{{ t("common.status") }}</span>
              <select
                class="control-select compact-control"
                :value="selectedStatus"
                @change="updateStatusFilter(($event.target as HTMLSelectElement).value)"
              >
                <option value="">{{ t("tasks.allStatuses") }}</option>
                <option v-for="status in taskStatusOptions" :key="status" :value="status">
                  {{ t(`status.task.${status}`) }}
                </option>
              </select>
            </label>
            <button class="primary-action spotlight-surface" type="button" @click="openCreateTask($event)">
              <Plus :size="15" />
              {{ t("tasks.createTaskAction") }}
            </button>
          </div>
        </section>

        <section class="workspace-list-region">
          <div v-if="tasksStore.tasks.length === 0" class="empty-state">
            {{ t("tasks.noMatches") }}
          </div>
          <div v-else class="workspace-row-list">
            <button
              v-for="task in tasksStore.tasks"
              :key="task.task_id"
              v-spotlight
              class="list-row spotlight-surface"
              :class="{ 'list-row-active': selectedTask?.task_id === task.task_id }"
              @click="selectTask(task.task_id)"
            >
              <SquareKanban :size="16" />
              <div class="min-w-0 flex-1 space-y-2">
                <div class="flex items-center justify-between gap-3">
                  <p class="truncate text-sm font-medium text-[var(--text-main)]">{{ task.title }}</p>
                  <span class="status-pill">{{ t(`status.task.${task.status}`) }}</span>
                </div>
                <p class="truncate text-xs text-[var(--text-muted)]">{{ task.summary || task.task_context_digest }}</p>
              </div>
              <div class="list-row-meta">
                <span>{{ t(`status.priority.${task.priority}`) }}</span>
                <span>{{ formatDateTime(task.updated_at) }}</span>
              </div>
            </button>
          </div>
        </section>
      </div>
    </aside>

    <div class="workspace-inspector-pane">
      <div v-if="isCreatingTask" class="workspace-pane-stack">
        <section class="overview-editor">
          <div class="overview-editor-copy">
            <p class="section-label">{{ t("tasks.createTask") }}</p>
            <h2 class="overview-editor-title">{{ t("tasks.createTask") }}</h2>
            <p class="overview-editor-summary">{{ selectedProjectLabel }}</p>
          </div>

          <div class="overview-field-grid">
            <label class="form-field overview-field-wide">
              <span class="field-label">{{ t("tasks.fields.title") }}</span>
              <input
                ref="createTaskTitleInput"
                v-model="createTaskForm.title"
                class="quiet-control-input"
                :placeholder="t('tasks.placeholders.title')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("tasks.fields.summary") }}</span>
              <input
                v-model="createTaskForm.summary"
                class="quiet-control-input"
                :placeholder="t('tasks.placeholders.summary')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("common.status") }}</span>
              <select v-model="createTaskForm.status" class="quiet-control-select">
                <option v-for="status in taskStatusOptions" :key="status" :value="status">
                  {{ t(`status.task.${status}`) }}
                </option>
              </select>
            </label>
            <label class="form-field overview-field-wide">
              <span class="field-label">{{ t("tasks.fields.description") }}</span>
              <textarea
                v-model="createTaskForm.description"
                class="quiet-control-textarea"
                :placeholder="t('tasks.placeholders.executionContext')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("tasks.fields.priority") }}</span>
              <select v-model="createTaskForm.priority" class="quiet-control-select">
                <option v-for="priority in taskPriorityOptions" :key="priority" :value="priority">
                  {{ t(`status.priority.${priority}`) }}
                </option>
              </select>
            </label>
          </div>

          <div class="overview-editor-actions gap-2">
            <button class="secondary-action spotlight-surface" type="button" @click="cancelCreateTask">
              <X :size="15" />
              {{ t("common.cancel") }}
            </button>
            <button class="primary-action spotlight-surface" type="button" @click="submitCreateTask">
              <Plus :size="15" />
              {{ t("tasks.createTaskAction") }}
            </button>
          </div>
        </section>
      </div>

      <div v-else-if="selectedTask" class="workspace-pane-stack">
        <section class="glass-panel p-5">
          <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
            <div>
              <p class="section-label">{{ t("tasks.taskDetail") }}</p>
              <h2 class="text-2xl font-semibold text-[var(--text-main)]">{{ selectedTask.title }}</h2>
              <p class="mt-2 max-w-3xl text-sm leading-6 text-[var(--text-muted)]">
                {{ selectedTask.summary || t("tasks.noSummary") }}
              </p>
            </div>
            <div class="flex items-center gap-2">
              <span class="status-pill">{{ t(`status.task.${selectedTask.status}`) }}</span>
              <span class="status-pill">{{ t(`status.priority.${selectedTask.priority}`) }}</span>
            </div>
          </div>

          <div class="task-detail-tablist" role="tablist" :aria-label="t('tasks.detailTabs')">
            <button
              v-for="tab in taskDetailTabOptions"
              :key="tab"
              :id="`task-tab-${tab}`"
              :ref="(element) => registerTabRef(tab, element)"
              :aria-controls="`task-panel-${tab}`"
              :aria-selected="detailTab === tab"
              :tabindex="detailTab === tab ? 0 : -1"
              class="task-detail-tab"
              :class="{ 'task-detail-tab-active': detailTab === tab }"
              role="tab"
              type="button"
              @click="detailTab = tab"
              @keydown="handleTabKeydown($event, tab)"
            >
              {{ t(`tasks.tabs.${tab}`) }}
            </button>
          </div>

          <div
            v-if="detailTab === 'overview'"
            id="task-panel-overview"
            aria-labelledby="task-tab-overview"
            class="space-y-5"
            role="tabpanel"
            tabindex="0"
          >
            <div class="grid gap-5 xl:grid-cols-[minmax(0,0.66fr)_minmax(320px,0.34fr)]">
              <section class="panel-section">
                <p class="section-label">{{ t("tasks.editTask") }}</p>
                <div class="space-y-3">
                  <input v-model="taskForm.title" class="control-input" />
                  <input v-model="taskForm.summary" class="control-input" />
                  <textarea v-model="taskForm.description" class="control-textarea" />
                  <div class="grid gap-3 md:grid-cols-2">
                    <select v-model="taskForm.status" class="control-select">
                      <option v-for="status in taskStatusOptions" :key="status" :value="status">
                        {{ t(`status.task.${status}`) }}
                      </option>
                    </select>
                    <select v-model="taskForm.priority" class="control-select">
                      <option v-for="priority in taskPriorityOptions" :key="priority" :value="priority">
                        {{ t(`status.priority.${priority}`) }}
                      </option>
                    </select>
                  </div>
                  <button class="primary-action spotlight-surface" @click="submitTaskUpdate">
                    <BadgeCheck :size="15" />
                    {{ t("tasks.saveTask") }}
                  </button>
                </div>
              </section>

              <section class="panel-section">
                <p class="section-label">{{ t("tasks.contextDigest") }}</p>
                <JsonBlock :value="selectedTask" />
              </section>
            </div>
          </div>

          <div
            v-else-if="detailTab === 'notes'"
            id="task-panel-notes"
            aria-labelledby="task-tab-notes"
            class="space-y-4"
            role="tabpanel"
            tabindex="0"
          >
            <section class="panel-section">
              <p class="section-label">{{ t("tasks.addNote") }}</p>
              <textarea
                v-model="noteForm.content"
                class="control-textarea"
                :placeholder="t('tasks.placeholders.note')"
              />
              <button class="primary-action mt-3 spotlight-surface" @click="submitNote">
                <FileText :size="15" />
                {{ t("tasks.addNoteAction") }}
              </button>
            </section>
            <section class="space-y-3">
              <article v-for="note in tasksStore.notes" :key="note.activity_id" class="panel-section">
                <div class="mb-2 flex items-center justify-between">
                  <span class="status-pill">{{ t(`activityKind.${note.kind}`) }}</span>
                  <span class="text-xs text-[var(--text-muted)]">{{ formatDateTime(note.created_at) }}</span>
                </div>
                <p class="text-sm leading-6 text-[var(--text-main)]">{{ note.content }}</p>
              </article>
              <div v-if="tasksStore.notes.length === 0" class="empty-state">
                {{ t("tasks.notesEmpty") }}
              </div>
            </section>
          </div>

          <div
            v-else-if="detailTab === 'attachments'"
            id="task-panel-attachments"
            aria-labelledby="task-tab-attachments"
            class="space-y-4"
            role="tabpanel"
            tabindex="0"
          >
            <section class="panel-section">
              <p class="section-label">{{ t("tasks.addAttachment") }}</p>
              <div class="space-y-3">
                <input
                  v-model="attachmentForm.path"
                  class="control-input"
                  :placeholder="t('tasks.placeholders.attachmentPath')"
                />
                <input
                  v-model="attachmentForm.summary"
                  class="control-input"
                  :placeholder="t('tasks.placeholders.attachmentSummary')"
                />
                <select v-model="attachmentForm.kind" class="control-select">
                  <option v-for="kind in attachmentKindOptions" :key="kind" :value="kind">
                    {{ t(`status.attachmentKind.${kind}`) }}
                  </option>
                </select>
                <button class="primary-action spotlight-surface" @click="submitAttachment">
                  <Paperclip :size="15" />
                  {{ t("tasks.addAttachmentAction") }}
                </button>
              </div>
            </section>
            <article v-for="attachment in tasksStore.attachments" :key="attachment.attachment_id" class="panel-section">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <div>
                  <p class="text-sm font-medium text-[var(--text-main)]">{{ attachment.summary }}</p>
                  <p class="text-xs text-[var(--text-muted)]">{{ attachment.original_filename }}</p>
                </div>
                <div class="flex items-center gap-2">
                  <span class="status-pill">{{ t(`status.attachmentKind.${attachment.kind}`) }}</span>
                  <button class="secondary-action spotlight-surface" @click="openAttachment(attachment.original_path)">
                    <Check :size="15" />
                    {{ t("tasks.openPath") }}
                  </button>
                </div>
              </div>
              <div class="mt-3 text-xs text-[var(--text-muted)]">
                {{ attachment.original_path }}
              </div>
            </article>
            <div v-if="tasksStore.attachments.length === 0" class="empty-state">
              {{ t("tasks.attachmentsEmpty") }}
            </div>
          </div>

          <div
            v-else
            id="task-panel-activity"
            aria-labelledby="task-tab-activity"
            class="space-y-3"
            role="tabpanel"
            tabindex="0"
          >
            <article v-for="item in tasksStore.activities" :key="item.activity_id" class="panel-section">
              <div class="mb-2 flex items-center justify-between">
                <span class="status-pill">{{ t(`activityKind.${item.kind}`) }}</span>
                <span class="inline-flex items-center gap-1 text-xs text-[var(--text-muted)]">
                  <Clock3 :size="13" />
                  {{ formatDateTime(item.created_at) }}
                </span>
              </div>
              <p class="text-sm text-[var(--text-main)]">{{ item.content }}</p>
              <div v-if="Object.keys(item.metadata_json || {}).length > 0" class="mt-3">
                <JsonBlock :value="item.metadata_json" />
              </div>
            </article>
            <div v-if="tasksStore.activities.length === 0" class="empty-state">
              {{ t("tasks.activityEmpty") }}
            </div>
          </div>
        </section>
      </div>

      <div v-else class="empty-state">
        {{ t("tasks.emptySelection") }}
      </div>
    </div>
  </section>
</template>
