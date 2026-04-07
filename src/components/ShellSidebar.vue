<script setup lang="ts">
import {
  Activity,
  FolderKanban,
  Moon,
  PanelLeft,
  Search,
  ShieldCheck,
  SquareKanban,
  SunMedium,
} from "@lucide/vue";
import { computed } from "vue";
import { RouterLink, useRoute } from "vue-router";

import { useApprovalsStore } from "../stores/approvals";
import { useShellStore } from "../stores/shell";

const shell = useShellStore();
const approvals = useApprovalsStore();
const route = useRoute();

const navItems = computed(() => [
  { name: "Projects", to: "/projects", icon: FolderKanban },
  { name: "Tasks", to: "/tasks", icon: SquareKanban },
  { name: "Approvals", to: "/approvals", icon: ShieldCheck },
  { name: "Runtime", to: "/runtime", icon: Activity },
]);
</script>

<template>
  <aside
    class="flex min-h-0 flex-col border-r border-[var(--border-color)] bg-[var(--bg-sidebar)] px-3 py-4"
    :class="shell.sidebarCollapsed ? 'w-[88px]' : 'w-[250px]'"
  >
    <div class="mb-6 flex items-center justify-between px-2">
      <div>
        <p class="text-[11px] uppercase tracking-[0.28em] text-[var(--text-muted)]">
          Agenta
        </p>
        <p v-if="!shell.sidebarCollapsed" class="mt-1 text-sm font-semibold text-[var(--text-main)]">
          Desktop Console
        </p>
      </div>
      <button class="icon-button spotlight-surface" @click="shell.toggleSidebar()">
        <PanelLeft :size="16" />
      </button>
    </div>

    <nav class="flex flex-col gap-1">
      <RouterLink
        v-for="item in navItems"
        :key="item.name"
        v-slot="{ href, navigate }"
        :to="item.to"
        custom
      >
        <a
          v-spotlight
          :href="href"
          class="nav-item spotlight-surface"
          :class="{ 'nav-item-active': route.path === item.to }"
          @click="navigate"
        >
          <component :is="item.icon" :size="16" />
          <span v-if="!shell.sidebarCollapsed" class="truncate">{{ item.name }}</span>
          <span
            v-if="item.name === 'Approvals' && !shell.sidebarCollapsed && approvals.pendingCount > 0"
            class="status-pill status-pill-warning ml-auto"
          >
            {{ approvals.pendingCount }}
          </span>
        </a>
      </RouterLink>
    </nav>

    <div class="mt-auto flex flex-col gap-2 px-2">
      <button class="secondary-action spotlight-surface justify-start" @click="shell.openSearch()">
        <Search :size="15" />
        <span v-if="!shell.sidebarCollapsed" class="flex-1 text-left">Global Search</span>
        <span v-if="!shell.sidebarCollapsed" class="kbd-hint">Ctrl K</span>
      </button>
      <button class="secondary-action spotlight-surface justify-start" @click="shell.cycleTheme()">
        <component :is="shell.resolvedTheme === 'dark' ? SunMedium : Moon" :size="15" />
        <span v-if="!shell.sidebarCollapsed" class="flex-1 text-left">
          Theme: {{ shell.theme }}
        </span>
      </button>
    </div>
  </aside>
</template>
