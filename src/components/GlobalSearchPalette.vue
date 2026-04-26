<script setup lang="ts">
import { Search, SquareKanban } from "@lucide/vue";
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import AppSelect from "./AppSelect.vue";
import { desktopBridge } from "../lib/desktop";
import { knowledgeStatusOptions, taskKindOptions, taskPriorityOptions, type TaskDetailTab } from "../lib/options";
import { buildProjectWorkspacePath, resolveProjectSlug } from "../lib/projectWorkspace";
import { localizeEvidenceSource, renderHighlightedEvidence } from "../lib/searchEvidence";
import type {
  GlobalSearchFilters,
  KnowledgeStatus,
  SearchActivityHit,
  SearchTaskHit,
  Task,
  TaskKind,
  TaskPriority,
} from "../lib/types";
import { useProjectsStore } from "../stores/projects";
import { useSearchStore } from "../stores/search";
import { useShellStore } from "../stores/shell";

const shell = useShellStore();
const search = useSearchStore();
const projectsStore = useProjectsStore();
const router = useRouter();
const { t } = useI18n({ useScope: "global" });
const inputValue = ref(search.query);
const inputEl = ref<HTMLInputElement | null>(null);
const activeIndex = ref(0);
const selectedTaskKind = ref("");
const selectedPriority = ref("");
const selectedKnowledgeStatus = ref("");
let timer: number | undefined;

const taskKindFilterOptions = computed(() => [
  { value: "", label: t("tasks.allTaskKinds") },
  ...taskKindOptions.map((kind) => ({ value: kind, label: t(`status.taskKind.${kind}`) })),
]);

const taskPriorityFilterOptions = computed(() => [
  { value: "", label: t("tasks.allPriorities") },
  ...taskPriorityOptions.map((priority) => ({ value: priority, label: t(`status.priority.${priority}`) })),
]);

const knowledgeStatusFilterOptions = computed(() => [
  { value: "", label: t("tasks.allKnowledgeStatuses") },
  ...knowledgeStatusOptions.map((status) => ({ value: status, label: t(`status.knowledge.${status}`) })),
]);

const activeFilters = computed<GlobalSearchFilters>(() => ({
  knowledge_status: selectedKnowledgeStatus.value ? (selectedKnowledgeStatus.value as KnowledgeStatus) : undefined,
  priority: selectedPriority.value ? (selectedPriority.value as TaskPriority) : undefined,
  task_kind: selectedTaskKind.value ? (selectedTaskKind.value as TaskKind) : undefined,
}));

const activeFilterLabels = computed(() => {
  const labels: string[] = [];
  if (selectedTaskKind.value) {
    labels.push(t(`status.taskKind.${selectedTaskKind.value}`));
  }
  if (selectedPriority.value) {
    labels.push(t(`status.priority.${selectedPriority.value}`));
  }
  if (selectedKnowledgeStatus.value) {
    labels.push(t(`status.knowledge.${selectedKnowledgeStatus.value}`));
  }
  return labels;
});

const flatResults = computed(() => {
  if (!search.results) {
    return [];
  }

  return [
    ...search.results.tasks.map((item) => {
      const tab = taskHitTab(item);
      return {
      key: `task:${item.task_id}`,
      kind: "task" as const,
      taskId: item.task_id,
      activityId: tab === "activity" ? item.evidence_activity_id : null,
      attachmentId: tab === "attachments" ? item.evidence_attachment_id : null,
      tab,
    };
    }),
    ...search.results.activities.map((item) => ({
      key: `activity:${item.activity_id}`,
      kind: "activity" as const,
      taskId: item.task_id,
      activityId: item.activity_id,
      attachmentId: item.evidence_attachment_id,
      tab: "activity" as const,
    })),
  ];
});

const activeResultId = computed(() => {
  const active = flatResults.value[activeIndex.value];
  return active ? globalSearchOptionId(active.key) : undefined;
});

const retrievalStatus = computed(() => {
  if (!search.results) {
    return "";
  }

  if (search.results.meta.retrieval_mode === "structured_only") {
    return t("search.retrieval.structuredOnly");
  }
  if (search.results.meta.vector_status === "indexing") {
    return t("search.retrieval.indexing");
  }
  if (search.results.meta.retrieval_mode === "hybrid") {
    return t("search.retrieval.hybrid");
  }
  return t("search.retrieval.lexicalFallback");
});

watch(
  () => shell.searchOpen,
  async (isOpen) => {
    if (isOpen) {
      activeIndex.value = 0;
      await nextTick();
      inputEl.value?.focus();
      return;
    }

    search.clear();
    inputValue.value = "";
    activeIndex.value = 0;
    await nextTick();
    shell.restoreSearchFocus();
  },
);

watch(inputValue, (value) => {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    void search.runSearch(value, activeFilters.value);
  }, 160);
});

watch(activeFilters, () => {
  if (!inputValue.value.trim()) {
    return;
  }
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    void search.runSearch(inputValue.value, activeFilters.value);
  }, 80);
});

watch(flatResults, (results) => {
  if (results.length === 0) {
    activeIndex.value = 0;
    return;
  }
  activeIndex.value = Math.min(activeIndex.value, results.length - 1);
});

function close() {
  shell.closeSearch();
}

function clearFilters() {
  selectedTaskKind.value = "";
  selectedPriority.value = "";
  selectedKnowledgeStatus.value = "";
}

function setActive(index: number) {
  activeIndex.value = index;
}

function moveActive(step: number) {
  if (flatResults.value.length === 0) {
    return;
  }

  const total = flatResults.value.length;
  activeIndex.value = (activeIndex.value + step + total) % total;
}

function isActive(key: string) {
  return flatResults.value[activeIndex.value]?.key === key;
}

function globalSearchOptionId(key: string) {
  return `global-search-${key.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
}

function taskHitTab(task: SearchTaskHit): TaskDetailTab | undefined {
  if (task.evidence_attachment_id) {
    return "attachments";
  }
  if (task.evidence_activity_id || task.evidence_chunk_id) {
    return "activity";
  }
  return undefined;
}

function activityHitTab(_activity: SearchActivityHit): TaskDetailTab {
  return "activity";
}

function onInputKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    moveActive(1);
    return;
  }
  if (event.key === "ArrowUp") {
    event.preventDefault();
    moveActive(-1);
    return;
  }
  if (event.key === "Enter") {
    event.preventDefault();
    const active = flatResults.value[activeIndex.value];
    if (active) {
      void jumpToTask(active.taskId, {
        activityId: active.activityId ?? undefined,
        attachmentId: active.attachmentId ?? undefined,
        tab: active.tab,
      });
    }
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    close();
  }
}

async function jumpToTask(
  taskId: string,
  options: { activityId?: string | null; attachmentId?: string | null; tab?: TaskDetailTab } = {},
) {
  if (projectsStore.projects.length === 0) {
    await projectsStore.loadProjects();
  }

  const envelope = await desktopBridge.task({ action: "get", task: taskId });
  const task = envelope.result as Task;
  const projectSlug = resolveProjectSlug(projectsStore.projects, task.project_id);

  await router.push(
    projectSlug
      ? {
          path: buildProjectWorkspacePath(projectSlug, "tasks"),
          query: {
            activity: options.activityId || undefined,
            attachment: options.attachmentId || undefined,
            tab: options.tab,
            task: taskId,
          },
        }
      : "/projects",
  );
  close();
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="shell.searchOpen"
      class="fixed inset-0 z-50 flex items-start justify-center bg-black/50 px-4 py-16 backdrop-blur-sm"
      @click.self="close"
    >
      <section
        class="glass-panel flex w-full max-w-3xl flex-col overflow-hidden"
        role="dialog"
        aria-modal="true"
        :aria-label="t('search.dialogLabel')"
      >
        <div class="border-b border-[var(--border-color)] px-5 py-4">
          <label class="flex items-center gap-3">
            <Search :size="18" class="text-[var(--text-muted)]" />
            <input
              ref="inputEl"
              v-model="inputValue"
              aria-controls="global-search-results"
              :aria-activedescendant="activeResultId"
              :aria-label="t('search.placeholder')"
              class="w-full bg-transparent text-base text-[var(--text-main)] outline-none"
              :placeholder="t('search.placeholder')"
              @keydown="onInputKeydown"
            />
          </label>
          <p v-if="search.results" class="mt-2 text-xs text-[var(--text-muted)]">
            {{ retrievalStatus }}
            <span v-if="activeFilterLabels.length > 0"> · {{ activeFilterLabels.join(" · ") }}</span>
          </p>
          <div class="mt-3 flex flex-wrap items-center gap-2">
            <AppSelect
              v-model="selectedTaskKind"
              class="max-w-[11rem]"
              :aria-label="t('tasks.fields.taskKind')"
              :options="taskKindFilterOptions"
              size="compact"
              variant="quiet"
            />
            <AppSelect
              v-model="selectedPriority"
              class="max-w-[10rem]"
              :aria-label="t('tasks.fields.priority')"
              :options="taskPriorityFilterOptions"
              size="compact"
              variant="quiet"
            />
            <AppSelect
              v-model="selectedKnowledgeStatus"
              class="max-w-[11rem]"
              :aria-label="t('tasks.fields.knowledgeStatus')"
              :options="knowledgeStatusFilterOptions"
              size="compact"
              variant="quiet"
            />
            <button
              v-if="activeFilterLabels.length > 0"
              class="status-pill hover:border-[var(--accent-color)]/50"
              type="button"
              @click="clearFilters"
            >
              {{ t("search.clearFilters") }}
            </button>
          </div>
        </div>

        <div
          id="global-search-results"
          class="max-h-[60vh] overflow-y-auto px-3 py-3"
          role="listbox"
          :aria-label="t('search.resultsLabel')"
        >
          <div v-if="search.loading" class="empty-state">{{ t("search.loading") }}</div>
          <div v-else-if="!search.results" class="empty-state">
            {{ t("search.empty") }}
          </div>
          <div v-else class="space-y-4">
            <section role="group" :aria-label="t('search.tasks')">
              <p class="section-label">{{ t("search.tasks") }}</p>
              <button
                v-for="(item, index) in search.results.tasks"
                :key="item.task_id"
                :id="globalSearchOptionId(`task:${item.task_id}`)"
                v-spotlight
                class="list-row spotlight-surface"
                :class="{ 'border-[var(--accent-color)]/60': isActive(`task:${item.task_id}`) }"
                role="option"
                :aria-selected="isActive(`task:${item.task_id}`)"
                tabindex="-1"
                @mouseenter="setActive(index)"
                @click="
                  jumpToTask(item.task_id, {
                    activityId: taskHitTab(item) === 'activity' ? item.evidence_activity_id : undefined,
                    attachmentId: taskHitTab(item) === 'attachments' ? item.evidence_attachment_id : undefined,
                    tab: taskHitTab(item),
                  })
                "
              >
                <SquareKanban :size="15" />
                <div class="min-w-0 flex-1 text-left">
                  <div class="flex items-center gap-2">
                    <p class="truncate text-sm font-medium text-[var(--text-main)]">
                      {{ item.title }}
                    </p>
                    <span
                      v-if="item.task_code"
                      class="rounded-full bg-black/5 px-2 py-0.5 text-[10px] uppercase tracking-[0.12em] text-[var(--text-muted)]"
                    >
                      {{ item.task_code }}
                    </span>
                    <span
                      class="rounded-full bg-[var(--accent-color)]/12 px-2 py-0.5 text-[10px] uppercase tracking-[0.12em] text-[var(--accent-color)]"
                    >
                      {{ t(`search.source.${item.retrieval_source}`) }}
                    </span>
                  </div>
                  <p class="truncate text-xs text-[var(--text-muted)]">{{ item.summary }}</p>
                  <p v-if="item.evidence_snippet" class="truncate text-[11px] text-[var(--text-muted)]/90">
                    <span v-if="item.evidence_source" class="font-medium">
                      {{ localizeEvidenceSource(item.evidence_source, t) }}
                    </span>
                    <span v-if="item.evidence_source"> · </span>
                    <span v-html="renderHighlightedEvidence(item.evidence_snippet, search.results?.query)" />
                  </p>
                </div>
                <div class="list-row-meta">
                  <span>{{ t(`status.task.${item.status}`) }}</span>
                  <span>{{ t(`status.priority.${item.priority}`) }}</span>
                  <span>{{ t(`status.knowledge.${item.knowledge_status}`) }}</span>
                </div>
              </button>
            </section>

            <section role="group" :aria-label="t('search.activity')">
              <p class="section-label">{{ t("search.activity") }}</p>
              <button
                v-for="(item, index) in search.results.activities"
                :key="item.activity_id"
                :id="globalSearchOptionId(`activity:${item.activity_id}`)"
                v-spotlight
                class="list-row spotlight-surface"
                :class="{ 'border-[var(--accent-color)]/60': isActive(`activity:${item.activity_id}`) }"
                role="option"
                :aria-selected="isActive(`activity:${item.activity_id}`)"
                tabindex="-1"
                @mouseenter="setActive(search.results.tasks.length + index)"
                @click="
                  jumpToTask(item.task_id, {
                    activityId: item.activity_id,
                    attachmentId: item.evidence_attachment_id,
                    tab: activityHitTab(item),
                  })
                "
              >
                <Search :size="15" />
                <div class="min-w-0 flex-1 text-left">
                  <p class="truncate text-sm font-medium text-[var(--text-main)]">
                    {{ t(`activityKind.${item.kind}`) }}
                  </p>
                  <p class="truncate text-xs text-[var(--text-muted)]">{{ item.summary }}</p>
                  <p v-if="item.evidence_snippet" class="truncate text-[11px] text-[var(--text-muted)]/90">
                    <span v-if="item.evidence_source" class="font-medium">
                      {{ localizeEvidenceSource(item.evidence_source, t) }}
                    </span>
                    <span v-if="item.evidence_source"> · </span>
                    <span v-html="renderHighlightedEvidence(item.evidence_snippet, search.results?.query)" />
                  </p>
                </div>
                <span class="text-[11px] text-[var(--text-muted)]">{{ item.task_id }}</span>
              </button>
            </section>
          </div>
        </div>
      </section>
    </div>
  </Teleport>
</template>
