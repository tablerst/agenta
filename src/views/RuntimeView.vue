<script setup lang="ts">
import {
  Activity,
  AlertTriangle,
  Database,
  FileText,
  FolderArchive,
  Play,
  RotateCcw,
  Square,
} from "@lucide/vue";
import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";

import { desktopBridge } from "../lib/desktop";
import { formatDesktopError } from "../lib/errorMessage";
import { formatDateTime } from "../lib/format";
import type {
  McpLaunchOverrides,
  McpLogDestination,
  McpLogEntry,
  McpLogLevel,
  McpRuntimeStatus,
  RuntimeStatus,
  SyncOutboxListItem,
  SyncStatusSummary,
} from "../lib/types";
import { useShellStore } from "../stores/shell";

const shell = useShellStore();
const runtime = ref<RuntimeStatus | null>(null);
const mcp = ref<McpRuntimeStatus | null>(null);
const syncStatus = ref<SyncStatusSummary | null>(null);
const syncOutbox = ref<SyncOutboxListItem[]>([]);
const logs = ref<McpLogEntry[]>([]);
const busy = ref(false);
const syncBusy = ref(false);
const loadedAt = ref<string>("");
const saveAsDefault = ref(false);
const unlisteners: Array<() => void> = [];
const { locale, t } = useI18n({ useScope: "global" });

const logLevelOptions: McpLogLevel[] = ["trace", "debug", "info", "warn", "error"];
const logDestinationOptions: McpLogDestination[] = ["ui", "stdout", "file"];

const form = reactive({
  bind: "",
  path: "/mcp",
  autostart: false,
  logLevel: "info" as McpRuntimeStatus["log_level"],
  logDestinations: ["ui", "file"] as McpLogDestination[],
  logFilePath: "",
  logUiBufferLines: 1000,
});

const isTransitioning = computed(
  () => mcp.value?.state === "starting" || mcp.value?.state === "stopping",
);
const endpointLabel = computed(() => {
  void locale.value;
  if (!mcp.value) {
    return t("common.na");
  }
  return `${mcp.value.actual_bind ?? mcp.value.bind}${mcp.value.path}`;
});
const canSaveDefaults = computed(() => Boolean(runtime.value?.loaded_config_path));
const visibleLogs = computed(() => [...logs.value].reverse());
const visibleSyncOutbox = computed(() => [...syncOutbox.value].slice(0, 8));
const statusClass = computed(() => statusPillClass(mcp.value?.state ?? "stopped"));
const canOpenLogDirectory = computed(
  () =>
    Boolean(mcp.value?.log_file_path) &&
    (mcp.value?.log_destinations ?? []).includes("file"),
);
const syncPendingCount = computed(() => syncStatus.value?.pending_outbox_count ?? 0);
const syncRemoteHost = computed(
  () => syncStatus.value?.remote?.postgres?.host ?? t("common.na"),
);
const syncRemoteDatabase = computed(
  () => syncStatus.value?.remote?.postgres?.database ?? t("common.na"),
);

function statusPillClass(state: McpRuntimeStatus["state"]) {
  switch (state) {
    case "running":
      return "status-pill status-pill-success";
    case "failed":
      return "status-pill status-pill-danger";
    case "starting":
    case "stopping":
      return "status-pill status-pill-warning";
    default:
      return "status-pill";
  }
}

function logLevelClass(level: McpLogEntry["level"]) {
  switch (level) {
    case "error":
      return "status-pill status-pill-danger";
    case "warn":
      return "status-pill status-pill-warning";
    case "info":
      return "status-pill status-pill-success";
    default:
      return "status-pill";
  }
}

function formatRuntimeState(state: McpRuntimeStatus["state"]) {
  void locale.value;
  return t(`runtime.state.${state}`);
}

function formatLogLevel(level: McpLogLevel) {
  void locale.value;
  return t(`runtime.logLevels.${level}`);
}

function formatLogDestinations(destinations: McpLogDestination[]) {
  void locale.value;
  return destinations.map((destination) => t(`runtime.destinations.${destination}`)).join(" + ");
}

function formatLogDestination(destination: McpLogDestination) {
  void locale.value;
  return t(`runtime.destinations.${destination}`);
}

function formatSyncEntityKind(kind: SyncOutboxListItem["entity_kind"]) {
  void locale.value;
  return t(`runtime.sync.entityKinds.${kind}`);
}

function formatSyncOperation(operation: SyncOutboxListItem["operation"]) {
  void locale.value;
  return t(`runtime.sync.operations.${operation}`);
}

function formatSyncOutboxStatus(status: SyncOutboxListItem["status"]) {
  void locale.value;
  return t(`runtime.sync.statuses.${status}`);
}

function syncOutboxStatusClass(status: SyncOutboxListItem["status"]) {
  switch (status) {
    case "acked":
      return "status-pill status-pill-success";
    case "failed":
      return "status-pill status-pill-danger";
    default:
      return "status-pill status-pill-warning";
  }
}

function hydrateForm(status: McpRuntimeStatus | null) {
  if (!status) {
    return;
  }
  form.bind = status.bind;
  form.path = status.path;
  form.autostart = status.autostart;
  form.logLevel = status.log_level;
  form.logDestinations = [...status.log_destinations];
  form.logFilePath = status.log_file_path;
  form.logUiBufferLines = status.log_ui_buffer_lines;
}

function appendLog(entry: McpLogEntry) {
  const capacity = mcp.value?.log_ui_buffer_lines ?? form.logUiBufferLines;
  logs.value = [...logs.value, entry].slice(-capacity);
}

function formatFields(fields: Record<string, unknown>) {
  if (Object.keys(fields).length === 0) {
    return "";
  }
  return JSON.stringify(fields, null, 2);
}

function toggleDestination(destination: McpLogDestination) {
  if (form.logDestinations.includes(destination)) {
    form.logDestinations = form.logDestinations.filter((item) => item !== destination);
    return;
  }
  form.logDestinations = [...form.logDestinations, destination];
}

async function refreshLogs() {
  try {
    const envelope = await desktopBridge.mcpLogsSnapshot();
    logs.value = envelope.result.entries;
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function loadSync() {
  try {
    const [syncEnvelope, outboxEnvelope] = await Promise.all([
      desktopBridge.syncStatus(),
      desktopBridge.syncOutboxList(20),
    ]);
    syncStatus.value = syncEnvelope.result;
    syncOutbox.value = outboxEnvelope.result;
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function loadRuntime() {
  try {
    const [runtimeEnvelope, mcpEnvelope, logsEnvelope, syncEnvelope, outboxEnvelope] = await Promise.all([
      desktopBridge.status(),
      desktopBridge.mcpStatus(),
      desktopBridge.mcpLogsSnapshot(),
      desktopBridge.syncStatus(),
      desktopBridge.syncOutboxList(20),
    ]);
    runtime.value = runtimeEnvelope.result;
    mcp.value = mcpEnvelope.result;
    logs.value = logsEnvelope.result.entries;
    syncStatus.value = syncEnvelope.result;
    syncOutbox.value = outboxEnvelope.result;
    hydrateForm(mcpEnvelope.result);
    loadedAt.value = new Date().toISOString();
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function startMcp() {
  busy.value = true;
  try {
    const payload: McpLaunchOverrides = {
      bind: form.bind,
      path: form.path,
      autostart: form.autostart,
      log_level: form.logLevel,
      log_destinations: form.logDestinations,
      log_file_path: form.logFilePath,
      log_ui_buffer_lines: form.logUiBufferLines,
      save_as_default: saveAsDefault.value,
    };
    const envelope = await desktopBridge.mcpStart(payload);
    mcp.value = envelope.result;
    hydrateForm(envelope.result);
    await Promise.all([refreshLogs(), loadDesktopStatus()]);
    saveAsDefault.value = false;
    shell.pushNotice("success", t("notices.runtimeStarted"));
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  } finally {
    busy.value = false;
  }
}

async function stopMcp() {
  busy.value = true;
  try {
    const envelope = await desktopBridge.mcpStop();
    mcp.value = envelope.result;
    await refreshLogs();
    shell.pushNotice("info", t("notices.runtimeStopped"));
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  } finally {
    busy.value = false;
  }
}

async function loadDesktopStatus() {
  const envelope = await desktopBridge.status();
  runtime.value = envelope.result;
}

async function openLogDirectory() {
  if (!canOpenLogDirectory.value) {
    return;
  }
  const logFilePath = mcp.value?.log_file_path;
  if (!logFilePath) {
    return;
  }
  try {
    await desktopBridge.revealItemInDir(logFilePath);
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function runSyncBackfill() {
  syncBusy.value = true;
  try {
    await desktopBridge.syncBackfill(100);
    await loadSync();
    shell.pushNotice("success", t("notices.syncBackfillCompleted"));
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  } finally {
    syncBusy.value = false;
  }
}

async function runSyncPush() {
  syncBusy.value = true;
  try {
    await desktopBridge.syncPush(100);
    await loadSync();
    shell.pushNotice("success", t("notices.syncPushCompleted"));
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  } finally {
    syncBusy.value = false;
  }
}

async function runSyncPull() {
  syncBusy.value = true;
  try {
    await desktopBridge.syncPull(100);
    await loadSync();
    shell.pushNotice("success", t("notices.syncPullCompleted"));
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  } finally {
    syncBusy.value = false;
  }
}

onMounted(async () => {
  unlisteners.push(
    await desktopBridge.onMcpStatus((payload) => {
      const sessionChanged = payload.session_id !== mcp.value?.session_id;
      mcp.value = payload;
      if (!busy.value) {
        hydrateForm(payload);
      }
      if (sessionChanged && payload.session_id) {
        logs.value = [];
      }
    }),
  );
  unlisteners.push(
    await desktopBridge.onMcpLog((payload) => {
      appendLog(payload);
    }),
  );

  await loadRuntime();
});

onUnmounted(() => {
  unlisteners.forEach((dispose) => {
    dispose();
  });
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
          <div class="flex flex-wrap items-center gap-2">
            <span :class="statusClass">{{ formatRuntimeState(mcp?.state ?? "stopped") }}</span>
            <span class="status-pill">{{ formatDateTime(loadedAt) }}</span>
          </div>
        </div>
      </header>

      <div v-if="runtime && mcp && syncStatus" class="grid gap-5 md:grid-cols-2 xl:grid-cols-5">
        <section class="panel-section">
          <div class="mb-2 flex items-center gap-2 text-[var(--text-muted)]">
            <Activity :size="16" />
            <p class="section-label !mb-0">{{ t("runtime.lifecycle") }}</p>
          </div>
          <p class="text-2xl font-semibold">{{ formatRuntimeState(mcp.state) }}</p>
          <p class="mt-2 text-sm text-[var(--text-muted)]">{{ mcp.session_id ?? t("runtime.noSession") }}</p>
        </section>

        <section class="panel-section">
          <div class="mb-2 flex items-center gap-2 text-[var(--text-muted)]">
            <Database :size="16" />
            <p class="section-label !mb-0">{{ t("runtime.endpoint") }}</p>
          </div>
          <p class="text-sm font-medium">{{ endpointLabel }}</p>
          <p class="mt-2 text-sm text-[var(--text-muted)]">
            {{ t("runtime.requestedBind") }} {{ mcp.bind }}
          </p>
        </section>

        <section class="panel-section">
          <div class="mb-2 flex items-center gap-2 text-[var(--text-muted)]">
            <FileText :size="16" />
            <p class="section-label !mb-0">{{ t("runtime.logRouting") }}</p>
          </div>
          <p class="text-sm font-medium">{{ formatLogDestinations(mcp.log_destinations) }}</p>
          <p class="mt-2 text-sm text-[var(--text-muted)]">
            {{ t("runtime.bufferSize", { count: mcp.log_ui_buffer_lines }) }}
          </p>
        </section>

        <section class="panel-section">
          <div class="mb-2 flex items-center gap-2 text-[var(--text-muted)]">
            <FolderArchive :size="16" />
            <p class="section-label !mb-0">{{ t("runtime.configSource") }}</p>
          </div>
          <p class="text-sm font-medium">{{ runtime.loaded_config_path ?? t("runtime.transientSession") }}</p>
          <p class="mt-2 text-sm text-[var(--text-muted)]">
            {{ t("runtime.pendingApprovals") }} {{ runtime.pending_approval_count }}
          </p>
        </section>

        <section class="panel-section">
          <div class="mb-2 flex items-center gap-2 text-[var(--text-muted)]">
            <Database :size="16" />
            <p class="section-label !mb-0">{{ t("runtime.sync.title") }}</p>
          </div>
          <p class="text-sm font-medium">{{ syncRemoteDatabase }}</p>
          <p class="mt-2 text-sm text-[var(--text-muted)]">
            {{ t("runtime.sync.pending") }} {{ syncPendingCount }}
          </p>
        </section>
      </div>

      <div v-if="runtime && mcp" class="grid gap-5 xl:grid-cols-[minmax(0,0.54fr)_minmax(0,0.46fr)]">
        <section class="panel-section">
          <div class="mb-4 flex flex-wrap items-start justify-between gap-3">
            <div>
              <p class="section-label">{{ t("runtime.launchConfig") }}</p>
              <p class="text-sm text-[var(--text-muted)]">{{ t("runtime.launchConfigSummary") }}</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <button class="secondary-action spotlight-surface" :disabled="busy" @click="loadRuntime">
                <RotateCcw :size="14" />
                {{ t("runtime.actions.refresh") }}
              </button>
              <button
                class="primary-action spotlight-surface"
                :disabled="busy || isTransitioning"
                @click="startMcp"
              >
                <Play :size="14" />
                {{ t("runtime.actions.start") }}
              </button>
              <button
                class="secondary-action spotlight-surface"
                :disabled="busy || (!mcp || (mcp.state !== 'running' && mcp.state !== 'starting' && mcp.state !== 'failed'))"
                @click="stopMcp"
              >
                <Square :size="14" />
                {{ t("runtime.actions.stop") }}
              </button>
            </div>
          </div>

          <div class="runtime-config-grid">
            <label class="form-field">
              <span class="field-label">{{ t("runtime.fields.bind") }}</span>
              <input v-model="form.bind" class="control-input" />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("runtime.fields.path") }}</span>
              <input v-model="form.path" class="control-input" />
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("runtime.fields.logLevel") }}</span>
              <select v-model="form.logLevel" class="control-select">
                <option v-for="level in logLevelOptions" :key="level" :value="level">
                  {{ formatLogLevel(level) }}
                </option>
              </select>
            </label>
            <label class="form-field">
              <span class="field-label">{{ t("runtime.fields.uiBufferLines") }}</span>
              <input v-model.number="form.logUiBufferLines" class="control-input" min="1" type="number" />
            </label>
            <label class="form-field xl:col-span-2">
              <span class="field-label">{{ t("runtime.fields.logFilePath") }}</span>
              <input v-model="form.logFilePath" class="control-input" />
            </label>
          </div>

          <div class="mt-4 grid gap-4 md:grid-cols-2">
            <section class="panel-section">
              <p class="section-label">{{ t("runtime.logDestinations") }}</p>
              <label
                v-for="destination in logDestinationOptions"
                :key="destination"
                class="runtime-option-row"
              >
                <input
                  :checked="form.logDestinations.includes(destination)"
                  type="checkbox"
                  @change="toggleDestination(destination)"
                />
                <span>{{ formatLogDestination(destination) }}</span>
              </label>
            </section>

            <section class="panel-section">
              <p class="section-label">{{ t("runtime.persistence") }}</p>
              <label class="runtime-option-row">
                <input v-model="form.autostart" type="checkbox" />
                <span>{{ t("runtime.fields.autostart") }}</span>
              </label>
              <p class="mt-3 text-sm text-[var(--text-muted)]">
                {{ t("runtime.autostartHint") }}
              </p>
              <label class="runtime-option-row">
                <input v-model="saveAsDefault" :disabled="!canSaveDefaults" type="checkbox" />
                <span>{{ t("runtime.fields.saveAsDefault") }}</span>
              </label>
              <p class="mt-3 text-sm text-[var(--text-muted)]">
                {{
                  canSaveDefaults
                    ? t("runtime.saveHint")
                    : t("runtime.transientHint")
                }}
              </p>
            </section>
          </div>

          <section class="panel-section mt-4">
            <p class="section-label">{{ t("runtime.paths") }}</p>
            <dl class="space-y-3 text-sm">
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

          <section v-if="syncStatus" class="panel-section mt-4">
            <div class="mb-4 flex flex-wrap items-start justify-between gap-3">
              <div>
                <p class="section-label">{{ t("runtime.sync.title") }}</p>
                <p class="text-sm text-[var(--text-muted)]">{{ t("runtime.sync.statusSummary") }}</p>
              </div>
              <div class="flex flex-wrap items-center gap-2">
                <button class="secondary-action spotlight-surface" :disabled="syncBusy" @click="loadSync">
                  <RotateCcw :size="14" />
                  {{ t("runtime.actions.refresh") }}
                </button>
                <button class="secondary-action spotlight-surface" :disabled="syncBusy" @click="runSyncBackfill">
                  <FolderArchive :size="14" />
                  {{ t("runtime.actions.backfill") }}
                </button>
                <button class="secondary-action spotlight-surface" :disabled="syncBusy" @click="runSyncPush">
                  <Play :size="14" />
                  {{ t("runtime.actions.push") }}
                </button>
                <button class="secondary-action spotlight-surface" :disabled="syncBusy" @click="runSyncPull">
                  <RotateCcw :size="14" />
                  {{ t("runtime.actions.pull") }}
                </button>
              </div>
            </div>

            <div v-if="!syncStatus.enabled" class="empty-state">
              {{ t("runtime.sync.disabled") }}
            </div>
            <template v-else>
              <div class="grid gap-4 md:grid-cols-2">
                <section class="panel-section">
                  <p class="section-label">{{ t("runtime.sync.remote") }}</p>
                  <dl class="space-y-3 text-sm">
                    <div>
                      <dt class="text-[var(--text-muted)]">{{ t("runtime.sync.kind") }}</dt>
                      <dd>{{ syncStatus.remote?.kind ?? t("common.na") }}</dd>
                    </div>
                    <div>
                      <dt class="text-[var(--text-muted)]">{{ t("runtime.sync.host") }}</dt>
                      <dd>{{ syncRemoteHost }}</dd>
                    </div>
                    <div>
                      <dt class="text-[var(--text-muted)]">{{ t("runtime.sync.database") }}</dt>
                      <dd>{{ syncRemoteDatabase }}</dd>
                    </div>
                    <div>
                      <dt class="text-[var(--text-muted)]">{{ t("runtime.sync.mode") }}</dt>
                      <dd>{{ syncStatus.mode }}</dd>
                    </div>
                  </dl>
                </section>

                <section class="panel-section">
                  <p class="section-label">{{ t("runtime.sync.outbox") }}</p>
                  <dl class="space-y-3 text-sm">
                    <div>
                      <dt class="text-[var(--text-muted)]">{{ t("runtime.sync.pending") }}</dt>
                      <dd>{{ syncStatus.pending_outbox_count }}</dd>
                    </div>
                    <div>
                      <dt class="text-[var(--text-muted)]">{{ t("runtime.sync.pushAck") }}</dt>
                      <dd>{{ syncStatus.checkpoints.push_ack ?? t("common.na") }}</dd>
                    </div>
                    <div>
                      <dt class="text-[var(--text-muted)]">{{ t("runtime.sync.pullCheckpoint") }}</dt>
                      <dd>{{ syncStatus.checkpoints.pull ?? t("common.na") }}</dd>
                    </div>
                    <div>
                      <dt class="text-[var(--text-muted)]">{{ t("runtime.sync.oldestPendingAt") }}</dt>
                      <dd>{{ formatDateTime(syncStatus.oldest_pending_at) }}</dd>
                    </div>
                  </dl>
                </section>
              </div>

              <section class="panel-section mt-4">
                <div class="mb-3">
                  <p class="section-label">{{ t("runtime.sync.outbox") }}</p>
                  <p class="text-sm text-[var(--text-muted)]">{{ t("runtime.sync.outboxSummary") }}</p>
                </div>
                <div v-if="visibleSyncOutbox.length === 0" class="empty-state">
                  {{ t("runtime.sync.emptyOutbox") }}
                </div>
                <div v-else class="space-y-3">
                  <article
                    v-for="item in visibleSyncOutbox"
                    :key="item.mutation_id"
                    class="rounded-2xl border border-[color:var(--border-muted)] bg-[var(--surface-raised)] px-4 py-3"
                  >
                    <div class="flex flex-wrap items-center justify-between gap-2">
                      <div class="flex flex-wrap items-center gap-2">
                        <span :class="syncOutboxStatusClass(item.status)">
                          {{ formatSyncOutboxStatus(item.status) }}
                        </span>
                        <span class="status-pill">
                          {{ formatSyncEntityKind(item.entity_kind) }}
                        </span>
                        <span class="status-pill">
                          {{ formatSyncOperation(item.operation) }}
                        </span>
                      </div>
                      <span class="text-xs text-[var(--text-muted)]">
                        {{ formatDateTime(item.created_at) }}
                      </span>
                    </div>
                    <p class="mt-2 text-sm font-medium text-[var(--text-main)]">{{ item.local_id }}</p>
                    <p class="mt-1 text-xs text-[var(--text-muted)]">
                      v{{ item.local_version }} · attempts {{ item.attempt_count }}
                    </p>
                    <p v-if="item.last_error" class="mt-2 text-xs text-[var(--danger-text)]">
                      {{ item.last_error }}
                    </p>
                  </article>
                </div>
              </section>
            </template>
          </section>
        </section>

        <section class="panel-section">
          <div class="mb-4 flex flex-wrap items-start justify-between gap-3">
            <div>
              <p class="section-label">{{ t("runtime.logs") }}</p>
              <p class="text-sm text-[var(--text-muted)]">{{ t("runtime.logsSummary") }}</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <button class="secondary-action spotlight-surface" :disabled="busy" @click="refreshLogs">
                <RotateCcw :size="14" />
                {{ t("runtime.actions.refreshLogs") }}
              </button>
              <button
                class="secondary-action spotlight-surface"
                :disabled="busy || !canOpenLogDirectory"
                @click="openLogDirectory"
              >
                <FileText :size="14" />
                {{ t("runtime.actions.openLogDirectory") }}
              </button>
            </div>
          </div>

          <div v-if="visibleLogs.length === 0" class="empty-state">{{ t("runtime.emptyLogs") }}</div>
          <div v-else class="runtime-log-list">
            <article v-for="entry in visibleLogs" :key="`${entry.timestamp}-${entry.component}-${entry.message}`" class="runtime-log-row">
              <div class="runtime-log-meta">
                <span :class="logLevelClass(entry.level)">{{ formatLogLevel(entry.level) }}</span>
                <span>{{ formatDateTime(entry.timestamp) }}</span>
                <span>{{ entry.component }}</span>
              </div>
              <p class="runtime-log-message">{{ entry.message }}</p>
              <pre v-if="formatFields(entry.fields)" class="runtime-log-fields">{{ formatFields(entry.fields) }}</pre>
            </article>
          </div>
        </section>
      </div>

      <section v-if="mcp && (mcp.last_error || mcp.state === 'failed')" class="panel-section">
        <div class="flex flex-wrap items-start justify-between gap-3">
          <div class="flex items-start gap-3">
            <AlertTriangle class="mt-1 text-[var(--text-main)]" :size="18" />
            <div>
              <p class="section-label">{{ t("runtime.recovery") }}</p>
              <p class="text-sm font-medium text-[var(--text-main)]">{{ t("runtime.failedSummary") }}</p>
              <p class="mt-2 text-sm leading-6 text-[var(--text-muted)]">
                {{ mcp.last_error ?? t("runtime.failedSummary") }}
              </p>
            </div>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <button class="secondary-action spotlight-surface" :disabled="busy" @click="refreshLogs">
              {{ t("runtime.actions.refreshLogs") }}
            </button>
            <button class="primary-action spotlight-surface" :disabled="busy || isTransitioning" @click="startMcp">
              {{ t("runtime.actions.retry") }}
            </button>
          </div>
        </div>
      </section>

      <div v-if="!runtime || !mcp" class="empty-state">{{ t("runtime.loading") }}</div>
    </div>
  </section>
</template>
