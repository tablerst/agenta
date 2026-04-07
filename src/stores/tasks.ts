import { ref } from "vue";
import { defineStore } from "pinia";

import { desktopBridge } from "../lib/desktop";
import type { Attachment, Task, TaskActivity } from "../lib/types";

export const useTasksStore = defineStore("tasks", () => {
  const tasks = ref<Task[]>([]);
  const currentTask = ref<Task | null>(null);
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
      const [taskEnvelope, notesEnvelope, attachmentEnvelope, activityEnvelope] =
        await Promise.all([
          desktopBridge.task({ action: "get", task }),
          desktopBridge.note({ action: "list", task }),
          desktopBridge.attachment({ action: "list", task }),
          desktopBridge.task({ action: "activity_list", task }),
        ]);

      currentTask.value = taskEnvelope.result as Task;
      notes.value = notesEnvelope.result as TaskActivity[];
      attachments.value = attachmentEnvelope.result as Attachment[];
      activities.value = activityEnvelope.result as TaskActivity[];
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

  return {
    activities,
    attachments,
    createAttachment,
    createNote,
    createTask,
    currentTask,
    loadTask,
    loadTaskDetail,
    loadTasks,
    loadingDetail,
    loadingTasks,
    notes,
    tasks,
    updateTask,
  };
});
