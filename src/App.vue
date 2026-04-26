<script setup lang="ts">
import { Menu, Search, X } from "@lucide/vue";
import { computed, onMounted, onUnmounted, watch } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";

import GlobalSearchPalette from "./components/GlobalSearchPalette.vue";
import ShellSidebar from "./components/ShellSidebar.vue";
import { useApprovalsStore } from "./stores/approvals";
import { useProjectsStore } from "./stores/projects";
import { useShellStore } from "./stores/shell";

const route = useRoute();
const shell = useShellStore();
const projects = useProjectsStore();
const approvals = useApprovalsStore();
const { locale, t } = useI18n({ useScope: "global" });

const pageTitle = computed(() => {
  void locale.value;
  return t(String(route.meta.titleKey ?? "app.name"));
});
const pageKicker = computed(() => {
  void locale.value;
  return t(String(route.meta.kickerKey ?? "routes.tasks.kicker"));
});

function handleKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    shell.openSearch();
  }
  if (event.key === "Escape") {
    shell.closeSearch();
    shell.closeSidebar();
  }
}

watch(
  () => route.fullPath,
  () => {
    shell.closeSidebar();
  },
);

watch(
  [() => route.fullPath, () => locale.value],
  () => {
    document.title = `${pageTitle.value} | ${t("app.name")}`;
  },
  { immediate: true },
);

onMounted(async () => {
  shell.initialize();
  window.addEventListener("keydown", handleKeydown);

  await Promise.allSettled([projects.loadProjects(), approvals.refreshPendingCount()]);
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <div
    class="app-shell"
    :class="{
      'app-shell-sidebar-collapsed': shell.sidebarCondensed,
      'app-shell-sidebar-open': shell.mobileSidebarOpen,
    }"
  >
    <div
      v-if="shell.mobileSidebarOpen && shell.isCompactViewport"
      class="shell-backdrop"
      @click="shell.closeSidebar()"
    />

    <ShellSidebar />

    <div class="shell-main">
      <header class="shell-mobile-bar">
        <button
          class="icon-button spotlight-surface"
          :aria-label="t('mobile.toggleNavigation')"
          @click="shell.toggleSidebar()"
        >
          <Menu :size="17" />
        </button>

        <div class="min-w-0 flex-1">
          <p class="section-label !mb-1">{{ pageKicker }}</p>
          <p class="truncate text-base font-semibold text-[var(--text-main)]">
            {{ pageTitle }}
          </p>
        </div>

        <button
          class="icon-button spotlight-surface"
          :aria-label="t('mobile.openGlobalSearch')"
          @click="shell.openSearch()"
        >
          <Search :size="17" />
        </button>
      </header>

      <main class="shell-router-view">
        <RouterView />
      </main>
    </div>

    <div
      v-if="shell.notice"
      class="shell-notice"
      :class="{
        'shell-notice-success': shell.notice.kind === 'success',
        'shell-notice-error': shell.notice.kind === 'error',
        'shell-notice-info': shell.notice.kind === 'info',
      }"
    >
      <span class="shell-notice-message">{{ shell.notice.message }}</span>
      <button
        v-if="shell.notice.sticky"
        class="shell-notice-dismiss"
        type="button"
        :aria-label="t('common.dismiss')"
        :title="t('common.dismiss')"
        @click="shell.clearNotice()"
      >
        <X :size="14" />
      </button>
    </div>

    <GlobalSearchPalette />
  </div>
</template>
