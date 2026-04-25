import type { InjectionKey } from "vue";
import {
  computed,
  inject,
  onMounted,
  onUnmounted,
  provide,
  reactive,
  ref,
} from "vue";
import { useI18n } from "vue-i18n";

import { desktopBridge } from "./desktop";
import { formatDesktopError } from "./errorMessage";
import { formatDateTime } from "./format";
import type {
  McpLaunchOverrides,
  McpLogDestination,
  McpLogEntry,
  McpLogLevel,
  McpRuntimeStatus,
  RuntimeStatus,
  SearchBackfillSummary,
  SearchIndexStatusSummary,
  SyncOutboxListItem,
  SyncStatusSummary,
} from "./types";
import { useShellStore } from "../stores/shell";

type SearchIndexSurfaceState =
  | "disabled"
  | "ready"
  | "running"
  | "degraded"
  | "attention_required";

function createRuntimeConsoleModel() {
  const SEARCH_INDEX_POLL_MS = 3000;
  const shell = useShellStore();
  const { locale, t } = useI18n({ useScope: "global" });

  type RuntimeAction =
    | "refresh"
    | "start"
    | "stop"
    | "refreshLogs"
    | "openLogDirectory";
  type SyncAction =
    | "refresh"
    | "backfill"
    | "push"
    | "pull"
    | "searchBackfill"
    | "searchRetryFailed"
    | "searchRecoverStale";

  const runtime = ref<RuntimeStatus | null>(null);
  const mcp = ref<McpRuntimeStatus | null>(null);
  const syncStatus = ref<SyncStatusSummary | null>(null);
  const syncOutbox = ref<SyncOutboxListItem[]>([]);
  const searchIndexStatus = ref<SearchIndexStatusSummary | null>(null);
  const searchBackfillResult = ref<SearchBackfillSummary | null>(null);
  const searchIndexAutoRefreshActive = ref(false);
  const searchIndexLastUpdated = ref<string>("");
  const searchBackfillForm = reactive({
    batchSize: 10,
    limit: 1000,
  });
  const logs = ref<McpLogEntry[]>([]);
  const busy = ref(false);
  const syncBusy = ref(false);
  const runtimeAction = ref<RuntimeAction | null>(null);
  const syncAction = ref<SyncAction | null>(null);
  const loadedAt = ref<string>("");
  const saveAsDefault = ref(false);
  const unlisteners: Array<() => void> = [];

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
  const loadedAtLabel = computed(() => {
    if (!loadedAt.value) {
      return t("common.na");
    }
    return formatDateTime(loadedAt.value);
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
  const searchIndexLastUpdatedLabel = computed(() => formatDateTime(searchIndexLastUpdated.value));
  const searchIndexSurfaceState = computed<SearchIndexSurfaceState>(() => {
    const status = searchIndexStatus.value;
    if (!status?.enabled) {
      return "disabled";
    }
    if ((status.failed_count ?? 0) > 0 || (status.stale_processing_count ?? 0) > 0) {
      return "attention_required";
    }
    if ((status.pending_count ?? 0) > 0 || (status.processing_count ?? 0) > 0) {
      return "running";
    }
    if (!status.vector_available) {
      return "degraded";
    }
    return "ready";
  });
  const searchIndexHealthClass = computed(() => {
    switch (searchIndexSurfaceState.value) {
      case "ready":
        return "status-pill status-pill-success";
      case "running":
      case "degraded":
        return "status-pill status-pill-warning";
      case "attention_required":
        return "status-pill status-pill-danger";
      default:
        return "status-pill";
    }
  });
  const searchIndexStatusSummary = computed(() => {
    void locale.value;
    return t(`runtime.searchIndex.surfaceStates.${searchIndexSurfaceState.value}.summary`);
  });

  let searchIndexPollTimer: number | null = null;
  let searchIndexPollInFlight = false;
  let searchIndexLiveRefreshRequested = false;

  function clampInteger(
    value: unknown,
    fallback: number,
    min: number,
    max: number,
  ): number {
    const normalized =
      typeof value === "number"
        ? value
        : typeof value === "string" && value.trim()
          ? Number(value)
          : Number.NaN;
    if (!Number.isFinite(normalized)) {
      return fallback;
    }
    return Math.min(max, Math.max(min, Math.trunc(normalized)));
  }

  function normalizeSearchBackfillForm() {
    searchBackfillForm.batchSize = clampInteger(searchBackfillForm.batchSize, 10, 1, 200);
    searchBackfillForm.limit = clampInteger(searchBackfillForm.limit, 1000, 1, 100_000);
  }

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

  function formatSearchIndexStatus(status: string) {
    void locale.value;
    return t(`runtime.searchIndex.statuses.${status}`, status);
  }

  function formatSearchIndexSidecar(status: string) {
    void locale.value;
    return t(`runtime.searchIndex.sidecarStatuses.${status}`, status);
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

  function hasLogFields(fields: Record<string, unknown>) {
    return Object.keys(fields).length > 0;
  }

  function toggleDestination(destination: McpLogDestination) {
    if (form.logDestinations.includes(destination)) {
      form.logDestinations = form.logDestinations.filter((item) => item !== destination);
      return;
    }
    form.logDestinations = [...form.logDestinations, destination];
  }

  function isRuntimeActionPending(action: RuntimeAction) {
    return runtimeAction.value === action;
  }

  function isSyncActionPending(action: SyncAction) {
    return syncAction.value === action;
  }

  async function withRuntimeAction<T>(action: RuntimeAction, task: () => Promise<T>) {
    if (busy.value) {
      return;
    }

    busy.value = true;
    runtimeAction.value = action;
    try {
      return await task();
    } finally {
      runtimeAction.value = null;
      busy.value = false;
    }
  }

  async function withSyncAction<T>(action: SyncAction, task: () => Promise<T>) {
    if (syncBusy.value) {
      return;
    }

    syncBusy.value = true;
    syncAction.value = action;
    try {
      return await task();
    } finally {
      syncAction.value = null;
      syncBusy.value = false;
    }
  }

  async function refreshLogsSnapshot() {
    try {
      const envelope = await desktopBridge.mcpLogsSnapshot();
      logs.value = envelope.result.entries;
    } catch (error) {
      shell.pushNotice("error", formatDesktopError(error, t));
    }
  }

  async function refreshLogs() {
    await withRuntimeAction("refreshLogs", refreshLogsSnapshot);
  }

  function hasSearchIndexWorkInFlight(status: SearchIndexStatusSummary | null) {
    if (!status?.enabled) {
      return false;
    }
    if (status.active_run) {
      return status.active_run.remaining_count > 0 || status.active_run.status === "running";
    }
    return (status.pending_count ?? 0) > 0 || (status.processing_count ?? 0) > 0;
  }

  async function refreshSearchIndexStatusSnapshot(showNotice = true) {
    try {
      const envelope = await desktopBridge.searchIndexStatus();
      searchIndexStatus.value = envelope.result;
      searchIndexLastUpdated.value = new Date().toISOString();
    } catch (error) {
      if (showNotice) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    }
  }

  function stopSearchIndexPolling() {
    if (searchIndexPollTimer !== null) {
      window.clearInterval(searchIndexPollTimer);
      searchIndexPollTimer = null;
    }
    searchIndexAutoRefreshActive.value = false;
  }

  function syncSearchIndexPolling() {
    if (!searchIndexLiveRefreshRequested || !hasSearchIndexWorkInFlight(searchIndexStatus.value)) {
      stopSearchIndexPolling();
      return;
    }
    if (searchIndexPollTimer !== null) {
      searchIndexAutoRefreshActive.value = true;
      return;
    }

    searchIndexAutoRefreshActive.value = true;
    searchIndexPollTimer = window.setInterval(async () => {
      if (searchIndexPollInFlight) {
        return;
      }
      searchIndexPollInFlight = true;
      try {
        await refreshSearchIndexStatusSnapshot(false);
        syncSearchIndexPolling();
      } finally {
        searchIndexPollInFlight = false;
      }
    }, SEARCH_INDEX_POLL_MS);
  }

  async function setSearchIndexLiveRefresh(enabled: boolean) {
    searchIndexLiveRefreshRequested = enabled;
    if (!enabled) {
      stopSearchIndexPolling();
      return;
    }
    await refreshSearchIndexStatusSnapshot(false);
    syncSearchIndexPolling();
  }

  async function loadSearchIndexStatus() {
    await withSyncAction("refresh", async () => {
      await refreshSearchIndexStatusSnapshot();
      syncSearchIndexPolling();
    });
  }

  async function loadSyncSnapshot() {
    try {
      const [syncEnvelope, outboxEnvelope, searchIndexEnvelope] = await Promise.all([
        desktopBridge.syncStatus(),
        desktopBridge.syncOutboxList(20),
        desktopBridge.searchIndexStatus(),
      ]);
      syncStatus.value = syncEnvelope.result;
      syncOutbox.value = outboxEnvelope.result;
      searchIndexStatus.value = searchIndexEnvelope.result;
      searchIndexLastUpdated.value = new Date().toISOString();
      syncSearchIndexPolling();
    } catch (error) {
      shell.pushNotice("error", formatDesktopError(error, t));
    }
  }

  async function loadSync() {
    await withSyncAction("refresh", loadSyncSnapshot);
  }

  async function loadRuntimeSnapshot() {
    try {
      const [
        runtimeEnvelope,
        mcpEnvelope,
        logsEnvelope,
        syncEnvelope,
        outboxEnvelope,
        searchIndexEnvelope,
      ] =
        await Promise.all([
          desktopBridge.status(),
          desktopBridge.mcpStatus(),
          desktopBridge.mcpLogsSnapshot(),
          desktopBridge.syncStatus(),
          desktopBridge.syncOutboxList(20),
          desktopBridge.searchIndexStatus(),
        ]);
      runtime.value = runtimeEnvelope.result;
      mcp.value = mcpEnvelope.result;
      logs.value = logsEnvelope.result.entries;
      syncStatus.value = syncEnvelope.result;
      syncOutbox.value = outboxEnvelope.result;
      searchIndexStatus.value = searchIndexEnvelope.result;
      searchIndexLastUpdated.value = new Date().toISOString();
      syncSearchIndexPolling();
      hydrateForm(mcpEnvelope.result);
      loadedAt.value = new Date().toISOString();
    } catch (error) {
      shell.pushNotice("error", formatDesktopError(error, t));
    }
  }

  async function loadRuntime() {
    await withRuntimeAction("refresh", loadRuntimeSnapshot);
  }

  async function startMcp() {
    await withRuntimeAction("start", async () => {
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
        await Promise.all([refreshLogsSnapshot(), loadDesktopStatus()]);
        saveAsDefault.value = false;
        shell.pushNotice("success", t("notices.runtimeStarted"));
      } catch (error) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    });
  }

  async function stopMcp() {
    await withRuntimeAction("stop", async () => {
      try {
        const envelope = await desktopBridge.mcpStop();
        mcp.value = envelope.result;
        await refreshLogsSnapshot();
        shell.pushNotice("info", t("notices.runtimeStopped"));
      } catch (error) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    });
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
    await withRuntimeAction("openLogDirectory", async () => {
      try {
        await desktopBridge.revealItemInDir(logFilePath);
      } catch (error) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    });
  }

  async function runSyncBackfill() {
    await withSyncAction("backfill", async () => {
      try {
        await desktopBridge.syncBackfill(100);
        await loadSyncSnapshot();
        shell.pushNotice("success", t("notices.syncBackfillCompleted"));
      } catch (error) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    });
  }

  async function runSyncPush() {
    await withSyncAction("push", async () => {
      try {
        await desktopBridge.syncPush(100);
        await loadSyncSnapshot();
        shell.pushNotice("success", t("notices.syncPushCompleted"));
      } catch (error) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    });
  }

  async function runSyncPull() {
    await withSyncAction("pull", async () => {
      try {
        await desktopBridge.syncPull(100);
        await loadSyncSnapshot();
        shell.pushNotice("success", t("notices.syncPullCompleted"));
      } catch (error) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    });
  }

  async function runSearchBackfill() {
    await withSyncAction("searchBackfill", async () => {
      try {
        normalizeSearchBackfillForm();
        const envelope = await desktopBridge.searchBackfill({
          limit: searchBackfillForm.limit,
          batchSize: searchBackfillForm.batchSize,
        });
        searchBackfillResult.value = envelope.result;
        await loadSyncSnapshot();
        if (envelope.result.processing_error) {
          shell.pushNotice("error", envelope.result.processing_error);
          return;
        }
        shell.pushNotice(
          "success",
          t("notices.searchBackfillCompleted", { count: envelope.result.queued }),
        );
      } catch (error) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    });
  }

  async function runSearchRetryFailed() {
    await withSyncAction("searchRetryFailed", async () => {
      try {
        normalizeSearchBackfillForm();
        const envelope = await desktopBridge.searchRetryFailed({
          limit: searchBackfillForm.limit,
          batchSize: searchBackfillForm.batchSize,
        });
        await refreshSearchIndexStatusSnapshot(false);
        syncSearchIndexPolling();
        if (envelope.result.processing_error) {
          shell.pushNotice("error", envelope.result.processing_error);
          return;
        }
        shell.pushNotice(
          "success",
          t("notices.searchRetryFailedCompleted", { count: envelope.result.queued }),
        );
      } catch (error) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    });
  }

  async function runSearchRecoverStale() {
    await withSyncAction("searchRecoverStale", async () => {
      try {
        normalizeSearchBackfillForm();
        const envelope = await desktopBridge.searchRecoverStale({
          limit: searchBackfillForm.limit,
          batchSize: searchBackfillForm.batchSize,
        });
        await refreshSearchIndexStatusSnapshot(false);
        syncSearchIndexPolling();
        if (envelope.result.processing_error) {
          shell.pushNotice("error", envelope.result.processing_error);
          return;
        }
        shell.pushNotice(
          "success",
          t("notices.searchRecoverStaleCompleted", { count: envelope.result.queued }),
        );
      } catch (error) {
        shell.pushNotice("error", formatDesktopError(error, t));
      }
    });
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

    await loadRuntimeSnapshot();
  });

  onUnmounted(() => {
    stopSearchIndexPolling();
    unlisteners.forEach((dispose) => {
      dispose();
    });
  });

  return {
    busy,
    canOpenLogDirectory,
    canSaveDefaults,
    endpointLabel,
    form,
    hasLogFields,
    formatLogDestination,
    formatLogDestinations,
    formatLogLevel,
    formatRuntimeState,
    formatSearchIndexSidecar,
    formatSearchIndexStatus,
    formatSyncEntityKind,
    formatSyncOperation,
    formatSyncOutboxStatus,
    isRuntimeActionPending,
    isSyncActionPending,
    isTransitioning,
    loadRuntime,
    loadSearchIndexStatus,
    loadSync,
    loadedAtLabel,
    logDestinationOptions,
    logLevelClass,
    logLevelOptions,
    logs,
    mcp,
    normalizeSearchBackfillForm,
    openLogDirectory,
    refreshLogs,
    runSearchBackfill,
    runSearchRecoverStale,
    runSearchRetryFailed,
    runSyncBackfill,
    runSyncPull,
    runSyncPush,
    runtime,
    saveAsDefault,
    searchBackfillForm,
    searchBackfillResult,
    searchIndexAutoRefreshActive,
    searchIndexHealthClass,
    searchIndexLastUpdatedLabel,
    searchIndexStatusSummary,
    searchIndexSurfaceState,
    searchIndexStatus,
    setSearchIndexLiveRefresh,
    startMcp,
    statusClass,
    stopMcp,
    syncBusy,
    syncOutbox,
    syncOutboxStatusClass,
    syncPendingCount,
    syncRemoteDatabase,
    syncRemoteHost,
    syncStatus,
    toggleDestination,
    visibleLogs,
    visibleSyncOutbox,
  };
}

export type RuntimeConsoleContext = ReturnType<typeof createRuntimeConsoleModel>;

const runtimeConsoleKey: InjectionKey<RuntimeConsoleContext> = Symbol("runtime-console");

export function useRuntimeConsoleModel() {
  return createRuntimeConsoleModel();
}

export function provideRuntimeConsole() {
  const model = createRuntimeConsoleModel();
  provide(runtimeConsoleKey, model);
  return model;
}

export function useRuntimeConsole() {
  const model = inject(runtimeConsoleKey);

  if (!model) {
    throw new Error("Runtime console context is not available.");
  }

  return model;
}
