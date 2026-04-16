<script setup lang="ts">
import { computed } from "vue";
import { RouterLink, RouterView, useRoute } from "vue-router";
import { useI18n } from "vue-i18n";

import { useRuntimeConsoleModel } from "../lib/runtimeConsole";
import {
  buildRuntimeWorkspacePath,
  getRuntimeWorkspaceSection,
  runtimeWorkspaceSectionOptions,
  type RuntimeWorkspaceSection,
} from "../lib/runtimeWorkspace";

const route = useRoute();
const { t } = useI18n({ useScope: "global" });

const runtimeConsole = useRuntimeConsoleModel();
const currentSection = computed<RuntimeWorkspaceSection>(() => getRuntimeWorkspaceSection(route));
const navItems = computed(() =>
  runtimeWorkspaceSectionOptions.map((section) => ({
    key: section,
    label: t(`routes.runtime.sections.${section}`),
  })),
);

function sectionLocation(section: RuntimeWorkspaceSection) {
  return buildRuntimeWorkspacePath(section);
}
</script>

<template>
  <section class="runtime-page">
    <header class="runtime-header">
      <div class="runtime-header-copy">
        <p class="section-label">{{ t("runtime.kicker") }}</p>
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="min-w-0">
            <h1 class="workspace-title">{{ t("routes.runtime.title") }}</h1>
            <p class="workspace-summary">{{ t("runtime.shellSummary") }}</p>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <span
              v-if="runtimeConsole.mcp.value"
              :class="runtimeConsole.statusClass.value"
            >
              {{ runtimeConsole.formatRuntimeState(runtimeConsole.mcp.value.state) }}
            </span>
            <span class="status-pill">{{ runtimeConsole.loadedAtLabel.value }}</span>
          </div>
        </div>
      </div>

      <nav class="workspace-secondary-nav" :aria-label="t('runtime.navigation')">
        <RouterLink
          v-for="item in navItems"
          :key="item.key"
          v-slot="{ href, navigate }"
          :to="sectionLocation(item.key)"
          custom
        >
          <a
            :href="href"
            class="section-route-chip"
            :class="{ 'section-route-chip-active': currentSection === item.key }"
            :aria-current="currentSection === item.key ? 'page' : undefined"
            @click="navigate"
          >
            {{ item.label }}
          </a>
        </RouterLink>
      </nav>
    </header>

    <div class="runtime-content">
      <RouterView v-if="route.meta.runtimeSection" v-slot="{ Component }">
        <component :is="Component" :runtime-console="runtimeConsole" />
      </RouterView>
    </div>
  </section>
</template>
