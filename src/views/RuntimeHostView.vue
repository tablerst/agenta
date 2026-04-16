<script setup lang="ts">
import {
  Activity,
  AlertTriangle,
  FileText,
  Play,
  RotateCcw,
  Square,
} from "@lucide/vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import { formatDateTime } from "../lib/format";
import type { RuntimeConsoleContext } from "../lib/runtimeConsole";
import { buildRuntimeWorkspacePath } from "../lib/runtimeWorkspace";

const router = useRouter();
const props = defineProps<{
  runtimeConsole: RuntimeConsoleContext;
}>();
const runtimeConsole = props.runtimeConsole;
const { t } = useI18n({ useScope: "global" });

function openSync() {
  void router.push(buildRuntimeWorkspacePath("sync"));
}
</script>

<template>
  <section class="runtime-section-scroll">
    <div v-if="runtimeConsole.runtime.value && runtimeConsole.mcp.value" class="runtime-host-stack">
      <section class="runtime-metadata-strip">
        <article class="runtime-metadata-item">
          <span class="field-label">{{ t("runtime.lifecycle") }}</span>
          <strong class="runtime-metadata-value">
            {{ runtimeConsole.formatRuntimeState(runtimeConsole.mcp.value.state) }}
          </strong>
          <p class="runtime-metadata-meta">
            {{ runtimeConsole.mcp.value.session_id ?? t("runtime.noSession") }}
          </p>
        </article>

        <article class="runtime-metadata-item">
          <span class="field-label">{{ t("runtime.endpoint") }}</span>
          <strong class="runtime-metadata-value runtime-metadata-mono">
            {{ runtimeConsole.endpointLabel.value }}
          </strong>
          <p class="runtime-metadata-meta">
            {{ t("runtime.requestedBind") }} {{ runtimeConsole.mcp.value.bind }}
          </p>
        </article>

        <article class="runtime-metadata-item">
          <span class="field-label">{{ t("runtime.logRouting") }}</span>
          <strong class="runtime-metadata-value">
            {{ runtimeConsole.formatLogDestinations(runtimeConsole.mcp.value.log_destinations) }}
          </strong>
          <p class="runtime-metadata-meta">
            {{ t("runtime.bufferSize", { count: runtimeConsole.mcp.value.log_ui_buffer_lines }) }}
          </p>
        </article>

        <article class="runtime-metadata-item">
          <span class="field-label">{{ t("runtime.configSource") }}</span>
          <strong class="runtime-metadata-value runtime-metadata-mono">
            {{ runtimeConsole.runtime.value.loaded_config_path ?? t("runtime.transientSession") }}
          </strong>
          <p class="runtime-metadata-meta">
            {{ t("runtime.pendingApprovals") }} {{ runtimeConsole.runtime.value.pending_approval_count }}
          </p>
        </article>

        <button class="runtime-metadata-item runtime-metadata-action" type="button" @click="openSync">
          <span class="field-label">{{ t("runtime.sync.title") }}</span>
          <strong class="runtime-metadata-value">
            {{ runtimeConsole.syncRemoteDatabase.value }}
          </strong>
          <p class="runtime-metadata-meta">
            {{ t("runtime.sync.pending") }} {{ runtimeConsole.syncPendingCount.value }}
          </p>
        </button>
      </section>

      <div class="runtime-host-grid">
        <div class="runtime-main-column">
          <section class="runtime-block">
            <div class="runtime-block-header">
              <div>
                <p class="section-label">{{ t("runtime.host.sections.control") }}</p>
                <h2 class="runtime-block-title">{{ t("runtime.launchConfig") }}</h2>
                <p class="runtime-block-summary">{{ t("runtime.launchConfigSummary") }}</p>
              </div>
              <div class="flex flex-wrap items-center gap-2">
                <button
                  class="secondary-action spotlight-surface"
                  :aria-busy="runtimeConsole.isRuntimeActionPending('refresh') ? 'true' : undefined"
                  :data-pending="runtimeConsole.isRuntimeActionPending('refresh') ? 'true' : undefined"
                  :disabled="runtimeConsole.busy.value"
                  @click="runtimeConsole.loadRuntime()"
                >
                  <RotateCcw :size="14" />
                  {{ t("runtime.actions.refresh") }}
                </button>
                <button
                  class="primary-action spotlight-surface"
                  :aria-busy="runtimeConsole.isRuntimeActionPending('start') ? 'true' : undefined"
                  :data-pending="runtimeConsole.isRuntimeActionPending('start') ? 'true' : undefined"
                  :disabled="runtimeConsole.busy.value || runtimeConsole.isTransitioning.value"
                  @click="runtimeConsole.startMcp"
                >
                  <Play :size="14" />
                  {{ t("runtime.actions.start") }}
                </button>
                <button
                  class="secondary-action spotlight-surface"
                  :aria-busy="runtimeConsole.isRuntimeActionPending('stop') ? 'true' : undefined"
                  :data-pending="runtimeConsole.isRuntimeActionPending('stop') ? 'true' : undefined"
                  :disabled="
                    runtimeConsole.busy.value ||
                    (runtimeConsole.mcp.value.state !== 'running' &&
                      runtimeConsole.mcp.value.state !== 'starting' &&
                      runtimeConsole.mcp.value.state !== 'failed')
                  "
                  @click="runtimeConsole.stopMcp"
                >
                  <Square :size="14" />
                  {{ t("runtime.actions.stop") }}
                </button>
              </div>
            </div>

            <div class="runtime-field-grid">
              <label class="form-field">
                <span class="field-label">{{ t("runtime.fields.bind") }}</span>
                <input v-model="runtimeConsole.form.bind" class="quiet-control-input" />
              </label>
              <label class="form-field">
                <span class="field-label">{{ t("runtime.fields.path") }}</span>
                <input v-model="runtimeConsole.form.path" class="quiet-control-input" />
              </label>
              <label class="form-field">
                <span class="field-label">{{ t("runtime.fields.logLevel") }}</span>
                <select v-model="runtimeConsole.form.logLevel" class="quiet-control-select">
                  <option
                    v-for="level in runtimeConsole.logLevelOptions"
                    :key="level"
                    :value="level"
                  >
                    {{ runtimeConsole.formatLogLevel(level) }}
                  </option>
                </select>
              </label>
              <label class="form-field">
                <span class="field-label">{{ t("runtime.fields.uiBufferLines") }}</span>
                <input
                  v-model.number="runtimeConsole.form.logUiBufferLines"
                  class="quiet-control-input"
                  min="1"
                  type="number"
                />
              </label>
              <label class="form-field runtime-field-wide">
                <span class="field-label">{{ t("runtime.fields.logFilePath") }}</span>
                <input v-model="runtimeConsole.form.logFilePath" class="quiet-control-input runtime-field-mono" />
              </label>
            </div>
          </section>

          <section class="runtime-block">
            <div class="runtime-split-blocks">
              <div class="runtime-subblock">
                <div class="runtime-subblock-header">
                  <p class="section-label">{{ t("runtime.host.sections.destinations") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.logDestinations") }}</h3>
                </div>
                <label
                  v-for="destination in runtimeConsole.logDestinationOptions"
                  :key="destination"
                  class="runtime-option-row"
                >
                  <input
                    :checked="runtimeConsole.form.logDestinations.includes(destination)"
                    type="checkbox"
                    @change="runtimeConsole.toggleDestination(destination)"
                  />
                  <span>{{ runtimeConsole.formatLogDestination(destination) }}</span>
                </label>
              </div>

              <div class="runtime-subblock">
                <div class="runtime-subblock-header">
                  <p class="section-label">{{ t("runtime.host.sections.persistence") }}</p>
                  <h3 class="runtime-subblock-title">{{ t("runtime.persistence") }}</h3>
                </div>
                <label class="runtime-option-row">
                  <input v-model="runtimeConsole.form.autostart" type="checkbox" />
                  <span>{{ t("runtime.fields.autostart") }}</span>
                </label>
                <p class="runtime-option-hint">{{ t("runtime.autostartHint") }}</p>
                <label class="runtime-option-row">
                  <input
                    v-model="runtimeConsole.saveAsDefault.value"
                    :disabled="!runtimeConsole.canSaveDefaults.value"
                    type="checkbox"
                  />
                  <span>{{ t("runtime.fields.saveAsDefault") }}</span>
                </label>
                <p class="runtime-option-hint">
                  {{
                    runtimeConsole.canSaveDefaults.value
                      ? t("runtime.saveHint")
                      : t("runtime.transientHint")
                  }}
                </p>
              </div>
            </div>
          </section>

          <section class="runtime-block">
            <div class="runtime-block-header">
              <div>
                <p class="section-label">{{ t("runtime.host.sections.paths") }}</p>
                <h2 class="runtime-block-title">{{ t("runtime.paths") }}</h2>
              </div>
            </div>
            <dl class="runtime-definition-list">
              <div>
                <dt>{{ t("runtime.dataDirectory") }}</dt>
                <dd class="runtime-field-mono">{{ runtimeConsole.runtime.value.data_dir }}</dd>
              </div>
              <div>
                <dt>{{ t("runtime.database") }}</dt>
                <dd class="runtime-field-mono">{{ runtimeConsole.runtime.value.database_path }}</dd>
              </div>
              <div>
                <dt>{{ t("runtime.attachments") }}</dt>
                <dd class="runtime-field-mono">{{ runtimeConsole.runtime.value.attachments_dir }}</dd>
              </div>
            </dl>
          </section>

          <section
            v-if="runtimeConsole.mcp.value.last_error || runtimeConsole.mcp.value.state === 'failed'"
            class="runtime-block"
          >
            <div class="runtime-block-header">
              <div class="runtime-alert-copy">
                <AlertTriangle class="mt-1 text-[var(--text-main)]" :size="18" />
                <div>
                  <p class="section-label">{{ t("runtime.recovery") }}</p>
                  <h2 class="runtime-block-title">{{ t("runtime.failedSummary") }}</h2>
                  <p class="runtime-block-summary">
                    {{ runtimeConsole.mcp.value.last_error ?? t("runtime.failedSummary") }}
                  </p>
                </div>
              </div>
              <div class="flex flex-wrap items-center gap-2">
                <button
                  class="secondary-action spotlight-surface"
                  :aria-busy="runtimeConsole.isRuntimeActionPending('refreshLogs') ? 'true' : undefined"
                  :data-pending="runtimeConsole.isRuntimeActionPending('refreshLogs') ? 'true' : undefined"
                  :disabled="runtimeConsole.busy.value"
                  @click="runtimeConsole.refreshLogs()"
                >
                  {{ t("runtime.actions.refreshLogs") }}
                </button>
                <button
                  class="primary-action spotlight-surface"
                  :aria-busy="runtimeConsole.isRuntimeActionPending('start') ? 'true' : undefined"
                  :data-pending="runtimeConsole.isRuntimeActionPending('start') ? 'true' : undefined"
                  :disabled="runtimeConsole.busy.value || runtimeConsole.isTransitioning.value"
                  @click="runtimeConsole.startMcp"
                >
                  {{ t("runtime.actions.retry") }}
                </button>
              </div>
            </div>
          </section>
        </div>

        <aside class="runtime-inspector">
          <div class="runtime-inspector-header">
            <div>
              <p class="section-label">{{ t("runtime.host.sections.logs") }}</p>
              <h2 class="runtime-block-title">{{ t("runtime.logs") }}</h2>
              <p class="runtime-block-summary">{{ t("runtime.logsSummary") }}</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <button
                class="secondary-action spotlight-surface"
                :aria-busy="runtimeConsole.isRuntimeActionPending('refreshLogs') ? 'true' : undefined"
                :data-pending="runtimeConsole.isRuntimeActionPending('refreshLogs') ? 'true' : undefined"
                :disabled="runtimeConsole.busy.value"
                @click="runtimeConsole.refreshLogs()"
              >
                <RotateCcw :size="14" />
                {{ t("runtime.actions.refreshLogs") }}
              </button>
              <button
                class="secondary-action spotlight-surface"
                :aria-busy="runtimeConsole.isRuntimeActionPending('openLogDirectory') ? 'true' : undefined"
                :data-pending="runtimeConsole.isRuntimeActionPending('openLogDirectory') ? 'true' : undefined"
                :disabled="runtimeConsole.busy.value || !runtimeConsole.canOpenLogDirectory.value"
                @click="runtimeConsole.openLogDirectory"
              >
                <FileText :size="14" />
                {{ t("runtime.actions.openLogDirectory") }}
              </button>
            </div>
          </div>

          <div v-if="runtimeConsole.visibleLogs.value.length === 0" class="runtime-log-empty">
            <Activity :size="16" />
            <span>{{ t("runtime.emptyLogs") }}</span>
          </div>

          <div v-else class="runtime-log-list runtime-log-list-inspector">
            <article
              v-for="entry in runtimeConsole.visibleLogs.value"
              :key="`${entry.timestamp}-${entry.component}-${entry.message}`"
              class="runtime-log-row"
            >
              <div class="runtime-log-meta">
                <span :class="runtimeConsole.logLevelClass(entry.level)">
                  {{ runtimeConsole.formatLogLevel(entry.level) }}
                </span>
                <span>{{ formatDateTime(entry.timestamp) }}</span>
                <span>{{ entry.component }}</span>
              </div>
              <p class="runtime-log-message">{{ entry.message }}</p>
              <pre v-if="runtimeConsole.formatFields(entry.fields)" class="runtime-log-fields">{{
                runtimeConsole.formatFields(entry.fields)
              }}</pre>
            </article>
          </div>
        </aside>
      </div>
    </div>

    <div v-else class="empty-state">{{ t("runtime.loading") }}</div>
  </section>
</template>
