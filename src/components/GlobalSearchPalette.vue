<script setup lang="ts">
import { Search, SquareKanban } from "@lucide/vue";
import { nextTick, ref, watch } from "vue";
import { useRouter } from "vue-router";

import { useSearchStore } from "../stores/search";
import { useShellStore } from "../stores/shell";

const shell = useShellStore();
const search = useSearchStore();
const router = useRouter();
const inputValue = ref(search.query);
const inputEl = ref<HTMLInputElement | null>(null);
let timer: number | undefined;

watch(
  () => shell.searchOpen,
  async (isOpen) => {
    if (isOpen) {
      await nextTick();
      inputEl.value?.focus();
    }
  },
);

watch(inputValue, (value) => {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    void search.runSearch(value);
  }, 160);
});

function close() {
  shell.closeSearch();
}

async function jumpToTask(taskId: string) {
  await router.push({ path: "/tasks", query: { task: taskId } });
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
      <section class="glass-panel flex w-full max-w-3xl flex-col overflow-hidden">
        <div class="border-b border-[var(--border-color)] px-5 py-4">
          <label class="flex items-center gap-3">
            <Search :size="18" class="text-[var(--text-muted)]" />
            <input
              ref="inputEl"
              v-model="inputValue"
              class="w-full bg-transparent text-base text-[var(--text-main)] outline-none"
              placeholder="Search tasks and activity summaries"
            />
          </label>
        </div>

        <div class="max-h-[60vh] overflow-y-auto px-3 py-3">
          <div v-if="search.loading" class="empty-state">Searching…</div>
          <div v-else-if="!search.results" class="empty-state">
            Type a query to search task and activity summaries.
          </div>
          <div v-else class="space-y-4">
            <section>
              <p class="section-label">Tasks</p>
              <button
                v-for="item in search.results.tasks"
                :key="item.task_id"
                v-spotlight
                class="list-row spotlight-surface"
                @click="jumpToTask(item.task_id)"
              >
                <SquareKanban :size="15" />
                <div class="min-w-0 flex-1 text-left">
                  <p class="truncate text-sm font-medium text-[var(--text-main)]">
                    {{ item.title }}
                  </p>
                  <p class="truncate text-xs text-[var(--text-muted)]">{{ item.summary }}</p>
                </div>
                <span class="status-pill">{{ item.status }}</span>
              </button>
            </section>

            <section>
              <p class="section-label">Activity</p>
              <button
                v-for="item in search.results.activities"
                :key="item.activity_id"
                v-spotlight
                class="list-row spotlight-surface"
                @click="jumpToTask(item.task_id)"
              >
                <Search :size="15" />
                <div class="min-w-0 flex-1 text-left">
                  <p class="truncate text-sm font-medium text-[var(--text-main)]">
                    {{ item.kind }}
                  </p>
                  <p class="truncate text-xs text-[var(--text-muted)]">{{ item.summary }}</p>
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
