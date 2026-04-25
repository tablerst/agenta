<script setup lang="ts">
import {
  BadgeCheck,
  Check,
  Clock3,
  FileText,
  ListFilter,
  Paperclip,
  Plus,
  Search,
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
import { localizeEvidenceSource, renderHighlightedEvidence } from "../lib/searchEvidence";
import {
  attachmentKindOptions,
  knowledgeStatusOptions,
  taskDetailTabOptions,
  taskKindOptions,
  taskPriorityOptions,
  taskStatusOptions,
  type TaskDetailTab,
} from "../lib/options";
import type {
  KnowledgeStatus,
  NoteKind,
  ProjectSearchFilters,
  SearchResponse,
  TaskKind,
  TaskPriority,
  TaskStatus,
} from "../lib/types";
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
const projectSearchInput = ref("");
const projectSearchResults = ref<SearchResponse | null>(null);
const projectSearchLoading = ref(false);
const projectSearchError = ref("");
const showAdvancedFilters = ref(false);
let projectSearchTimer: number | undefined;
let projectSearchRequestId = 0;

const createTaskForm = reactive({
  task_code: "",
  kind: "standard" as TaskKind,
  title: "",
  summary: "",
  description: "",
  status: "ready" as TaskStatus,
  priority: "normal" as TaskPriority,
  created_by: "desktop",
});

const taskForm = reactive({
  task_code: "",
  kind: "standard" as TaskKind,
  title: "",
  summary: "",
  description: "",
  status: "ready" as TaskStatus,
  priority: "normal" as TaskPriority,
  updated_by: "desktop",
});

const noteForm = reactive({
  content: "",
  note_kind: "finding" as NoteKind,
});

const attachmentForm = reactive({
  kind: "artifact",
  path: "",
  summary: "",
});

const relationForm = reactive({
  blockerTaskId: "",
  childTaskId: "",
});

const selectedProjectSlug = computed(() => String(route.params.projectSlug ?? ""));
const selectedVersionId = computed(() => readRouteString(route.query.version) ?? "");
const selectedTaskId = computed(() => readRouteString(route.query.task) ?? "");
const selectedStatus = computed(() => readRouteString(route.query.status) ?? "");
const selectedSearchQuery = computed(() => readRouteString(route.query.q) ?? "");
const selectedTaskKindFilter = ref("");
const selectedPriorityFilter = ref("");
const selectedKnowledgeStatusFilter = ref("");
const taskCodePrefixFilter = ref("");
const selectedSortBy = ref("task_code");
const selectedSortOrder = ref("asc");
const noteKindOptions = ["scratch", "finding", "conclusion"] as const;
const taskSortOptions = [
  "task_code",
  "title",
  "latest_activity_at",
  "updated_at",
  "created_at",
] as const;
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
const isProjectSearchActive = computed(() => Boolean(selectedSearchQuery.value));
const projectSearchResultTaskIds = computed(() => {
  const taskIds = new Set<string>();
  projectSearchResults.value?.tasks.forEach((item) => taskIds.add(item.task_id));
  projectSearchResults.value?.activities.forEach((item) => taskIds.add(item.task_id));
  return Array.from(taskIds);
});
const projectSearchPrimaryTaskId = computed(
  () => projectSearchResults.value?.tasks[0]?.task_id ?? projectSearchResults.value?.activities[0]?.task_id ?? null,
);
const projectSearchTaskMap = computed(
  () => new Map((projectSearchResults.value?.tasks ?? []).map((item) => [item.task_id, item])),
);
const projectSearchRetrievalStatus = computed(() => {
  if (!projectSearchResults.value) {
    return "";
  }

  if (projectSearchResults.value.meta.retrieval_mode === "structured_only") {
    return t("search.retrieval.structuredOnly");
  }
  if (projectSearchResults.value.meta.vector_status === "indexing") {
    return t("search.retrieval.indexing");
  }
  if (projectSearchResults.value.meta.retrieval_mode === "hybrid") {
    return t("search.retrieval.hybrid");
  }
  return t("search.retrieval.lexicalFallback");
});
const activeTaskFilterLabels = computed(() => {
  const labels: string[] = [];
  if (selectedTaskKindFilter.value) {
    labels.push(
      t("tasks.filterSummary.taskKind", {
        value: t(`status.taskKind.${selectedTaskKindFilter.value}`),
      }),
    );
  }
  if (selectedPriorityFilter.value) {
    labels.push(
      t("tasks.filterSummary.priority", {
        value: t(`status.priority.${selectedPriorityFilter.value}`),
      }),
    );
  }
  if (selectedKnowledgeStatusFilter.value) {
    labels.push(
      t("tasks.filterSummary.knowledgeStatus", {
        value: t(`status.knowledge.${selectedKnowledgeStatusFilter.value}`),
      }),
    );
  }
  if (taskCodePrefixFilter.value.trim()) {
    labels.push(
      t("tasks.filterSummary.taskCodePrefix", {
        value: taskCodePrefixFilter.value.trim(),
      }),
    );
  }
  if (selectedSortBy.value !== "task_code" || selectedSortOrder.value !== "asc") {
    labels.push(
      t("tasks.filterSummary.sort", {
        field: t(`tasks.sortBy.${selectedSortBy.value}`),
        order: t(`tasks.sortOrder.${selectedSortOrder.value}`),
      }),
    );
  }
  return labels;
});
const activeTaskFilterCount = computed(() => activeTaskFilterLabels.value.length);

type TaskQueryKey = "q" | "status" | "task" | "version";

function queryValue(
  overrides: Record<TaskQueryKey, string | undefined>,
  key: TaskQueryKey,
  current: string,
) {
  return key in overrides ? overrides[key] : current || undefined;
}

function buildTaskQuery(overrides: Partial<Record<TaskQueryKey, string | undefined>> = {}) {
  return mergeWorkspaceQuery({
    q: queryValue(overrides as Record<TaskQueryKey, string | undefined>, "q", selectedSearchQuery.value),
    status: queryValue(overrides as Record<TaskQueryKey, string | undefined>, "status", selectedStatus.value),
    task: queryValue(overrides as Record<TaskQueryKey, string | undefined>, "task", selectedTaskId.value),
    version: queryValue(overrides as Record<TaskQueryKey, string | undefined>, "version", selectedVersionId.value),
  });
}

async function updateQuery(
  overrides: Partial<Record<TaskQueryKey, string | undefined>>,
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

function buildTaskListFilters() {
  return {
    project: selectedProjectSlug.value || undefined,
    kind: selectedTaskKindFilter.value || undefined,
    status: selectedStatus.value || undefined,
    task_code_prefix: taskCodePrefixFilter.value || undefined,
    sort_by: selectedSortBy.value || undefined,
    sort_order: selectedSortOrder.value || undefined,
    version: selectedVersionId.value || undefined,
  };
}

function buildProjectSearchFilters(): ProjectSearchFilters {
  return {
    project: selectedProjectSlug.value,
    query: selectedSearchQuery.value,
    version: selectedVersionId.value || undefined,
    status: selectedStatus.value ? (selectedStatus.value as TaskStatus) : undefined,
    priority: selectedPriorityFilter.value ? (selectedPriorityFilter.value as TaskPriority) : undefined,
    knowledge_status: selectedKnowledgeStatusFilter.value
      ? (selectedKnowledgeStatusFilter.value as KnowledgeStatus)
      : undefined,
    task_kind: selectedTaskKindFilter.value ? (selectedTaskKindFilter.value as TaskKind) : undefined,
    task_code_prefix: taskCodePrefixFilter.value || undefined,
    limit: 20,
  };
}

async function runProjectSearch() {
  if (!selectedProjectSlug.value || !selectedSearchQuery.value) {
    projectSearchRequestId += 1;
    projectSearchLoading.value = false;
    projectSearchResults.value = null;
    projectSearchError.value = "";
    return;
  }

  const requestId = projectSearchRequestId + 1;
  projectSearchRequestId = requestId;
  projectSearchLoading.value = true;
  projectSearchResults.value = null;
  projectSearchError.value = "";

  try {
    const envelope = await desktopBridge.search({
      action: "query",
      ...buildProjectSearchFilters(),
    });
    if (requestId !== projectSearchRequestId) {
      return;
    }
    projectSearchResults.value = envelope.result as SearchResponse;
  } catch (error) {
    if (requestId !== projectSearchRequestId) {
      return;
    }
    projectSearchError.value = formatDesktopError(error, t);
  } finally {
    if (requestId === projectSearchRequestId) {
      projectSearchLoading.value = false;
    }
  }
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
  createTaskForm.task_code = "";
  createTaskForm.kind = "standard";
  createTaskForm.title = "";
  createTaskForm.summary = "";
  createTaskForm.description = "";
  createTaskForm.status = "ready";
  createTaskForm.priority = "normal";
}

function clearAdvancedTaskFilters() {
  selectedTaskKindFilter.value = "";
  selectedPriorityFilter.value = "";
  selectedKnowledgeStatusFilter.value = "";
  taskCodePrefixFilter.value = "";
  selectedSortBy.value = "task_code";
  selectedSortOrder.value = "asc";
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
      taskForm.task_code = "";
      taskForm.kind = "standard";
      taskForm.summary = "";
      taskForm.description = "";
      taskForm.status = "ready";
      taskForm.priority = "normal";
      relationForm.blockerTaskId = "";
      relationForm.childTaskId = "";
      return;
    }

    taskForm.task_code = task.task_code ?? "";
    taskForm.kind = task.task_kind;
    taskForm.title = task.title;
    taskForm.summary = task.summary ?? "";
    taskForm.description = task.description ?? "";
    taskForm.status = task.status;
    taskForm.priority = task.priority;
    relationForm.blockerTaskId = "";
    relationForm.childTaskId = "";
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
  selectedSearchQuery,
  (value) => {
    if (projectSearchInput.value !== value) {
      projectSearchInput.value = value;
    }
  },
  { immediate: true },
);

watch(projectSearchInput, (value) => {
  window.clearTimeout(projectSearchTimer);
  projectSearchTimer = window.setTimeout(() => {
    const normalized = value.trim();
    if (normalized === selectedSearchQuery.value) {
      return;
    }
    void updateQuery({ q: normalized || undefined, task: undefined }, true);
  }, 160);
});

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
  () =>
    [
      selectedProjectSlug.value,
      selectedSearchQuery.value,
      selectedVersionId.value,
      selectedStatus.value,
      selectedTaskKindFilter.value,
      selectedPriorityFilter.value,
      selectedKnowledgeStatusFilter.value,
      taskCodePrefixFilter.value,
      selectedSortBy.value,
      selectedSortOrder.value,
    ] as const,
  async ([projectSlug]) => {
    if (!projectSlug) {
      tasksStore.tasks = [];
      tasksStore.clearTaskDetail();
      projectSearchResults.value = null;
      projectSearchError.value = "";
      return;
    }

    await reloadVisibleTasks();
  },
  { immediate: true },
);

watch(
  () => [
    isProjectSearchActive.value ? "search" : "list",
    projectSearchLoading.value ? "loading" : "idle",
    projectSearchResults.value?.tasks.map((item) => item.task_id).join("|") ?? "",
    projectSearchResults.value?.activities.map((item) => `${item.activity_id}:${item.task_id}`).join("|") ?? "",
    projectSearchError.value,
    tasksStore.tasks.map((item) => item.task_id).join("|"),
  ] as const,
  async () => {
    if (isProjectSearchActive.value) {
      if (projectSearchLoading.value || (!projectSearchResults.value && !projectSearchError.value)) {
        return;
      }
    } else if (tasksStore.loadingTasks) {
      return;
    }

    const visibleTaskIds = isProjectSearchActive.value ? projectSearchResultTaskIds.value : tasksStore.tasks.map((item) => item.task_id);
    const firstTask = isProjectSearchActive.value ? projectSearchPrimaryTaskId.value : tasksStore.tasks[0]?.task_id ?? null;
    const hasSelection = Boolean(
      selectedTaskId.value &&
      visibleTaskIds.includes(selectedTaskId.value),
    );

    if (!firstTask) {
      tasksStore.clearTaskDetail();
      if (selectedTaskId.value) {
        await updateQuery({ task: undefined }, true);
      }
      return;
    }

    if (!hasSelection) {
      await updateQuery({ task: firstTask }, true);
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
      kind: createTaskForm.kind,
      project: selectedProject.value.slug,
      summary: createTaskForm.summary || null,
      task_code: createTaskForm.task_code || null,
      version: selectedVersionId.value || undefined,
    });
    await reloadVisibleTasks();
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
      kind: taskForm.kind,
      priority: taskForm.priority,
      status: taskForm.status,
      summary: taskForm.summary || null,
      task_code: taskForm.task_code || null,
      title: taskForm.title,
      updated_by: taskForm.updated_by,
      version: selectedVersionId.value || selectedTask.value.version_id || undefined,
    });
    await reloadVisibleTasks();
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
      note_kind: noteForm.note_kind,
    });
    noteForm.content = "";
    noteForm.note_kind = "finding";
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

async function reloadVisibleTasks() {
  if (isProjectSearchActive.value) {
    await runProjectSearch();
    return;
  }

  projectSearchResults.value = null;
  projectSearchError.value = "";
  await tasksStore.loadTasks(buildTaskListFilters());
}

async function jumpToLinkedTask(taskId: string) {
  await selectTask(taskId);
}

async function submitAttachChild() {
  if (!selectedTask.value || !relationForm.childTaskId.trim()) {
    return;
  }

  try {
    await tasksStore.attachChild(selectedTask.value.task_id, relationForm.childTaskId.trim(), "desktop");
    relationForm.childTaskId = "";
    await reloadVisibleTasks();
    shell.pushNotice("success", t("notices.taskRelationUpdated"));
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function submitAddBlocker() {
  if (!selectedTask.value || !relationForm.blockerTaskId.trim()) {
    return;
  }

  try {
    await tasksStore.addBlocker(selectedTask.value.task_id, relationForm.blockerTaskId.trim(), "desktop");
    relationForm.blockerTaskId = "";
    await reloadVisibleTasks();
    shell.pushNotice("success", t("notices.taskRelationUpdated"));
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function handleDetachChild(childTaskId: string) {
  if (!selectedTask.value) {
    return;
  }

  try {
    await tasksStore.detachChild(selectedTask.value.task_id, childTaskId, "desktop");
    await reloadVisibleTasks();
    shell.pushNotice("success", t("notices.taskRelationUpdated"));
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function handleResolveBlocker(relationId: string) {
  if (!selectedTask.value) {
    return;
  }

  try {
    await tasksStore.resolveBlocker(selectedTask.value.task_id, {
      relation_id: relationId,
      updated_by: "desktop",
    });
    await reloadVisibleTasks();
    shell.pushNotice("success", t("notices.taskRelationUpdated"));
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
              <template v-if="isProjectSearchActive">
                <span v-if="projectSearchLoading" class="status-pill">{{ t("search.loading") }}</span>
                <template v-else-if="projectSearchResults">
                  <span class="status-pill">
                    {{ t("tasks.projectSearch.taskHits", { count: projectSearchResults.tasks.length }) }}
                  </span>
                  <span class="status-pill">
                    {{ t("tasks.projectSearch.activityHits", { count: projectSearchResults.activities.length }) }}
                  </span>
                  <span class="status-pill">{{ projectSearchRetrievalStatus }}</span>
                  <span v-if="projectSearchResults.meta.pending_index_jobs > 0" class="status-pill">
                    {{ t("tasks.projectSearch.pendingJobs", { count: projectSearchResults.meta.pending_index_jobs }) }}
                  </span>
                </template>
              </template>
              <template v-else>
                <span class="status-pill">{{ tasksStore.tasks.length }}</span>
                <span v-if="tasksStore.taskSummary" class="status-pill">
                  {{ t("tasks.summaryReady", { count: tasksStore.taskSummary.status_counts.ready }) }}
                </span>
                <span v-if="tasksStore.taskSummary" class="status-pill">
                  {{ t("tasks.summaryDone", { count: tasksStore.taskSummary.status_counts.done }) }}
                </span>
                <span v-if="tasksStore.taskSummary" class="status-pill">
                  {{ t("tasks.summaryReusable", { count: tasksStore.taskSummary.knowledge_counts.reusable }) }}
                </span>
              </template>
            </div>
          </div>
          <div class="workspace-filter-toolbar">
            <div class="workspace-filter-primary">
              <label class="compact-field compact-field-wide workspace-search-field">
                <span class="field-label">{{ t("tasks.projectSearch.label") }}</span>
                <div class="relative">
                  <Search
                    :size="14"
                    class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-[var(--text-muted)]"
                  />
                  <input
                    v-model="projectSearchInput"
                    class="control-input compact-control pl-9"
                    :placeholder="t('tasks.projectSearch.placeholder')"
                  />
                </div>
              </label>
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
              <div class="workspace-filter-actions">
                <button
                  class="secondary-action spotlight-surface"
                  type="button"
                  aria-controls="task-advanced-filters"
                  :aria-expanded="showAdvancedFilters"
                  @click="showAdvancedFilters = !showAdvancedFilters"
                >
                  <ListFilter :size="15" />
                  {{
                    activeTaskFilterCount > 0
                      ? t("tasks.advancedFiltersWithCount", { count: activeTaskFilterCount })
                      : t("tasks.advancedFilters")
                  }}
                </button>
                <button class="primary-action spotlight-surface" type="button" @click="openCreateTask($event)">
                  <Plus :size="15" />
                  {{ t("tasks.createTaskAction") }}
                </button>
              </div>
            </div>

            <div v-if="activeTaskFilterLabels.length > 0" class="filter-chip-row">
              <span v-for="label in activeTaskFilterLabels" :key="label" class="filter-chip filter-chip-active">
                {{ label }}
              </span>
              <button
                class="icon-button spotlight-surface"
                type="button"
                :aria-label="t('tasks.clearAdvancedFilters')"
                :title="t('tasks.clearAdvancedFilters')"
                @click="clearAdvancedTaskFilters"
              >
                <X :size="14" />
              </button>
            </div>

            <div v-if="showAdvancedFilters" id="task-advanced-filters" class="workspace-advanced-filter-panel">
              <label class="compact-field">
                <span class="field-label">{{ t("tasks.fields.taskKind") }}</span>
                <select v-model="selectedTaskKindFilter" class="control-select compact-control">
                  <option value="">{{ t("tasks.allTaskKinds") }}</option>
                  <option v-for="kind in taskKindOptions" :key="kind" :value="kind">
                    {{ t(`status.taskKind.${kind}`) }}
                  </option>
                </select>
              </label>
              <label class="compact-field">
                <span class="field-label">{{ t("tasks.fields.priority") }}</span>
                <select v-model="selectedPriorityFilter" class="control-select compact-control">
                  <option value="">{{ t("tasks.allPriorities") }}</option>
                  <option v-for="priority in taskPriorityOptions" :key="priority" :value="priority">
                    {{ t(`status.priority.${priority}`) }}
                  </option>
                </select>
              </label>
              <label class="compact-field">
                <span class="field-label">{{ t("tasks.fields.knowledgeStatus") }}</span>
                <select v-model="selectedKnowledgeStatusFilter" class="control-select compact-control">
                  <option value="">{{ t("tasks.allKnowledgeStatuses") }}</option>
                  <option v-for="status in knowledgeStatusOptions" :key="status" :value="status">
                    {{ t(`status.knowledge.${status}`) }}
                  </option>
                </select>
              </label>
              <label class="compact-field">
                <span class="field-label">{{ t("tasks.fields.taskCode") }}</span>
                <input
                  v-model="taskCodePrefixFilter"
                  class="control-input compact-control"
                  :placeholder="t('tasks.placeholders.taskCodePrefix')"
                />
              </label>
              <label class="compact-field">
                <span class="field-label">{{ t("tasks.fields.sortBy") }}</span>
                <select v-model="selectedSortBy" class="control-select compact-control">
                  <option v-for="sortBy in taskSortOptions" :key="sortBy" :value="sortBy">
                    {{ t(`tasks.sortBy.${sortBy}`) }}
                  </option>
                </select>
              </label>
              <label class="compact-field">
                <span class="field-label">{{ t("tasks.fields.sortOrder") }}</span>
                <select v-model="selectedSortOrder" class="control-select compact-control">
                  <option value="asc">{{ t("tasks.sortOrder.asc") }}</option>
                  <option value="desc">{{ t("tasks.sortOrder.desc") }}</option>
                </select>
              </label>
            </div>
          </div>
        </section>

        <section class="workspace-list-region">
          <div v-if="isProjectSearchActive" class="space-y-4">
            <div v-if="projectSearchLoading" class="empty-state">
              {{ t("search.loading") }}
            </div>
            <div v-else-if="projectSearchError" class="empty-state">
              {{ projectSearchError }}
            </div>
            <div
              v-else-if="
                !projectSearchResults ||
                (projectSearchResults.tasks.length === 0 && projectSearchResults.activities.length === 0)
              "
              class="empty-state"
            >
              {{ t("tasks.projectSearch.noResults") }}
            </div>
            <template v-else>
              <section class="space-y-2">
                <div class="flex items-center justify-between gap-3">
                  <p class="section-label">{{ t("search.tasks") }}</p>
                  <span class="text-xs text-[var(--text-muted)]">
                    {{ t("tasks.projectSearch.taskHits", { count: projectSearchResults.tasks.length }) }}
                  </span>
                </div>
                <div v-if="projectSearchResults.tasks.length === 0" class="empty-state">
                  {{ t("tasks.projectSearch.noTaskHits") }}
                </div>
                <div v-else class="workspace-row-list">
                  <button
                    v-for="task in projectSearchResults.tasks"
                    :key="task.task_id"
                    v-spotlight
                    class="list-row spotlight-surface"
                    :class="{ 'list-row-active': selectedTaskId === task.task_id }"
                    @click="selectTask(task.task_id)"
                  >
                    <SquareKanban :size="16" />
                    <div class="min-w-0 flex-1 space-y-2">
                      <div class="flex items-center justify-between gap-3">
                        <p class="truncate text-sm font-medium text-[var(--text-main)]">
                          {{ task.task_code ? `${task.task_code} · ${task.title}` : task.title }}
                        </p>
                        <div class="flex flex-wrap items-center justify-end gap-2">
                          <span v-if="task.task_code" class="status-pill">{{ task.task_code }}</span>
                          <span class="status-pill">{{ t(`status.taskKind.${task.task_kind}`) }}</span>
                          <span class="status-pill">{{ t(`status.task.${task.status}`) }}</span>
                          <span class="status-pill">{{ t(`search.source.${task.retrieval_source}`) }}</span>
                        </div>
                      </div>
                      <p class="truncate text-xs text-[var(--text-muted)]">{{ task.summary }}</p>
                      <p v-if="task.matched_fields.length > 0" class="truncate text-[11px] text-[var(--text-muted)]">
                        {{ t("tasks.projectSearch.matchedFields") }} {{ task.matched_fields.join(" · ") }}
                      </p>
                      <p v-if="task.evidence_snippet" class="truncate text-[11px] text-[var(--text-muted)]/90">
                        <span v-if="task.evidence_source" class="font-medium">
                          {{ localizeEvidenceSource(task.evidence_source, t) }}
                        </span>
                        <span v-if="task.evidence_source"> · </span>
                        <span v-html="renderHighlightedEvidence(task.evidence_snippet, projectSearchResults?.query)" />
                      </p>
                    </div>
                    <div class="list-row-meta">
                      <span>{{ t(`status.priority.${task.priority}`) }}</span>
                      <span>{{ t(`status.knowledge.${task.knowledge_status}`) }}</span>
                    </div>
                  </button>
                </div>
              </section>

              <section class="space-y-2">
                <div class="flex items-center justify-between gap-3">
                  <p class="section-label">{{ t("search.activity") }}</p>
                  <span class="text-xs text-[var(--text-muted)]">
                    {{ t("tasks.projectSearch.activityHits", { count: projectSearchResults.activities.length }) }}
                  </span>
                </div>
                <div v-if="projectSearchResults.activities.length === 0" class="empty-state">
                  {{ t("tasks.projectSearch.noActivityHits") }}
                </div>
                <div v-else class="workspace-row-list">
                  <button
                    v-for="item in projectSearchResults.activities"
                    :key="item.activity_id"
                    v-spotlight
                    class="list-row spotlight-surface"
                    :class="{ 'list-row-active': selectedTaskId === item.task_id }"
                    @click="selectTask(item.task_id)"
                  >
                    <Search :size="16" />
                    <div class="min-w-0 flex-1 space-y-2">
                      <div class="flex items-center justify-between gap-3">
                        <p class="truncate text-sm font-medium text-[var(--text-main)]">
                          {{ t(`activityKind.${item.kind}`) }}
                        </p>
                        <div class="flex flex-wrap items-center justify-end gap-2">
                          <span class="status-pill">{{ t(`activityKind.${item.kind}`) }}</span>
                        </div>
                      </div>
                      <p class="truncate text-xs text-[var(--text-muted)]">{{ item.summary }}</p>
                      <p v-if="item.evidence_snippet" class="truncate text-[11px] text-[var(--text-muted)]/90">
                        <span v-if="item.evidence_source" class="font-medium">
                          {{ localizeEvidenceSource(item.evidence_source, t) }}
                        </span>
                        <span v-if="item.evidence_source"> · </span>
                        <span v-html="renderHighlightedEvidence(item.evidence_snippet, projectSearchResults?.query)" />
                      </p>
                      <p class="truncate text-[11px] text-[var(--text-muted)]">
                        {{ projectSearchTaskMap.get(item.task_id)?.title || item.task_id }}
                      </p>
                    </div>
                  </button>
                </div>
              </section>
            </template>
          </div>
          <div v-else-if="tasksStore.tasks.length === 0" class="empty-state">
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
                  <p class="truncate text-sm font-medium text-[var(--text-main)]">
                    {{ task.task_code ? `${task.task_code} · ${task.title}` : task.title }}
                  </p>
                  <div class="flex flex-wrap items-center justify-end gap-2">
                    <span class="status-pill">{{ t(`status.taskKind.${task.task_kind}`) }}</span>
                    <span class="status-pill">{{ t(`status.knowledge.${task.knowledge_status}`) }}</span>
                    <span class="status-pill">{{ t(`status.task.${task.status}`) }}</span>
                  </div>
                </div>
                <p class="truncate text-xs text-[var(--text-muted)]">
                  {{ task.latest_note_summary || task.summary || task.task_context_digest }}
                </p>
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
            <label class="form-field">
              <span class="field-label">{{ t("tasks.fields.taskCode") }}</span>
              <input
                v-model="createTaskForm.task_code"
                class="quiet-control-input"
                :placeholder="t('tasks.placeholders.taskCode')"
              />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("tasks.fields.taskKind") }}</span>
              <select v-model="createTaskForm.kind" class="quiet-control-select">
                <option v-for="kind in taskKindOptions" :key="kind" :value="kind">
                  {{ t(`status.taskKind.${kind}`) }}
                </option>
              </select>
            </label>
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
              <div class="flex flex-wrap items-center gap-2">
                <span v-if="selectedTask.task_code" class="status-pill">{{ selectedTask.task_code }}</span>
                <span class="status-pill">{{ t(`status.taskKind.${selectedTask.task_kind}`) }}</span>
                <span class="status-pill">{{ t(`status.knowledge.${selectedTask.knowledge_status}`) }}</span>
                <span class="status-pill">{{ t(`status.task.${selectedTask.status}`) }}</span>
                <span class="status-pill">{{ t(`status.priority.${selectedTask.priority}`) }}</span>
                <span class="status-pill">
                  {{ selectedTask.ready_to_start ? t("tasks.readyToStart") : t("tasks.notReadyToStart") }}
                </span>
                <span class="status-pill">{{ t("tasks.childCount", { count: selectedTask.child_count }) }}</span>
                <span class="status-pill">
                  {{ t("tasks.blockerCount", { count: selectedTask.open_blocker_count }) }}
                </span>
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
            <div class="grid gap-5 2xl:grid-cols-[minmax(360px,0.62fr)_minmax(320px,0.38fr)]">
              <section class="panel-section">
                <p class="section-label">{{ t("tasks.editTask") }}</p>
                <div class="space-y-3">
                  <input
                    v-model="taskForm.task_code"
                    class="control-input"
                    :placeholder="t('tasks.placeholders.taskCode')"
                  />
                  <select v-model="taskForm.kind" class="control-select">
                    <option v-for="kind in taskKindOptions" :key="kind" :value="kind">
                      {{ t(`status.taskKind.${kind}`) }}
                    </option>
                  </select>
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
            <section class="grid gap-5 2xl:grid-cols-2">
              <article class="panel-section space-y-4">
                <div class="flex items-center justify-between gap-3">
                  <p class="section-label">{{ t("tasks.relationships.parentChild") }}</p>
                  <span class="status-pill">{{ t("tasks.childCount", { count: selectedTask.child_count }) }}</span>
                </div>
                <div class="space-y-3">
                  <div>
                    <p class="text-xs uppercase tracking-[0.2em] text-[var(--text-muted)]">{{ t("tasks.parentTask") }}</p>
                    <button
                      v-if="tasksStore.parentTask"
                      class="secondary-action mt-2 spotlight-surface"
                      type="button"
                      @click="jumpToLinkedTask(tasksStore.parentTask.task_id)"
                    >
                      {{ tasksStore.parentTask.title }}
                    </button>
                    <p v-else class="mt-2 text-sm text-[var(--text-muted)]">{{ t("tasks.noParentTask") }}</p>
                  </div>
                  <div>
                    <p class="text-xs uppercase tracking-[0.2em] text-[var(--text-muted)]">{{ t("tasks.childTasks") }}</p>
                    <div v-if="tasksStore.childTasks.length > 0" class="mt-2 space-y-2">
                      <div
                        v-for="child in tasksStore.childTasks"
                        :key="child.relation_id"
                        class="flex flex-wrap items-center justify-between gap-2 rounded-2xl border border-[var(--border-color)] px-3 py-3"
                      >
                        <button class="secondary-action spotlight-surface" type="button" @click="jumpToLinkedTask(child.task_id)">
                          {{ child.title }}
                        </button>
                        <button class="secondary-action spotlight-surface" type="button" @click="handleDetachChild(child.task_id)">
                          {{ t("tasks.detachChildAction") }}
                        </button>
                      </div>
                    </div>
                    <p v-else class="mt-2 text-sm text-[var(--text-muted)]">{{ t("tasks.noChildTasks") }}</p>
                  </div>
                  <label class="form-field">
                    <span class="field-label">{{ t("tasks.fields.childTaskId") }}</span>
                    <input
                      v-model="relationForm.childTaskId"
                      class="control-input"
                      :placeholder="t('tasks.placeholders.childTaskId')"
                    />
                  </label>
                  <button class="primary-action spotlight-surface" type="button" @click="submitAttachChild">
                    <Plus :size="15" />
                    {{ t("tasks.attachChildAction") }}
                  </button>
                </div>
              </article>

              <article class="panel-section space-y-4">
                <div class="flex items-center justify-between gap-3">
                  <p class="section-label">{{ t("tasks.relationships.blockers") }}</p>
                  <span class="status-pill">
                    {{ t("tasks.blockerCount", { count: selectedTask.open_blocker_count }) }}
                  </span>
                </div>
                <div class="space-y-3">
                  <div>
                    <p class="text-xs uppercase tracking-[0.2em] text-[var(--text-muted)]">{{ t("tasks.blockedByTasks") }}</p>
                    <div v-if="tasksStore.blockedByTasks.length > 0" class="mt-2 space-y-2">
                      <div
                        v-for="item in tasksStore.blockedByTasks"
                        :key="item.relation_id"
                        class="flex flex-wrap items-center justify-between gap-2 rounded-2xl border border-[var(--border-color)] px-3 py-3"
                      >
                        <button class="secondary-action spotlight-surface" type="button" @click="jumpToLinkedTask(item.task_id)">
                          {{ item.title }}
                        </button>
                        <button
                          class="secondary-action spotlight-surface"
                          type="button"
                          @click="handleResolveBlocker(item.relation_id)"
                        >
                          {{ t("tasks.resolveBlockerAction") }}
                        </button>
                      </div>
                    </div>
                    <p v-else class="mt-2 text-sm text-[var(--text-muted)]">{{ t("tasks.noBlockers") }}</p>
                  </div>
                  <div>
                    <p class="text-xs uppercase tracking-[0.2em] text-[var(--text-muted)]">{{ t("tasks.blockingTasks") }}</p>
                    <div v-if="tasksStore.blockingTasks.length > 0" class="mt-2 space-y-2">
                      <button
                        v-for="item in tasksStore.blockingTasks"
                        :key="item.relation_id"
                        class="secondary-action spotlight-surface"
                        type="button"
                        @click="jumpToLinkedTask(item.task_id)"
                      >
                        {{ item.title }}
                      </button>
                    </div>
                    <p v-else class="mt-2 text-sm text-[var(--text-muted)]">{{ t("tasks.noBlockingTasks") }}</p>
                  </div>
                  <label class="form-field">
                    <span class="field-label">{{ t("tasks.fields.blockerTaskId") }}</span>
                    <input
                      v-model="relationForm.blockerTaskId"
                      class="control-input"
                      :placeholder="t('tasks.placeholders.blockerTaskId')"
                    />
                  </label>
                  <button class="primary-action spotlight-surface" type="button" @click="submitAddBlocker">
                    <Plus :size="15" />
                    {{ t("tasks.addBlockerAction") }}
                  </button>
                </div>
              </article>
            </section>
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
              <select v-model="noteForm.note_kind" class="control-select">
                <option v-for="noteKind in noteKindOptions" :key="noteKind" :value="noteKind">
                  {{ t(`status.noteKind.${noteKind}`) }}
                </option>
              </select>
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
                  <div class="flex items-center gap-2">
                    <span class="status-pill">{{ t(`activityKind.${note.kind}`) }}</span>
                    <span class="status-pill">{{ t(`status.noteKind.${note.note_kind || 'finding'}`) }}</span>
                  </div>
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
