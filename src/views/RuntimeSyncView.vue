<script setup lang="ts">
import { Database, FolderArchive, Play, RotateCcw } from "@lucide/vue";
import { useI18n } from "vue-i18n";

import { formatDateTime } from "../lib/format";
import type { RuntimeConsoleContext } from "../lib/runtimeConsole";

const props = defineProps<{
  runtimeConsole: RuntimeConsoleContext;
}>();
const runtimeConsole = props.runtimeConsole;
const { t } = useI18n({ useScope: "global" });
</script>

<template>
  <section class="runtime-section-scroll">
    <div v-if="runtimeConsole.syncStatus.value" class="runtime-sync-stack">
      <section class="runtime-block">
        <div class="runtime-block-header">
          <div>
            <p class="section-label">{{ t("runtime.sync.title") }}</p>
            <h2 class="runtime-block-title">{{ t("routes.runtime.sections.sync") }}</h2>
            <p class="runtime-block-summary">{{ t("runtime.sync.statusSummary") }}</p>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <button
              class="secondary-action spotlight-surface"
              :aria-busy="runtimeConsole.isSyncActionPending('refresh') ? 'true' : undefined"
              :data-pending="runtimeConsole.isSyncActionPending('refresh') ? 'true' : undefined"
              :disabled="runtimeConsole.syncBusy.value"
              @click="runtimeConsole.loadSync()"
            >
              <RotateCcw :size="14" />
              {{ t("runtime.actions.refresh") }}
            </button>
            <button
              class="secondary-action spotlight-surface"
              :aria-busy="runtimeConsole.isSyncActionPending('backfill') ? 'true' : undefined"
              :data-pending="runtimeConsole.isSyncActionPending('backfill') ? 'true' : undefined"
              :disabled="runtimeConsole.syncBusy.value"
              @click="runtimeConsole.runSyncBackfill"
            >
              <FolderArchive :size="14" />
              {{ t("runtime.actions.backfill") }}
            </button>
            <button
              class="primary-action spotlight-surface"
              :aria-busy="runtimeConsole.isSyncActionPending('push') ? 'true' : undefined"
              :data-pending="runtimeConsole.isSyncActionPending('push') ? 'true' : undefined"
              :disabled="runtimeConsole.syncBusy.value"
              @click="runtimeConsole.runSyncPush"
            >
              <Play :size="14" />
              {{ t("runtime.actions.push") }}
            </button>
            <button
              class="secondary-action spotlight-surface"
              :aria-busy="runtimeConsole.isSyncActionPending('pull') ? 'true' : undefined"
              :data-pending="runtimeConsole.isSyncActionPending('pull') ? 'true' : undefined"
              :disabled="runtimeConsole.syncBusy.value"
              @click="runtimeConsole.runSyncPull"
            >
              <RotateCcw :size="14" />
              {{ t("runtime.actions.pull") }}
            </button>
          </div>
        </div>

        <div v-if="!runtimeConsole.syncStatus.value.enabled" class="empty-state">
          {{ t("runtime.sync.disabled") }}
        </div>

        <div v-else class="runtime-sync-grid">
          <div class="runtime-main-column">
            <section class="runtime-block runtime-block-nested">
              <div class="runtime-block-header">
                <div>
                  <p class="section-label">{{ t("runtime.sync.remote") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.sync.remote") }}</h3>
                </div>
              </div>
              <dl class="runtime-definition-list">
                <div>
                  <dt>{{ t("runtime.sync.kind") }}</dt>
                  <dd>{{ runtimeConsole.syncStatus.value.remote?.kind ?? t("common.na") }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.host") }}</dt>
                  <dd class="runtime-field-mono">{{ runtimeConsole.syncRemoteHost.value }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.database") }}</dt>
                  <dd class="runtime-field-mono">{{ runtimeConsole.syncRemoteDatabase.value }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.mode") }}</dt>
                  <dd>{{ runtimeConsole.syncStatus.value.mode }}</dd>
                </div>
              </dl>
            </section>

            <section class="runtime-block runtime-block-nested">
              <div class="runtime-block-header">
                <div>
                  <p class="section-label">{{ t("runtime.sync.outbox") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.host.sections.checkpoints") }}</h3>
                </div>
              </div>
              <dl class="runtime-definition-list">
                <div>
                  <dt>{{ t("runtime.sync.pending") }}</dt>
                  <dd>{{ runtimeConsole.syncStatus.value.pending_outbox_count }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.pushAck") }}</dt>
                  <dd class="runtime-field-mono">
                    {{ runtimeConsole.syncStatus.value.checkpoints.push_ack ?? t("common.na") }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.pullCheckpoint") }}</dt>
                  <dd class="runtime-field-mono">
                    {{ runtimeConsole.syncStatus.value.checkpoints.pull ?? t("common.na") }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.oldestPendingAt") }}</dt>
                  <dd>{{ formatDateTime(runtimeConsole.syncStatus.value.oldest_pending_at) }}</dd>
                </div>
              </dl>
            </section>
          </div>

          <aside class="runtime-inspector">
            <div class="runtime-inspector-header">
              <div>
                <p class="section-label">{{ t("runtime.sync.outbox") }}</p>
                <h2 class="runtime-block-title">{{ t("runtime.sync.outbox") }}</h2>
                <p class="runtime-block-summary">{{ t("runtime.sync.outboxSummary") }}</p>
              </div>
              <div class="flex items-center gap-2">
                <span class="status-pill status-pill-warning">
                  {{ t("runtime.sync.pending") }} {{ runtimeConsole.syncPendingCount.value }}
                </span>
              </div>
            </div>

            <div v-if="runtimeConsole.visibleSyncOutbox.value.length === 0" class="runtime-log-empty">
              <Database :size="16" />
              <span>{{ t("runtime.sync.emptyOutbox") }}</span>
            </div>

            <div v-else class="runtime-outbox-list">
              <article
                v-for="item in runtimeConsole.visibleSyncOutbox.value"
                :key="item.mutation_id"
                class="runtime-outbox-row"
              >
                <div class="runtime-log-meta">
                  <span :class="runtimeConsole.syncOutboxStatusClass(item.status)">
                    {{ runtimeConsole.formatSyncOutboxStatus(item.status) }}
                  </span>
                  <span class="status-pill">
                    {{ runtimeConsole.formatSyncEntityKind(item.entity_kind) }}
                  </span>
                  <span class="status-pill">
                    {{ runtimeConsole.formatSyncOperation(item.operation) }}
                  </span>
                </div>
                <p class="runtime-log-message runtime-field-mono">{{ item.local_id }}</p>
                <p class="runtime-metadata-meta">
                  v{{ item.local_version }} · attempts {{ item.attempt_count }} ·
                  {{ formatDateTime(item.created_at) }}
                </p>
                <p v-if="item.last_error" class="runtime-sync-error">{{ item.last_error }}</p>
              </article>
            </div>
          </aside>
        </div>
      </section>
    </div>

    <div v-else class="empty-state">{{ t("runtime.loading") }}</div>
  </section>
</template>
