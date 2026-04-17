<script setup lang="ts">
import { computed, ref } from "vue";
import { Database, FolderArchive, Play, RotateCcw, Search } from "@lucide/vue";
import { useI18n } from "vue-i18n";

import { formatDateTime } from "../lib/format";
import type { RuntimeConsoleContext } from "../lib/runtimeConsole";

const props = defineProps<{
  runtimeConsole: RuntimeConsoleContext;
}>();
const runtimeConsole = props.runtimeConsole;
const { t } = useI18n({ useScope: "global" });

type SyncSurface = "pipeline" | "search";

const activeSurface = ref<SyncSurface>("pipeline");

const surfaceTabs = computed(() => [
  {
    key: "pipeline" as const,
    label: t("runtime.sync.surfaces.pipeline"),
    summary: t("runtime.sync.surfaces.pipelineSummary"),
  },
  {
    key: "search" as const,
    label: t("runtime.sync.surfaces.search"),
    summary: t("runtime.sync.surfaces.searchSummary"),
  },
]);

const activeSurfaceSummary = computed(
  () => surfaceTabs.value.find((tab) => tab.key === activeSurface.value)?.summary ?? "",
);
</script>

<template>
  <section class="runtime-section-scroll">
    <div v-if="runtimeConsole.syncStatus.value" class="runtime-sync-stack">
      <section class="runtime-block">
        <div class="runtime-block-header">
          <div>
            <p class="section-label">{{ t("runtime.sync.title") }}</p>
            <h2 class="runtime-block-title">{{ t("routes.runtime.sections.sync") }}</h2>
            <p class="runtime-block-summary">{{ activeSurfaceSummary }}</p>
          </div>
          <div v-if="activeSurface === 'pipeline'" class="flex flex-wrap items-center gap-2">
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
              :disabled="
                runtimeConsole.syncBusy.value || !runtimeConsole.syncStatus.value.enabled
              "
              @click="runtimeConsole.runSyncBackfill"
            >
              <FolderArchive :size="14" />
              {{ t("runtime.actions.backfill") }}
            </button>
            <button
              class="primary-action spotlight-surface"
              :aria-busy="runtimeConsole.isSyncActionPending('push') ? 'true' : undefined"
              :data-pending="runtimeConsole.isSyncActionPending('push') ? 'true' : undefined"
              :disabled="
                runtimeConsole.syncBusy.value || !runtimeConsole.syncStatus.value.enabled
              "
              @click="runtimeConsole.runSyncPush"
            >
              <Play :size="14" />
              {{ t("runtime.actions.push") }}
            </button>
            <button
              class="secondary-action spotlight-surface"
              :aria-busy="runtimeConsole.isSyncActionPending('pull') ? 'true' : undefined"
              :data-pending="runtimeConsole.isSyncActionPending('pull') ? 'true' : undefined"
              :disabled="
                runtimeConsole.syncBusy.value || !runtimeConsole.syncStatus.value.enabled
              "
              @click="runtimeConsole.runSyncPull"
            >
              <RotateCcw :size="14" />
              {{ t("runtime.actions.pull") }}
            </button>
          </div>
          <div v-else class="flex flex-wrap items-center gap-2">
            <button
              class="secondary-action spotlight-surface"
              :aria-busy="
                runtimeConsole.isSyncActionPending('searchBackfill') ? 'true' : undefined
              "
              :data-pending="
                runtimeConsole.isSyncActionPending('searchBackfill') ? 'true' : undefined
              "
              :disabled="runtimeConsole.syncBusy.value"
              @click="runtimeConsole.runSearchBackfill"
            >
              <Search :size="14" />
              {{ t("runtime.actions.searchBackfill") }}
            </button>
          </div>
        </div>

        <div class="task-detail-tablist runtime-sync-surface-tabs" role="tablist" :aria-label="t('runtime.sync.surfaceLabel')">
          <button
            v-for="item in surfaceTabs"
            :id="`runtime-sync-tab-${item.key}`"
            :key="item.key"
            :aria-controls="`runtime-sync-panel-${item.key}`"
            :aria-selected="activeSurface === item.key ? 'true' : 'false'"
            class="task-detail-tab"
            :class="{ 'task-detail-tab-active': activeSurface === item.key }"
            role="tab"
            type="button"
            @click="activeSurface = item.key"
          >
            {{ item.label }}
          </button>
        </div>

        <div
          v-if="activeSurface === 'pipeline' && !runtimeConsole.syncStatus.value.enabled"
          :id="'runtime-sync-panel-pipeline'"
          aria-labelledby="runtime-sync-tab-pipeline"
          class="empty-state"
          role="tabpanel"
        >
          {{ t("runtime.sync.disabled") }}
        </div>

        <div
          v-else-if="activeSurface === 'pipeline'"
          :id="'runtime-sync-panel-pipeline'"
          aria-labelledby="runtime-sync-tab-pipeline"
          class="runtime-sync-grid"
          role="tabpanel"
        >
          <div class="runtime-main-column">
            <section class="runtime-block runtime-block-nested">
              <div class="runtime-block-header">
                <div>
                  <p class="section-label">{{ t("runtime.sync.remote") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.sync.remote") }}</h3>
                  <p class="runtime-block-summary">{{ t("runtime.sync.statusSummary") }}</p>
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

        <div
          v-else
          :id="'runtime-sync-panel-search'"
          aria-labelledby="runtime-sync-tab-search"
          class="runtime-sync-search-grid"
          role="tabpanel"
        >
          <div class="runtime-main-column">
            <section class="runtime-block runtime-block-nested">
              <div class="runtime-block-header">
                <div>
                  <p class="section-label">{{ t("runtime.searchIndex.label") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.searchIndex.title") }}</h3>
                  <p class="runtime-block-summary">{{ t("runtime.searchIndex.summary") }}</p>
                </div>
              </div>

              <dl class="runtime-search-index-strip">
                <div class="runtime-search-index-item">
                  <dt>{{ t("runtime.searchIndex.batchSize") }}</dt>
                  <dd>10</dd>
                </div>
                <div class="runtime-search-index-item">
                  <dt>{{ t("runtime.searchIndex.limit") }}</dt>
                  <dd>1000</dd>
                </div>
                <div class="runtime-search-index-item">
                  <dt>{{ t("runtime.searchIndex.lastQueued") }}</dt>
                  <dd>{{ runtimeConsole.searchBackfillResult.value?.queued ?? t("common.na") }}</dd>
                </div>
                <div class="runtime-search-index-item">
                  <dt>{{ t("runtime.searchIndex.pendingAfter") }}</dt>
                  <dd>
                    {{ runtimeConsole.searchBackfillResult.value?.pending_after ?? t("common.na") }}
                  </dd>
                </div>
              </dl>
            </section>

            <section class="runtime-block runtime-block-nested">
              <div class="runtime-block-header">
                <div>
                  <p class="section-label">{{ t("runtime.searchIndex.coverageLabel") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.searchIndex.coverageTitle") }}</h3>
                  <p class="runtime-block-summary">{{ t("runtime.searchIndex.coverageSummary") }}</p>
                </div>
              </div>

              <dl class="runtime-definition-list runtime-search-context-grid">
                <div>
                  <dt>{{ t("runtime.searchIndex.coverage.sources") }}</dt>
                  <dd>{{ t("runtime.searchIndex.coverage.sourcesValue") }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.coverage.target") }}</dt>
                  <dd class="runtime-field-mono">{{ t("runtime.searchIndex.coverage.targetValue") }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.coverage.trigger") }}</dt>
                  <dd>{{ t("runtime.searchIndex.coverage.triggerValue") }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.coverage.isolation") }}</dt>
                  <dd>{{ t("runtime.searchIndex.coverage.isolationValue") }}</dd>
                </div>
              </dl>
            </section>
          </div>

          <aside class="runtime-inspector">
            <div class="runtime-inspector-header">
              <div>
                <p class="section-label">{{ t("runtime.searchIndex.resultLabel") }}</p>
                <h2 class="runtime-block-title">{{ t("runtime.searchIndex.resultTitle") }}</h2>
                <p class="runtime-block-summary">{{ t("runtime.searchIndex.resultSummary") }}</p>
              </div>
            </div>

            <div v-if="!runtimeConsole.searchBackfillResult.value" class="runtime-log-empty">
              <Search :size="16" />
              <span>{{ t("runtime.searchIndex.noResult") }}</span>
            </div>

            <div v-else class="runtime-search-result-panel">
              <dl class="runtime-definition-list runtime-search-context-grid">
                <div>
                  <dt>{{ t("runtime.searchIndex.scanned") }}</dt>
                  <dd>{{ runtimeConsole.searchBackfillResult.value.scanned }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.lastQueued") }}</dt>
                  <dd>{{ runtimeConsole.searchBackfillResult.value.queued }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.skipped") }}</dt>
                  <dd>{{ runtimeConsole.searchBackfillResult.value.skipped }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.pendingAfter") }}</dt>
                  <dd>{{ runtimeConsole.searchBackfillResult.value.pending_after }}</dd>
                </div>
              </dl>

              <p
                v-if="runtimeConsole.searchBackfillResult.value.processing_error"
                class="runtime-sync-error"
              >
                {{ runtimeConsole.searchBackfillResult.value.processing_error }}
              </p>
              <p v-else class="runtime-metadata-meta">
                {{
                  t("runtime.searchIndex.lastResult", {
                    scanned: runtimeConsole.searchBackfillResult.value.scanned,
                    skipped: runtimeConsole.searchBackfillResult.value.skipped,
                  })
                }}
              </p>
            </div>
          </aside>
        </div>
      </section>
    </div>

    <div v-else class="empty-state">{{ t("runtime.loading") }}</div>
  </section>
</template>
