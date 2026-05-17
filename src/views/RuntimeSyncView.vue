<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
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
type SearchQueueSegmentKey = "pending" | "processing" | "failed" | "stale";

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
const activeSearchRun = computed(() => runtimeConsole.searchIndexStatus.value?.active_run ?? null);
const displaySearchRun = computed(
  () => activeSearchRun.value ?? runtimeConsole.searchIndexStatus.value?.latest_run ?? null,
);
const searchRunCompletedCount = computed(() => {
  const run = displaySearchRun.value;
  if (!run) {
    return 0;
  }
  return Math.max(0, run.processed);
});
const searchRunProgressPercent = computed(() => {
  const run = displaySearchRun.value;
  if (!run || run.queued <= 0) {
    return 0;
  }
  return Math.max(0, Math.min(100, Math.round((searchRunCompletedCount.value / run.queued) * 100)));
});
const searchRunIsActive = computed(() => {
  const run = displaySearchRun.value;
  return Boolean(
    run &&
      (activeSearchRun.value ||
        run.status === "running" ||
        run.remaining_count > 0 ||
        run.processing_count > 0),
  );
});
const searchRunHasFailure = computed(() => {
  const run = displaySearchRun.value;
  return Boolean(run && (run.status === "failed" || run.failed > 0 || run.last_error));
});
const searchRunPanelState = computed(() => {
  if (!displaySearchRun.value) {
    return "idle";
  }
  if (searchRunHasFailure.value) {
    return "failed";
  }
  if (searchRunIsActive.value) {
    return "running";
  }
  return "completed";
});
const searchRunPanelClass = computed(() => `runtime-search-run-panel-${searchRunPanelState.value}`);
const searchRunStatusPillClass = computed(() => {
  switch (searchRunPanelState.value) {
    case "running":
      return "status-pill status-pill-warning";
    case "failed":
      return "status-pill status-pill-danger";
    case "completed":
      return "status-pill status-pill-success";
    default:
      return "status-pill";
  }
});
const searchRunLabel = computed(() => {
  if (!displaySearchRun.value) {
    return t("runtime.searchIndex.noRunLabel");
  }
  return activeSearchRun.value
    ? t("runtime.searchIndex.activeRunLabel")
    : t("runtime.searchIndex.latestRunLabel");
});
const searchRunProgressSummary = computed(() => {
  const run = displaySearchRun.value;
  if (!run) {
    return t("runtime.searchIndex.noRunProgressSummary");
  }
  return t("runtime.searchIndex.progressSummary", {
    completed: searchRunCompletedCount.value,
    queued: run.queued,
    percent: searchRunProgressPercent.value,
  });
});
const searchRunProgressAriaLabel = computed(() => {
  const run = displaySearchRun.value;
  if (!run) {
    return t("runtime.searchIndex.noRunProgressSummary");
  }
  return t("runtime.searchIndex.progressAria", {
    completed: searchRunCompletedCount.value,
    queued: run.queued,
    percent: searchRunProgressPercent.value,
    status: runtimeConsole.formatSearchIndexStatus(run.status),
  });
});
const searchQueueCounts = computed(() => {
  const status = runtimeConsole.searchIndexStatus.value;
  return [
    {
      key: "pending" as const,
      label: t("runtime.searchIndex.pending"),
      count: status?.pending_count ?? 0,
      className: "runtime-search-queue-segment-pending",
    },
    {
      key: "processing" as const,
      label: t("runtime.searchIndex.processing"),
      count: status?.processing_count ?? 0,
      className: "runtime-search-queue-segment-processing",
    },
    {
      key: "failed" as const,
      label: t("runtime.searchIndex.failed"),
      count: status?.failed_count ?? 0,
      className: "runtime-search-queue-segment-failed",
    },
    {
      key: "stale" as const,
      label: t("runtime.searchIndex.stale"),
      count: status?.stale_processing_count ?? 0,
      className: "runtime-search-queue-segment-stale",
    },
  ] satisfies Array<{
    key: SearchQueueSegmentKey;
    label: string;
    count: number;
    className: string;
  }>;
});
const visibleSearchQueueSegments = computed(() =>
  searchQueueCounts.value.filter((segment) => segment.count > 0),
);
const searchQueueTotal = computed(() =>
  searchQueueCounts.value.reduce((sum, segment) => sum + segment.count, 0),
);
const searchQueueHealthSummary = computed(() => {
  const status = runtimeConsole.searchIndexStatus.value;
  if (!status?.enabled) {
    return t("runtime.searchIndex.queueDisabledSummary");
  }
  if (searchQueueTotal.value === 0) {
    return t("runtime.searchIndex.queueEmptySummary");
  }
  return t("runtime.searchIndex.queueHealthSummary", { count: searchQueueTotal.value });
});
const syncAutoStateLabel = computed(() => {
  const auto = runtimeConsole.syncStatus.value?.auto;
  if (!auto?.enabled) {
    return t("runtime.sync.auto.disabled");
  }
  if (auto.paused_reason) {
    return t("runtime.sync.auto.paused");
  }
  if (auto.running) {
    return t("runtime.state.running");
  }
  return t("runtime.sync.auto.idle");
});

watch(
  activeSurface,
  (surface) => {
    void runtimeConsole.setSearchIndexLiveRefresh(surface === "search");
  },
  { immediate: true },
);

onUnmounted(() => {
  void runtimeConsole.setSearchIndexLiveRefresh(false);
});
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
              :aria-busy="runtimeConsole.isSyncActionPending('refresh') ? 'true' : undefined"
              :data-pending="runtimeConsole.isSyncActionPending('refresh') ? 'true' : undefined"
              :disabled="runtimeConsole.syncBusy.value"
              @click="runtimeConsole.loadSearchIndexStatus()"
            >
              <RotateCcw :size="14" />
              {{ t("runtime.actions.refresh") }}
            </button>
            <span
              v-if="runtimeConsole.searchIndexAutoRefreshActive.value"
              class="status-pill status-pill-warning"
            >
              {{ t("runtime.searchIndex.autoRefresh") }}
            </span>
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
                  <p class="section-label">{{ t("runtime.sync.auto.title") }}</p>
                  <h3 class="runtime-subblock-title">{{ syncAutoStateLabel }}</h3>
                  <p class="runtime-block-summary">{{ t("runtime.sync.auto.summary") }}</p>
                </div>
                <span
                  class="status-pill"
                  :class="{
                    'status-pill-success': runtimeConsole.syncStatus.value.auto.enabled && !runtimeConsole.syncStatus.value.auto.paused_reason,
                    'status-pill-warning': runtimeConsole.syncStatus.value.auto.paused_reason,
                  }"
                >
                  {{
                    runtimeConsole.syncStatus.value.auto.enabled
                      ? t("runtime.sync.auto.enabled")
                      : t("runtime.sync.auto.disabled")
                  }}
                </span>
              </div>
              <dl class="runtime-definition-list">
                <div>
                  <dt>{{ t("runtime.sync.auto.interval") }}</dt>
                  <dd>
                    {{
                      t("runtime.sync.auto.intervalSeconds", {
                        seconds: runtimeConsole.syncStatus.value.auto.interval_seconds,
                      })
                    }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.auto.batchLimit") }}</dt>
                  <dd>{{ runtimeConsole.syncStatus.value.auto.batch_limit }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.auto.startupBackfill") }}</dt>
                  <dd>
                    {{
                      runtimeConsole.syncStatus.value.auto.startup_backfill
                        ? t("runtime.sync.auto.on")
                        : t("runtime.sync.auto.off")
                    }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.auto.lastStartedAt") }}</dt>
                  <dd>{{ formatDateTime(runtimeConsole.syncStatus.value.auto.last_started_at) }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.auto.lastFinishedAt") }}</dt>
                  <dd>{{ formatDateTime(runtimeConsole.syncStatus.value.auto.last_finished_at) }}</dd>
                </div>
                <div>
                  <dt>{{ t("runtime.sync.auto.conflicts") }}</dt>
                  <dd>{{ runtimeConsole.syncStatus.value.conflict_count }}</dd>
                </div>
              </dl>
              <p
                v-if="runtimeConsole.syncStatus.value.auto.paused_reason"
                class="runtime-sync-error"
              >
                {{
                  t("runtime.sync.auto.pausedReason", {
                    reason: runtimeConsole.syncStatus.value.auto.paused_reason,
                  })
                }}
              </p>
              <p v-if="runtimeConsole.syncStatus.value.auto.last_error" class="runtime-sync-error">
                {{ runtimeConsole.syncStatus.value.auto.last_error }}
              </p>
            </section>

            <section v-if="displaySearchRun" class="runtime-block runtime-block-nested">
              <div class="runtime-block-header">
                <div>
                  <p class="section-label">
                    {{
                      activeSearchRun
                        ? t("runtime.searchIndex.activeRunLabel")
                        : t("runtime.searchIndex.latestRunLabel")
                    }}
                  </p>
                  <h3 class="runtime-subblock-title">
                    {{ t("runtime.searchIndex.progressTitle") }}
                  </h3>
                  <p class="runtime-block-summary">
                    {{
                      t("runtime.searchIndex.progressSummary", {
                        completed: searchRunCompletedCount,
                        queued: displaySearchRun.queued,
                        percent: searchRunProgressPercent,
                      })
                    }}
                  </p>
                </div>
                <div class="flex flex-wrap items-center gap-2">
                  <span class="status-pill status-pill-success">
                    {{ searchRunProgressPercent }}%
                  </span>
                  <span class="status-pill">
                    {{
                      runtimeConsole.formatSearchIndexStatus(displaySearchRun.status)
                    }}
                  </span>
                </div>
              </div>

              <div
                class="runtime-search-progress-track"
                role="progressbar"
                :aria-valuemin="0"
                :aria-valuemax="100"
                :aria-valuenow="searchRunProgressPercent"
                tabindex="0"
              >
                <div
                  class="runtime-search-progress-fill"
                  :style="{ width: `${searchRunProgressPercent}%` }"
                />
                <div class="runtime-search-progress-popover" role="tooltip">
                  <dl class="runtime-search-popover-grid">
                    <div>
                      <dt>{{ t("runtime.searchIndex.scanned") }}</dt>
                      <dd>{{ displaySearchRun.scanned }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.included") }}</dt>
                      <dd>{{ displaySearchRun.queued }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.processed") }}</dt>
                      <dd>{{ displaySearchRun.processed }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.succeeded") }}</dt>
                      <dd>{{ displaySearchRun.succeeded }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.unchanged") }}</dt>
                      <dd>{{ displaySearchRun.unchanged }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.failed") }}</dt>
                      <dd>{{ displaySearchRun.failed }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.remaining") }}</dt>
                      <dd>{{ displaySearchRun.remaining_count }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.retrying") }}</dt>
                      <dd>{{ displaySearchRun.retrying_count }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.batchSize") }}</dt>
                      <dd>{{ displaySearchRun.batch_size }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.lastUpdated") }}</dt>
                      <dd>{{ runtimeConsole.searchIndexLastUpdatedLabel.value }}</dd>
                    </div>
                  </dl>
                </div>
              </div>
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
          <section
            class="runtime-block runtime-block-nested runtime-search-run-panel"
            :class="searchRunPanelClass"
          >
            <div class="runtime-block-header runtime-search-run-header">
              <div>
                <p class="section-label">{{ searchRunLabel }}</p>
                <h3 class="runtime-subblock-title">{{ t("runtime.searchIndex.progressTitle") }}</h3>
                <p class="runtime-block-summary">{{ searchRunProgressSummary }}</p>
              </div>
              <div class="runtime-search-run-badges">
                <span :class="searchRunStatusPillClass">
                  {{
                    displaySearchRun
                      ? runtimeConsole.formatSearchIndexStatus(displaySearchRun.status)
                      : t("runtime.searchIndex.noRunStatus")
                  }}
                </span>
                <span v-if="displaySearchRun" class="status-pill runtime-search-progress-percent">
                  {{ searchRunProgressPercent }}%
                </span>
              </div>
            </div>

            <div class="runtime-search-progress-shell">
              <div
                class="runtime-search-progress-track runtime-search-progress-track-large"
                :class="{
                  'runtime-search-progress-track-active': searchRunIsActive,
                  'runtime-search-progress-track-failed': searchRunHasFailure,
                }"
                role="progressbar"
                :aria-label="searchRunProgressAriaLabel"
                :aria-valuemin="0"
                :aria-valuemax="100"
                :aria-valuenow="searchRunProgressPercent"
                tabindex="0"
              >
                <div
                  class="runtime-search-progress-fill"
                  :style="{ width: `${searchRunProgressPercent}%` }"
                />
                <div
                  v-if="displaySearchRun"
                  class="runtime-search-progress-popover"
                  role="tooltip"
                >
                  <dl class="runtime-search-popover-grid">
                    <div>
                      <dt>{{ t("runtime.searchIndex.scanned") }}</dt>
                      <dd>{{ displaySearchRun.scanned }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.included") }}</dt>
                      <dd>{{ displaySearchRun.queued }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.processed") }}</dt>
                      <dd>{{ displaySearchRun.processed }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.succeeded") }}</dt>
                      <dd>{{ displaySearchRun.succeeded }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.unchanged") }}</dt>
                      <dd>{{ displaySearchRun.unchanged }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.failed") }}</dt>
                      <dd>{{ displaySearchRun.failed }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.remaining") }}</dt>
                      <dd>{{ displaySearchRun.remaining_count }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.retrying") }}</dt>
                      <dd>{{ displaySearchRun.retrying_count }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.batchSize") }}</dt>
                      <dd>{{ displaySearchRun.batch_size }}</dd>
                    </div>
                    <div>
                      <dt>{{ t("runtime.searchIndex.lastUpdated") }}</dt>
                      <dd>{{ runtimeConsole.searchIndexLastUpdatedLabel.value }}</dd>
                    </div>
                  </dl>
                </div>
              </div>

              <div v-if="displaySearchRun" class="runtime-search-progress-meta">
                <span>{{ t(`runtime.searchIndex.operations.${displaySearchRun.operation_kind}`) }}</span>
                <span>
                  {{ t("runtime.searchIndex.remaining") }}
                  {{ displaySearchRun.remaining_count }}
                </span>
                <span>{{ runtimeConsole.searchIndexLastUpdatedLabel.value }}</span>
              </div>
              <p v-else class="runtime-metadata-meta">
                {{ t("runtime.searchIndex.noRunProgressHint") }}
              </p>
            </div>
          </section>

          <div class="runtime-main-column">
            <section class="runtime-block runtime-block-nested runtime-search-status-block">
              <div class="runtime-block-header">
                <div>
                  <p class="section-label">{{ t("runtime.searchIndex.queueLabel") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.searchIndex.queueTitle") }}</h3>
                  <p class="runtime-block-summary">{{ searchQueueHealthSummary }}</p>
                </div>
                <span :class="runtimeConsole.searchIndexHealthClass.value">
                  {{ t(`runtime.searchIndex.surfaceStates.${runtimeConsole.searchIndexSurfaceState.value}.label`) }}
                </span>
              </div>

              <div
                class="runtime-search-queue-meter"
                role="img"
                :aria-label="t('runtime.searchIndex.queueHealthAria', { count: searchQueueTotal })"
              >
                <span
                  v-if="visibleSearchQueueSegments.length === 0"
                  class="runtime-search-queue-segment runtime-search-queue-segment-empty"
                />
                <span
                  v-for="segment in visibleSearchQueueSegments"
                  :key="segment.key"
                  class="runtime-search-queue-segment"
                  :class="segment.className"
                  :style="{ flexGrow: segment.count }"
                  :title="t('runtime.searchIndex.queueSegmentTitle', {
                    label: segment.label,
                    count: segment.count,
                  })"
                />
              </div>

              <dl class="runtime-search-queue-list">
                <div v-for="segment in searchQueueCounts" :key="segment.key">
                  <dt>
                    <span
                      class="runtime-search-queue-dot"
                      :class="segment.className"
                      aria-hidden="true"
                    />
                    {{ segment.label }}
                  </dt>
                  <dd>{{ segment.count }}</dd>
                </div>
              </dl>

              <dl class="runtime-definition-list runtime-search-context-grid">
                <div>
                  <dt>{{ t("runtime.searchIndex.sidecar") }}</dt>
                  <dd>
                    {{
                      runtimeConsole.searchIndexStatus.value
                        ? runtimeConsole.formatSearchIndexSidecar(
                            runtimeConsole.searchIndexStatus.value.sidecar,
                          )
                        : t("common.na")
                    }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.lastUpdated") }}</dt>
                  <dd>{{ runtimeConsole.searchIndexLastUpdatedLabel.value }}</dd>
                </div>
              </dl>
            </section>

            <section class="runtime-block runtime-block-nested runtime-search-maintenance-block">
              <div class="runtime-block-header">
                <div>
                  <p class="section-label">{{ t("runtime.searchIndex.maintenanceLabel") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.searchIndex.rebuildTitle") }}</h3>
                  <p class="runtime-block-summary">{{ t("runtime.searchIndex.rebuildSummary") }}</p>
                </div>
                <button
                  class="primary-action spotlight-surface"
                  :aria-busy="
                    runtimeConsole.isSyncActionPending('searchIndex') ? 'true' : undefined
                  "
                  :data-pending="
                    runtimeConsole.isSyncActionPending('searchIndex') ? 'true' : undefined
                  "
                  :disabled="
                    runtimeConsole.syncBusy.value ||
                    runtimeConsole.searchIndexStatus.value?.requires_full_rebuild
                  "
                  @click="runtimeConsole.runSearchIndex"
                >
                  <Search :size="14" />
                  {{ t("runtime.actions.searchIndex") }}
                </button>
              </div>

              <p class="runtime-search-operation-note">
                {{
                  runtimeConsole.searchIndexStatus.value?.requires_full_rebuild
                    ? t("runtime.searchIndex.fullRebuildRequired")
                    : t("runtime.searchIndex.incrementalSummary")
                }}
              </p>

              <details class="runtime-search-advanced">
                <summary>{{ t("runtime.searchIndex.advancedParameters") }}</summary>
                <dl class="runtime-search-index-strip runtime-search-advanced-grid">
                  <div class="runtime-search-index-item runtime-search-index-item-control">
                    <dt>{{ t("runtime.searchIndex.batchSize") }}</dt>
                    <dd>
                      <input
                        v-model.number="runtimeConsole.searchBackfillForm.batchSize"
                        class="quiet-control-input runtime-search-index-input"
                        inputmode="numeric"
                        min="1"
                        max="200"
                        type="number"
                        @blur="runtimeConsole.normalizeSearchBackfillForm"
                      />
                    </dd>
                  </div>
                  <div class="runtime-search-index-item runtime-search-index-item-control">
                    <dt>{{ t("runtime.searchIndex.limit") }}</dt>
                    <dd>
                      <input
                        v-model.number="runtimeConsole.searchBackfillForm.limit"
                        class="quiet-control-input runtime-search-index-input"
                        inputmode="numeric"
                        min="1"
                        max="100000"
                        type="number"
                        @blur="runtimeConsole.normalizeSearchBackfillForm"
                      />
                    </dd>
                  </div>
                </dl>
                <div class="runtime-search-recovery-actions">
                  <button
                    class="secondary-action spotlight-surface"
                    :aria-busy="
                      runtimeConsole.isSyncActionPending('searchRebuild') ? 'true' : undefined
                    "
                    :data-pending="
                      runtimeConsole.isSyncActionPending('searchRebuild') ? 'true' : undefined
                    "
                    :disabled="runtimeConsole.syncBusy.value"
                    @click="runtimeConsole.runSearchRebuild"
                  >
                    <RotateCcw :size="14" />
                    {{ t("runtime.actions.searchRebuild") }}
                  </button>
                </div>
                <p class="runtime-search-operation-note">
                  {{ t("runtime.searchIndex.rebuildWarning") }}
                </p>
              </details>
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

            <div
              v-if="
                !runtimeConsole.searchIndexStatus.value?.latest_run &&
                !runtimeConsole.searchBackfillResult.value
              "
              class="runtime-log-empty"
            >
              <Search :size="16" />
              <span>{{ t("runtime.searchIndex.noResult") }}</span>
            </div>

            <details v-else class="runtime-search-result-panel runtime-search-advanced">
              <summary>{{ t("runtime.searchIndex.resultDetails") }}</summary>
              <p class="runtime-search-operation-note">
                {{
                  runtimeConsole.searchIndexStatus.value?.latest_run?.operation_description ??
                  runtimeConsole.searchBackfillResult.value?.operation_description ??
                  t("runtime.searchIndex.defaultOperationDescription")
                }}
              </p>

              <dl class="runtime-definition-list runtime-search-context-grid">
                <div>
                  <dt>{{ t("runtime.searchIndex.operation") }}</dt>
                  <dd>
                    {{
                      t(
                        `runtime.searchIndex.operations.${
                          runtimeConsole.searchIndexStatus.value?.latest_run?.operation_kind ??
                          runtimeConsole.searchBackfillResult.value?.operation_kind ??
                          'manual_rebuild'
                        }`,
                      )
                    }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.included") }}</dt>
                  <dd>
                    {{
                      runtimeConsole.searchIndexStatus.value?.latest_run?.queued ??
                      runtimeConsole.searchBackfillResult.value?.queued ??
                      t("common.na")
                    }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.scanned") }}</dt>
                  <dd>
                    {{
                      runtimeConsole.searchIndexStatus.value?.latest_run?.scanned ??
                      runtimeConsole.searchBackfillResult.value?.scanned ??
                      t("common.na")
                    }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.succeeded") }}</dt>
                  <dd>
                    {{
                      runtimeConsole.searchIndexStatus.value?.latest_run?.succeeded ??
                      runtimeConsole.searchBackfillResult.value?.succeeded ??
                      t("common.na")
                    }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.failed") }}</dt>
                  <dd>
                    {{
                      runtimeConsole.searchIndexStatus.value?.latest_run?.failed ??
                      runtimeConsole.searchBackfillResult.value?.failed ??
                      t("common.na")
                    }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.unchanged") }}</dt>
                  <dd>
                    {{
                      runtimeConsole.searchIndexStatus.value?.latest_run?.unchanged ??
                      runtimeConsole.searchBackfillResult.value?.unchanged ??
                      t("common.na")
                    }}
                  </dd>
                </div>
                <div>
                  <dt>{{ t("runtime.searchIndex.remaining") }}</dt>
                  <dd>
                    {{
                      runtimeConsole.searchIndexStatus.value?.latest_run?.remaining_count ??
                      runtimeConsole.searchBackfillResult.value?.pending_after ??
                      t("common.na")
                    }}
                  </dd>
                </div>
              </dl>

              <p
                v-if="
                  runtimeConsole.searchIndexStatus.value?.latest_run?.last_error ||
                  runtimeConsole.searchBackfillResult.value?.processing_error
                "
                class="runtime-sync-error"
              >
                {{
                  runtimeConsole.searchIndexStatus.value?.latest_run?.last_error ??
                  runtimeConsole.searchBackfillResult.value?.processing_error
                }}
              </p>
              <p v-else class="runtime-metadata-meta">
                {{
                  t("runtime.searchIndex.lastResult", {
                    scanned:
                      runtimeConsole.searchIndexStatus.value?.latest_run?.scanned ??
                      runtimeConsole.searchBackfillResult.value?.scanned ??
                      0,
                    included:
                      runtimeConsole.searchIndexStatus.value?.latest_run?.queued ??
                      runtimeConsole.searchBackfillResult.value?.queued ??
                      0,
                    skipped:
                      runtimeConsole.searchIndexStatus.value?.latest_run?.skipped ??
                      runtimeConsole.searchBackfillResult.value?.skipped ??
                      0,
                    unchanged:
                      runtimeConsole.searchIndexStatus.value?.latest_run?.unchanged ??
                      runtimeConsole.searchBackfillResult.value?.unchanged ??
                      0,
                  })
                }}
              </p>

            </details>

            <section
              v-if="runtimeConsole.searchIndexStatus.value?.failed_jobs.length"
              class="runtime-search-result-panel runtime-search-recovery-panel"
            >
              <div class="runtime-block-header">
                <div>
                  <p class="section-label">{{ t("runtime.searchIndex.recoveryLabel") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.searchIndex.recoveryTitle") }}</h3>
                  <p class="runtime-block-summary">{{ t("runtime.searchIndex.recoverySummary") }}</p>
                </div>
              </div>
              <div class="runtime-sync-outbox-list">
                <div class="runtime-search-recovery-actions">
                  <button
                    class="secondary-action spotlight-surface"
                    :aria-busy="
                      runtimeConsole.isSyncActionPending('searchRetryFailed') ? 'true' : undefined
                    "
                    :data-pending="
                      runtimeConsole.isSyncActionPending('searchRetryFailed') ? 'true' : undefined
                    "
                    :disabled="runtimeConsole.syncBusy.value"
                    @click="runtimeConsole.runSearchRetryFailed"
                  >
                    <RotateCcw :size="14" />
                    {{ t("runtime.actions.searchRetryFailed") }}
                  </button>
                  <button
                    class="secondary-action spotlight-surface"
                    :aria-busy="
                      runtimeConsole.isSyncActionPending('searchRecoverStale') ? 'true' : undefined
                    "
                    :data-pending="
                      runtimeConsole.isSyncActionPending('searchRecoverStale') ? 'true' : undefined
                    "
                    :disabled="runtimeConsole.syncBusy.value"
                    @click="runtimeConsole.runSearchRecoverStale"
                  >
                    <Play :size="14" />
                    {{ t("runtime.actions.searchRecoverStale") }}
                  </button>
                </div>
                <article
                  v-for="job in runtimeConsole.searchIndexStatus.value?.failed_jobs ?? []"
                  :key="job.task_id"
                  class="runtime-sync-outbox-item"
                >
                  <div>
                    <strong>{{ job.title ?? job.task_id }}</strong>
                    <span>{{ job.last_error ?? t("runtime.searchIndex.unknownError") }}</span>
                    <span class="runtime-metadata-meta">
                      {{ t("runtime.searchIndex.nextRetryAt") }}
                      {{ formatDateTime(job.next_attempt_at) }}
                    </span>
                  </div>
                  <span class="status-pill status-pill-danger">
                    {{ t("runtime.searchIndex.attempts", { count: job.attempt_count }) }}
                  </span>
                </article>
              </div>
            </section>
          </aside>
        </div>
      </section>
    </div>

    <div v-else class="empty-state">{{ t("runtime.loading") }}</div>
  </section>
</template>
