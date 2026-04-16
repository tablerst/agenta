import { ref } from "vue";
import { defineStore } from "pinia";

import { desktopBridge } from "../lib/desktop";
import type { Attachment, Task, TaskActivity, TaskContextPayload, TaskLink } from "../lib/types";

export const useTasksStore = defineStore("tasks", () => {
  const tasks = ref<Task[]>([]);
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
      tasks.value = envelope.result as Task[];
      return tasks.value;
    } finally {
      loadingTasks.value = false;
    }
  }

  async function loadTask(task: string) {
    const envelope = await desktopBridge.task({ action: "get", task });
    currentTask.value = envelope.result as Task;
    return currentTask.value;
  }

  async function loadTaskDetail(task: string) {
    loadingDetail.value = true;
    try {
      const envelope = await desktopBridge.task({ action: "get_context", task });
      const context = envelope.result as TaskContextPayload;

      currentTask.value = context.task;
      parentTask.value = context.parent;
      childTasks.value = context.children;
      blockedByTasks.value = context.blocked_by;
      blockingTasks.value = context.blocking;
      notes.value = context.notes;
      attachments.value = context.attachments;
      activities.value = context.recent_activities;
    } finally {
      loadingDetail.value = false;
    }
  }

  async function createTask(payload: Record<string, unknown>) {
    const envelope = await desktopBridge.task({ action: "create", ...payload });
    return envelope.result as Task;
  }

  async function updateTask(task: string, payload: Record<string, unknown>) {
    const envelope = await desktopBridge.task({ action: "update", task, ...payload });
    currentTask.value = envelope.result as Task;
    return currentTask.value;
  }

  async function createChildTask(payload: Record<string, unknown>) {
    const envelope = await desktopBridge.task({ action: "create_child", ...payload });
    return envelope.result as Task;
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
    updateTask,
  };
});
