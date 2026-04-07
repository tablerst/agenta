<script setup lang="ts">
import { onMounted } from "vue";

import GlobalSearchPalette from "./components/GlobalSearchPalette.vue";
import ShellSidebar from "./components/ShellSidebar.vue";
import { useApprovalsStore } from "./stores/approvals";
import { useProjectsStore } from "./stores/projects";
import { useShellStore } from "./stores/shell";

const shell = useShellStore();
const projects = useProjectsStore();
const approvals = useApprovalsStore();

onMounted(async () => {
  shell.initialize();
  window.addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      shell.openSearch();
    }
    if (event.key === "Escape") {
      shell.closeSearch();
    }
  });

  await Promise.allSettled([projects.loadProjects(), approvals.refreshPendingCount()]);
});
</script>

<template>
  <div class="app-shell">
    <ShellSidebar />
    <main class="min-w-0 flex-1 overflow-hidden">
      <RouterView />
    </main>

    <div
      v-if="shell.notice"
      class="fixed right-5 top-5 z-50 rounded-md border px-4 py-3 text-sm shadow-xl"
      :class="{
        'border-emerald-400/30 bg-emerald-500/10 text-emerald-100': shell.notice.kind === 'success',
        'border-rose-400/30 bg-rose-500/10 text-rose-100': shell.notice.kind === 'error',
        'border-white/10 bg-white/8 text-[var(--text-main)]': shell.notice.kind === 'info',
      }"
    >
      {{ shell.notice.message }}
    </div>

    <GlobalSearchPalette />
  </div>
</template>
