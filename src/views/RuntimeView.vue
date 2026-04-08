<script setup lang="ts">
import { Activity, Database, FolderArchive, Workflow } from "@lucide/vue";
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import JsonBlock from "../components/JsonBlock.vue";
import { desktopBridge } from "../lib/desktop";
import { formatDesktopError } from "../lib/errorMessage";
import { formatDateTime } from "../lib/format";
import type { RuntimeStatus } from "../lib/types";
import { useShellStore } from "../stores/shell";

const shell = useShellStore();
const runtime = ref<RuntimeStatus | null>(null);
const loadedAt = ref<string>("");
const { t } = useI18n({ useScope: "global" });

onMounted(async () => {
  try {
    const envelope = await desktopBridge.status();
    runtime.value = envelope.result;
    loadedAt.value = new Date().toISOString();
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  }
});
</script>

<template>
  <section class="h-full overflow-y-auto px-6 py-5">
    <div class="mx-auto max-w-6xl space-y-5">
      <header class="glass-panel p-5">
        <p class="section-label">{{ t("runtime.kicker") }}</p>
        <div class="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h1 class="text-2xl font-semibold text-[var(--text-main)]">{{ t("runtime.title") }}</h1>
            <p class="mt-2 text-sm leading-6 text-[var(--text-muted)]">
              {{ t("runtime.summary") }}
            </p>
          </div>
          <span class="status-pill">{{ formatDateTime(loadedAt) }}</span>
        </div>
      </header>

      <div v-if="runtime" class="grid gap-5 md:grid-cols-2 xl:grid-cols-4">
        <section class="panel-section">
          <div class="mb-2 flex items-center gap-2 text-[var(--text-muted)]">
            <FolderArchive :size="16" />
            <p class="section-label !mb-0">{{ t("runtime.projects") }}</p>
          </div>
          <p class="text-3xl font-semibold">{{ runtime.project_count }}</p>
        </section>
        <section class="panel-section">
          <div class="mb-2 flex items-center gap-2 text-[var(--text-muted)]">
            <Workflow :size="16" />
            <p class="section-label !mb-0">{{ t("runtime.tasks") }}</p>
          </div>
          <p class="text-3xl font-semibold">{{ runtime.task_count }}</p>
        </section>
        <section class="panel-section">
          <div class="mb-2 flex items-center gap-2 text-[var(--text-muted)]">
            <Activity :size="16" />
            <p class="section-label !mb-0">{{ t("runtime.pendingApprovals") }}</p>
          </div>
          <p class="text-3xl font-semibold">{{ runtime.pending_approval_count }}</p>
        </section>
        <section class="panel-section">
          <div class="mb-2 flex items-center gap-2 text-[var(--text-muted)]">
            <Database :size="16" />
            <p class="section-label !mb-0">{{ t("runtime.mcpBind") }}</p>
          </div>
          <p class="text-sm font-medium">{{ runtime.mcp_bind }}{{ runtime.mcp_path }}</p>
        </section>
      </div>

      <section v-if="runtime" class="grid gap-5 xl:grid-cols-[minmax(0,0.46fr)_minmax(0,0.54fr)]">
        <section class="panel-section">
          <p class="section-label">{{ t("runtime.paths") }}</p>
          <dl class="space-y-4 text-sm">
            <div>
              <dt class="text-[var(--text-muted)]">{{ t("runtime.dataDirectory") }}</dt>
              <dd>{{ runtime.data_dir }}</dd>
            </div>
            <div>
              <dt class="text-[var(--text-muted)]">{{ t("runtime.database") }}</dt>
              <dd>{{ runtime.database_path }}</dd>
            </div>
            <div>
              <dt class="text-[var(--text-muted)]">{{ t("runtime.attachments") }}</dt>
              <dd>{{ runtime.attachments_dir }}</dd>
            </div>
          </dl>
        </section>

        <section class="panel-section">
          <p class="section-label">{{ t("runtime.payload") }}</p>
          <JsonBlock :value="runtime" />
        </section>
      </section>

      <div v-else class="empty-state">{{ t("runtime.loading") }}</div>
    </div>
  </section>
</template>
