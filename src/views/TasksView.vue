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
} from "@lucide/vue";
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

import JsonBlock from "../components/JsonBlock.vue";
import { coerceString, formatDateTime } from "../lib/format";
import { DesktopBridgeError, desktopBridge } from "../lib/desktop";
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

const detailTab = ref<"overview" | "notes" | "attachments" | "activity">("overview");

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
  path: "",
  summary: "",
  kind: "artifact",
});

const selectedProjectKey = computed(() => coerceString(route.query.project as string | undefined));
const selectedVersionId = computed(() => coerceString(route.query.version as string | undefined));
const selectedTaskId = computed(() => coerceString(route.query.task as string | undefined));
const selectedStatus = computed(() => coerceString(route.query.status as string | undefined));

const selectedProject = computed(() =>
  projectsStore.projects.find(
    (item) => item.project_id === selectedProjectKey.value || item.slug === selectedProjectKey.value,
  ) ?? null,
);

const selectedTask = computed(() =>
  tasksStore.tasks.find((item) => item.task_id === selectedTaskId.value) ?? tasksStore.currentTask,
);

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
  () => selectedProject.value?.slug,
  async (project) => {
    if (project) {
      await projectsStore.loadVersions(project);
    } else {
      projectsStore.versions = [];
    }
  },
  { immediate: true },
);

watch(
  () => [selectedProjectKey.value, selectedVersionId.value, selectedStatus.value] as const,
  async ([project, version, status]) => {
    await tasksStore.loadTasks({
      project: project || undefined,
      version: version || undefined,
      status: status || undefined,
    });
  },
  { immediate: true },
);

watch(
  selectedTaskId,
  async (task) => {
    if (task) {
      await tasksStore.loadTaskDetail(task);
    }
  },
  { immediate: true },
);

onMounted(async () => {
  await Promise.allSettled([projectsStore.loadProjects(), approvalsStore.refreshPendingCount()]);
});

async function updateQuery(values: Record<string, string | undefined>) {
  const query = {
    ...route.query,
    ...values,
  };
  Object.keys(query).forEach((key) => {
    if (!query[key]) {
      delete query[key];
    }
  });
  await router.push({ path: "/tasks", query });
}

async function submitCreateTask() {
  if (!selectedProject.value) {
    shell.pushNotice("error", "Select a project before creating a task.");
    return;
  }

  try {
    const created = await tasksStore.createTask({
      project: selectedProject.value.slug,
      version: selectedVersionId.value || undefined,
      ...createTaskForm,
      summary: createTaskForm.summary || null,
      description: createTaskForm.description || null,
    });
    await tasksStore.loadTasks({
      project: selectedProject.value.slug,
      version: selectedVersionId.value || undefined,
      status: selectedStatus.value || undefined,
    });
    await approvalsStore.refreshPendingCount();
    shell.pushNotice("success", "Task submission sent.");
    createTaskForm.title = "";
    createTaskForm.summary = "";
    createTaskForm.description = "";
    await updateQuery({ task: created.task_id });
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", (error as Error).message);
    await approvalsStore.refreshPendingCount();
  }
}

async function submitTaskUpdate() {
  if (!selectedTask.value) {
    return;
  }

  try {
    await tasksStore.updateTask(selectedTask.value.task_id, {
      title: taskForm.title,
      summary: taskForm.summary || null,
      description: taskForm.description || null,
      status: taskForm.status,
      priority: taskForm.priority,
      updated_by: taskForm.updated_by,
      version: selectedVersionId.value || selectedTask.value.version_id || undefined,
    });
    await tasksStore.loadTasks({
      project: selectedProjectKey.value || undefined,
      version: selectedVersionId.value || undefined,
      status: selectedStatus.value || undefined,
    });
    await approvalsStore.refreshPendingCount();
    shell.pushNotice("success", "Task updated.");
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", (error as Error).message);
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
    shell.pushNotice("success", "Note added.");
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", (error as Error).message);
  }
}

async function submitAttachment() {
  if (!selectedTask.value) {
    return;
  }

  try {
    await tasksStore.createAttachment(selectedTask.value.task_id, {
      path: attachmentForm.path,
      summary: attachmentForm.summary || null,
      kind: attachmentForm.kind,
      created_by: "desktop",
    });
    attachmentForm.path = "";
    attachmentForm.summary = "";
    shell.pushNotice("success", "Attachment added.");
  } catch (error) {
    if (await jumpToQueuedApproval(error)) {
      return;
    }
    shell.pushNotice("error", (error as Error).message);
  }
}

async function openAttachment(path: string) {
  try {
    await desktopBridge.revealAttachment(path);
  } catch (error) {
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
          <p class="section-label">Tasks</p>
          <h1 class="text-lg font-semibold text-[var(--text-main)]">Task workspace</h1>
        </div>
        <span class="status-pill">{{ tasksStore.tasks.length }}</span>
      </div>

      <section class="panel-section mb-4">
        <div class="mb-3 flex items-center gap-2 text-[var(--text-muted)]">
          <ListFilter :size="14" />
          <p class="section-label !mb-0">Filters</p>
        </div>
        <div class="space-y-3">
          <select
            class="control-select"
            :value="selectedProject?.slug || ''"
            @change="updateQuery({ project: ($event.target as HTMLSelectElement).value || undefined, version: undefined, task: undefined })"
          >
            <option value="">All projects</option>
            <option v-for="project in projectsStore.projects" :key="project.project_id" :value="project.slug">
              {{ project.name }}
            </option>
          </select>
          <select
            class="control-select"
            :value="selectedVersionId"
            @change="updateQuery({ version: ($event.target as HTMLSelectElement).value || undefined, task: undefined })"
          >
            <option value="">All versions</option>
            <option v-for="version in projectsStore.versions" :key="version.version_id" :value="version.version_id">
              {{ version.name }}
            </option>
          </select>
          <select
            class="control-select"
            :value="selectedStatus"
            @change="updateQuery({ status: ($event.target as HTMLSelectElement).value || undefined })"
          >
            <option value="">All statuses</option>
            <option value="draft">draft</option>
            <option value="ready">ready</option>
            <option value="in_progress">in_progress</option>
            <option value="blocked">blocked</option>
            <option value="done">done</option>
            <option value="cancelled">cancelled</option>
          </select>
        </div>
      </section>

      <section class="panel-section mb-4">
        <p class="section-label">Create Task</p>
        <div class="space-y-3">
          <input v-model="createTaskForm.title" class="control-input" placeholder="Task title" />
          <input v-model="createTaskForm.summary" class="control-input" placeholder="Summary" />
          <textarea v-model="createTaskForm.description" class="control-textarea" placeholder="Execution context" />
          <div class="grid gap-3 md:grid-cols-2">
            <select v-model="createTaskForm.status" class="control-select">
              <option value="draft">draft</option>
              <option value="ready">ready</option>
              <option value="in_progress">in_progress</option>
              <option value="blocked">blocked</option>
              <option value="done">done</option>
              <option value="cancelled">cancelled</option>
            </select>
            <select v-model="createTaskForm.priority" class="control-select">
              <option value="low">low</option>
              <option value="normal">normal</option>
              <option value="high">high</option>
              <option value="critical">critical</option>
            </select>
          </div>
          <button class="primary-action spotlight-surface" @click="submitCreateTask">
            <Plus :size="15" />
            Create task
          </button>
        </div>
      </section>

      <div v-if="tasksStore.tasks.length === 0" class="empty-state">No tasks match the current filters.</div>
      <div v-else>
        <button
          v-for="task in tasksStore.tasks"
          :key="task.task_id"
          v-spotlight
          class="list-row spotlight-surface"
          :class="{ 'list-row-active': selectedTask?.task_id === task.task_id }"
          @click="updateQuery({ task: task.task_id })"
        >
          <SquareKanban :size="16" />
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium text-[var(--text-main)]">{{ task.title }}</p>
            <p class="truncate text-xs text-[var(--text-muted)]">{{ task.summary || task.task_context_digest }}</p>
          </div>
          <div class="flex flex-col items-end gap-1">
            <span class="status-pill">{{ task.status }}</span>
            <span class="text-[11px] text-[var(--text-muted)]">{{ task.priority }}</span>
          </div>
        </button>
      </div>
    </aside>

    <div class="page-detail">
      <div v-if="selectedTask" class="space-y-5">
        <section class="glass-panel p-5">
          <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
            <div>
              <p class="section-label">Task Detail</p>
              <h2 class="text-2xl font-semibold text-[var(--text-main)]">{{ selectedTask.title }}</h2>
              <p class="mt-2 max-w-3xl text-sm leading-6 text-[var(--text-muted)]">
                {{ selectedTask.summary || "No task summary yet." }}
              </p>
            </div>
            <div class="flex items-center gap-2">
              <span class="status-pill">{{ selectedTask.status }}</span>
              <span class="status-pill">{{ selectedTask.priority }}</span>
            </div>
          </div>

          <div class="mb-4 flex flex-wrap gap-2">
            <button
              v-for="tab in ['overview', 'notes', 'attachments', 'activity']"
              :key="tab"
              class="secondary-action spotlight-surface"
              :class="{ 'border-white/15 bg-white/6': detailTab === tab }"
              @click="detailTab = tab as typeof detailTab.value"
            >
              {{ tab }}
            </button>
          </div>

          <div v-if="detailTab === 'overview'" class="grid gap-5 xl:grid-cols-[minmax(0,0.68fr)_minmax(320px,0.32fr)]">
            <section class="panel-section">
              <p class="section-label">Edit Task</p>
              <div class="space-y-3">
                <input v-model="taskForm.title" class="control-input" />
                <input v-model="taskForm.summary" class="control-input" />
                <textarea v-model="taskForm.description" class="control-textarea" />
                <div class="grid gap-3 md:grid-cols-2">
                  <select v-model="taskForm.status" class="control-select">
                    <option value="draft">draft</option>
                    <option value="ready">ready</option>
                    <option value="in_progress">in_progress</option>
                    <option value="blocked">blocked</option>
                    <option value="done">done</option>
                    <option value="cancelled">cancelled</option>
                  </select>
                  <select v-model="taskForm.priority" class="control-select">
                    <option value="low">low</option>
                    <option value="normal">normal</option>
                    <option value="high">high</option>
                    <option value="critical">critical</option>
                  </select>
                </div>
                <button class="primary-action spotlight-surface" @click="submitTaskUpdate">
                  <BadgeCheck :size="15" />
                  Save task
                </button>
              </div>
            </section>

            <section class="panel-section">
              <p class="section-label">Context Digest</p>
              <JsonBlock :value="selectedTask" />
            </section>
          </div>

          <div v-else-if="detailTab === 'notes'" class="space-y-4">
            <section class="panel-section">
              <p class="section-label">Add Note</p>
              <textarea v-model="noteForm.content" class="control-textarea" placeholder="Captured human note or MCP transcript summary" />
              <button class="primary-action mt-3 spotlight-surface" @click="submitNote">
                <FileText :size="15" />
                Add note
              </button>
            </section>
            <section class="space-y-3">
              <article v-for="note in tasksStore.notes" :key="note.activity_id" class="panel-section">
                <div class="mb-2 flex items-center justify-between">
                  <span class="status-pill">{{ note.kind }}</span>
                  <span class="text-xs text-[var(--text-muted)]">{{ formatDateTime(note.created_at) }}</span>
                </div>
                <p class="text-sm leading-6 text-[var(--text-main)]">{{ note.content }}</p>
              </article>
            </section>
          </div>

          <div v-else-if="detailTab === 'attachments'" class="space-y-4">
            <section class="panel-section">
              <p class="section-label">Add Attachment</p>
              <div class="space-y-3">
                <input v-model="attachmentForm.path" class="control-input" placeholder="Absolute source path" />
                <input v-model="attachmentForm.summary" class="control-input" placeholder="Summary" />
                <select v-model="attachmentForm.kind" class="control-select">
                  <option value="artifact">artifact</option>
                  <option value="log">log</option>
                  <option value="report">report</option>
                  <option value="image">image</option>
                  <option value="screenshot">screenshot</option>
                  <option value="patch">patch</option>
                  <option value="other">other</option>
                </select>
                <button class="primary-action spotlight-surface" @click="submitAttachment">
                  <Paperclip :size="15" />
                  Add attachment
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
                  <span class="status-pill">{{ attachment.kind }}</span>
                  <button class="secondary-action spotlight-surface" @click="openAttachment(attachment.original_path)">
                    <Check :size="15" />
                    Open path
                  </button>
                </div>
              </div>
              <div class="mt-3 text-xs text-[var(--text-muted)]">
                {{ attachment.original_path }}
              </div>
            </article>
          </div>

          <div v-else class="space-y-3">
            <article v-for="item in tasksStore.activities" :key="item.activity_id" class="panel-section">
              <div class="mb-2 flex items-center justify-between">
                <span class="status-pill">{{ item.kind }}</span>
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
          </div>
        </section>
      </div>

      <div v-else class="empty-state">
        Select a task to inspect notes, attachments, and activity records.
      </div>
    </div>
  </section>
</template>
