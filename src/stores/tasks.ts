import { ref } from "vue";
import { defineStore } from "pinia";

import { desktopBridge } from "../lib/desktop";
import type {
  Attachment,
  Task,
  TaskActivity,
  TaskContextPayload,
  TaskLink,
  TaskListPageInfo,
  TaskListPayload,
  TaskListSummary,
} from "../lib/types";

function extractTaskRecord(value: unknown): Task {
  if (value && typeof value === "object" && "task" in value) {
    return (value as { task: Task }).task;
  }
  return value as Task;
}

function ensureArray<T>(value: T[] | null | undefined): T[] {
  return Array.isArray(value) ? value : [];
}

export const useTasksStore = defineStore("tasks", () => {
  const tasks = ref<Task[]>([]);
  const taskSummary = ref<TaskListSummary | null>(null);
  const taskPage = ref<TaskListPageInfo | null>(null);
  const currentTask = ref<Task | null>(null);
  const parentTask = ref<TaskLink | null>(null);
  const childTasks = ref<TaskLink[]>([]);
  const blockedByTasks = ref<TaskLink[]>([]);
  const blockingTasks = ref<TaskLink[]>([]);
  const notes = ref<TaskActivity[]>([]);
  const attachments = ref<Attachment[]>([]);
  const activities = ref<TaskActivity[]>([]);
  const loadingTasks = ref(false);
  const loadingDetail = ref(false);

  async function loadTasks(filters: Record<string, unknown>) {
    loadingTasks.value = true;
    try {
      const envelope = await desktopBridge.task({ action: "list", ...filters });
      const payload = envelope.result as TaskListPayload;
      tasks.value = payload.tasks;
      taskSummary.value = payload.summary;
      taskPage.value = payload.page;
      return tasks.value;
    } finally {
      loadingTasks.value = false;
    }
  }

  async function loadTask(task: string) {
    const envelope = await desktopBridge.task({ action: "get", task });
    currentTask.value = extractTaskRecord(envelope.result);
    return currentTask.value;
  }

  async function loadTaskDetail(task: string) {
    loadingDetail.value = true;
    try {
      const envelope = await desktopBridge.task({ action: "get_context", task });
      const context = envelope.result as Partial<TaskContextPayload> & { task: Task };

      currentTask.value = context.task;
      parentTask.value = context.parent ?? null;
      childTasks.value = ensureArray(context.children);
      blockedByTasks.value = ensureArray(context.blocked_by);
      blockingTasks.value = ensureArray(context.blocking);
      notes.value = ensureArray(context.notes);
      attachments.value = ensureArray(context.attachments);
      activities.value = ensureArray(context.recent_activities);
    } finally {
      loadingDetail.value = false;
    }
  }

  async function createTask(payload: Record<string, unknown>) {
    const envelope = await desktopBridge.task({ action: "create", ...payload });
    return extractTaskRecord(envelope.result);
  }

  async function updateTask(task: string, payload: Record<string, unknown>) {
    const envelope = await desktopBridge.task({ action: "update", task, ...payload });
    currentTask.value = extractTaskRecord(envelope.result);
    return currentTask.value;
  }

  async function createChildTask(payload: Record<string, unknown>) {
    const envelope = await desktopBridge.task({ action: "create_child", ...payload });
    return extractTaskRecord(envelope.result);
  }

  async function attachChild(parent: string, child: string, updated_by = "desktop") {
    const envelope = await desktopBridge.task({
      action: "attach_child",
      child,
      parent,
      updated_by,
    });
    if (currentTask.value) {
      await loadTaskDetail(currentTask.value.task_id);
    }
    return envelope.result;
  }

  async function detachChild(parent: string, child: string, updated_by = "desktop") {
    const envelope = await desktopBridge.task({
      action: "detach_child",
      child,
      parent,
      updated_by,
    });
    if (currentTask.value) {
      await loadTaskDetail(currentTask.value.task_id);
    }
    return envelope.result;
  }

  async function addBlocker(task: string, blocker: string, updated_by = "desktop") {
    const envelope = await desktopBridge.task({
      action: "add_blocker",
      blocker,
      task,
      updated_by,
    });
    await loadTaskDetail(task);
    return envelope.result;
  }

  async function resolveBlocker(task: string, payload: Record<string, unknown>) {
    const envelope = await desktopBridge.task({
      action: "resolve_blocker",
      task,
      ...payload,
    });
    await loadTaskDetail(task);
    return envelope.result;
  }

  async function createNote(task: string, payload: Record<string, unknown>) {
    const envelope = await desktopBridge.note({ action: "create", task, ...payload });
    await loadTaskDetail(task);
    return envelope.result as TaskActivity;
  }

  async function createAttachment(task: string, payload: Record<string, unknown>) {
    const envelope = await desktopBridge.attachment({
      action: "create",
      task,
      ...payload,
    });
    await loadTaskDetail(task);
    return envelope.result as Attachment;
  }

  function clearTaskDetail() {
    currentTask.value = null;
    taskSummary.value = null;
    taskPage.value = null;
    parentTask.value = null;
    childTasks.value = [];
    blockedByTasks.value = [];
    blockingTasks.value = [];
    notes.value = [];
    attachments.value = [];
    activities.value = [];
  }

  return {
    addBlocker,
    activities,
    attachChild,
    attachments,
    blockedByTasks,
    blockingTasks,
    clearTaskDetail,
    createAttachment,
    createChildTask,
    createNote,
    createTask,
    currentTask,
    childTasks,
    detachChild,
    loadTask,
    loadTaskDetail,
    loadTasks,
    loadingDetail,
    loadingTasks,
    notes,
    parentTask,
    resolveBlocker,
    tasks,
    taskPage,
    taskSummary,
    updateTask,
  };
});
