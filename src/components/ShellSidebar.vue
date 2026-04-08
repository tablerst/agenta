<script setup lang="ts">
import {
  Activity,
  FolderKanban,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  ShieldCheck,
  SquareKanban,
  SunMedium,
  X,
} from "@lucide/vue";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { RouterLink, useRoute } from "vue-router";

import SidebarLocaleSwitcher from "./SidebarLocaleSwitcher.vue";
import logoUrl from "../assets/logo.svg";
import { bridgeMode } from "../lib/desktop";
import { useApprovalsStore } from "../stores/approvals";
import { useShellStore } from "../stores/shell";

const shell = useShellStore();
const approvals = useApprovalsStore();
const route = useRoute();
const { locale, t } = useI18n({ useScope: "global" });

const navItems = computed(() => {
  void locale.value;
  return [
    { key: "projects", label: t("routes.projects.title"), to: "/projects", icon: FolderKanban },
    { key: "tasks", label: t("routes.tasks.title"), to: "/tasks", icon: SquareKanban },
    { key: "approvals", label: t("routes.approvals.title"), to: "/approvals", icon: ShieldCheck },
    { key: "runtime", label: t("routes.runtime.title"), to: "/runtime", icon: Activity },
  ];
});

const usingPreviewData = bridgeMode === "preview";
const isCondensed = computed(() => shell.sidebarCondensed);
const toggleLabel = computed(() => {
  if (shell.isCompactViewport) {
    return t("sidebar.closeNavigation");
  }
  return isCondensed.value ? t("sidebar.expandSidebar") : t("sidebar.collapseSidebar");
});
const currentThemeLabel = computed(() => {
  void locale.value;
  return t(`theme.${shell.theme}`);
});
const searchLabel = computed(() => t("sidebar.openGlobalSearch"));
const themeLabel = computed(() => t("sidebar.themeLabel", { theme: currentThemeLabel.value }));
const toggleIcon = computed(() => {
  if (shell.isCompactViewport) {
    return X;
  }
  return isCondensed.value ? PanelLeftOpen : PanelLeftClose;
});

function openSearch() {
  shell.openSearch();
  shell.closeSidebar();
}

function navigateAndClose(navigate: () => void) {
  navigate();
  shell.closeSidebar();
}
</script>

<template>
  <aside class="shell-sidebar">
    <div class="shell-sidebar-inner">
      <div class="shell-sidebar-header">
        <RouterLink
          to="/tasks"
          class="shell-brand spotlight-surface"
          :aria-label="t('app.name')"
          :title="t('app.name')"
          @click="shell.closeSidebar()"
        >
          <span class="shell-brand-mark" aria-hidden="true">
            <img :src="logoUrl" alt="" class="shell-brand-logo" />
          </span>
          <div v-if="!isCondensed" class="min-w-0 flex-1">
            <p class="shell-brand-kicker">{{ t("app.name") }}</p>
            <p class="shell-brand-title">{{ t("app.shellTitle") }}</p>
          </div>
        </RouterLink>

        <button
          class="icon-button spotlight-surface"
          :aria-label="toggleLabel"
          :title="toggleLabel"
          @click="shell.toggleSidebar()"
        >
          <component :is="toggleIcon" :size="16" />
        </button>
      </div>

      <div v-if="!isCondensed" class="shell-sidebar-meta">
        <span class="status-pill">{{ t("sidebar.surfaces", { count: navItems.length }) }}</span>
        <span v-if="usingPreviewData" class="status-pill status-pill-preview">{{ t("sidebar.previewData") }}</span>
      </div>

      <nav class="shell-nav">
        <RouterLink
          v-for="item in navItems"
          :key="item.key"
          v-slot="{ href, navigate }"
          :to="item.to"
          custom
        >
          <a
            v-spotlight
            :href="href"
            class="nav-item spotlight-surface"
            :class="{
              'nav-item-active': route.path === item.to,
              'nav-item-collapsed': isCondensed,
            }"
            :aria-current="route.path === item.to ? 'page' : undefined"
            :aria-label="item.label"
            :title="item.label"
            @click="navigateAndClose(navigate)"
          >
            <component :is="item.icon" class="nav-item-icon" :size="17" />
            <span v-if="!isCondensed" class="truncate">{{ item.label }}</span>
            <span
              v-if="item.key === 'approvals' && !isCondensed && approvals.pendingCount > 0"
              class="status-pill status-pill-warning ml-auto"
            >
              {{ approvals.pendingCount }}
            </span>
            <span
              v-else-if="item.key === 'approvals' && isCondensed && approvals.pendingCount > 0"
              class="shell-nav-dot"
            />
          </a>
        </RouterLink>
      </nav>

      <div class="mt-auto flex flex-col gap-3">
        <div class="shell-sidebar-footer">
          <button
            class="search-mock-input spotlight-surface"
            :class="{ 'justify-center px-0': isCondensed }"
            :aria-label="searchLabel"
            :title="searchLabel"
            @click="openSearch()"
          >
            <Search :size="15" />
            <span v-if="!isCondensed" class="flex-1 text-left">{{ t("sidebar.search") }}</span>
            <span v-if="!isCondensed" class="kbd-hint">{{ t("common.shortcutSearch") }}</span>
          </button>

          <SidebarLocaleSwitcher />

          <button
            class="secondary-action spotlight-surface"
            :class="{ 'justify-center px-0': isCondensed }"
            :aria-label="t('sidebar.cycleTheme')"
            :title="t('sidebar.cycleTheme')"
            @click="shell.cycleTheme()"
          >
            <component :is="shell.resolvedTheme === 'dark' ? SunMedium : Moon" :size="15" />
            <span v-if="!isCondensed" class="flex-1 text-left">{{ themeLabel }}</span>
          </button>
        </div>

        <p v-if="!isCondensed && usingPreviewData" class="shell-sidebar-caption">
          {{ t("sidebar.previewCaption") }}
        </p>
      </div>
    </div>
  </aside>
</template>
