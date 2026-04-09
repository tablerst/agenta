<script setup lang="ts">
import { BadgeCheck, ShieldCheck, ShieldOff, Sparkles } from "@lucide/vue";
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import JsonBlock from "../components/JsonBlock.vue";
import { formatDesktopError } from "../lib/errorMessage";
import { formatDateTime } from "../lib/format";
import { approvalScopeOptions, approvalStatusOptions } from "../lib/options";
import { buildProjectWorkspacePath, mergeWorkspaceQuery, readRouteString } from "../lib/projectWorkspace";
import type { ApprovalRequest, ApprovalScope, ApprovalStatus } from "../lib/types";
import { useApprovalsStore } from "../stores/approvals";
import { useProjectsStore } from "../stores/projects";
import { useShellStore } from "../stores/shell";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const approvalsStore = useApprovalsStore();
const projectsStore = useProjectsStore();
const { t } = useI18n({ useScope: "global" });

const reviewForm = reactive({
  review_note: "",
  reviewed_by: "desktop",
});

const selectedProjectSlug = computed(() => String(route.params.projectSlug ?? ""));
const selectedProject = computed(
  () => projectsStore.projects.find((item) => item.slug === selectedProjectSlug.value) ?? null,
);
const selectedScope = computed<ApprovalScope>(() =>
  readRouteString(route.query.approvalScope) === "all" ? "all" : "project",
);
const selectedState = computed(() => readRouteString(route.query.approvalState) ?? "");
const selectedRequestId = computed(() => readRouteString(route.query.request) ?? "");
const selectedRequest = ref<ApprovalRequest | null>(null);

function scopeLabel(scope: ApprovalScope) {
  if (scope === "all") {
    return t("approvals.scope.all");
  }
  return t("approvals.scope.project", {
    project: selectedProject.value?.name || selectedProjectSlug.value || t("common.resource"),
  });
}

function queryValue(
  overrides: Record<"approvalScope" | "approvalState" | "request", string | undefined>,
  key: "approvalScope" | "approvalState" | "request",
  current: string,
) {
  return key in overrides ? overrides[key] : current || undefined;
}

function buildApprovalsQuery(
  overrides: Partial<Record<"approvalScope" | "approvalState" | "request", string | undefined>> = {},
) {
  return mergeWorkspaceQuery({
    approvalScope: queryValue(
      overrides as Record<"approvalScope" | "approvalState" | "request", string | undefined>,
      "approvalScope",
      selectedScope.value,
    ),
    approvalState: queryValue(
      overrides as Record<"approvalScope" | "approvalState" | "request", string | undefined>,
      "approvalState",
      selectedState.value,
    ),
    request: queryValue(
      overrides as Record<"approvalScope" | "approvalState" | "request", string | undefined>,
      "request",
      selectedRequestId.value,
    ),
  });
}

async function updateQuery(
  overrides: Partial<Record<"approvalScope" | "approvalState" | "request", string | undefined>>,
  replace = false,
) {
  const targetProjectSlug =
    selectedProjectSlug.value || selectedRequest.value?.project_ref || projectsStore.projects[0]?.slug;

  if (!targetProjectSlug) {
    await router.replace({ path: "/projects" });
    return;
  }

  const location = {
    path: buildProjectWorkspacePath(targetProjectSlug, "approvals"),
    query: buildApprovalsQuery(overrides),
  };

  if (replace) {
    await router.replace(location);
    return;
  }

  await router.push(location);
}

function currentFilters() {
  return {
      project:
      selectedScope.value === "project"
        ? selectedProject.value?.slug || selectedProjectSlug.value || undefined
        : undefined,
    status: (selectedState.value || undefined) as ApprovalStatus | undefined,
  };
}

watch(
  () => [selectedScope.value, selectedState.value, selectedProject.value?.slug] as const,
  async () => {
    await approvalsStore.loadApprovals(currentFilters());
  },
  { immediate: true },
);

watch(
  selectedRequestId,
  async (requestId) => {
    if (!requestId) {
      selectedRequest.value = null;
      return;
    }

    selectedRequest.value = await approvalsStore.loadApproval(requestId);
  },
  { immediate: true },
);

watch(
  () => approvalsStore.approvals.map((item) => item.request_id).join("|"),
  async () => {
    if (approvalsStore.loading) {
      return;
    }

    const firstRequest = approvalsStore.approvals[0] ?? null;
    const hasSelection = Boolean(
      selectedRequestId.value &&
      approvalsStore.approvals.some((item) => item.request_id === selectedRequestId.value),
    );

    if (!firstRequest) {
      if (selectedRequestId.value) {
        selectedRequest.value = null;
        await updateQuery({ request: undefined }, true);
      }
      return;
    }

    if (!hasSelection) {
      await updateQuery({ request: firstRequest.request_id }, true);
    }
  },
  { immediate: true },
);

onMounted(async () => {
  await Promise.allSettled([projectsStore.loadProjects(), approvalsStore.refreshPendingCount()]);
});

async function setState(state?: ApprovalStatus) {
  await updateQuery({
    approvalState: state,
    request: undefined,
  });
}

async function setScope(scope: ApprovalScope) {
  await updateQuery({
    approvalScope: scope,
    request: undefined,
  });
}

async function selectRequest(requestId: string) {
  await updateQuery({ request: requestId });
}

async function approveRequest() {
  if (!selectedRequest.value) {
    return;
  }

  try {
    selectedRequest.value = await approvalsStore.approve(selectedRequest.value.request_id, reviewForm);
    await approvalsStore.loadApprovals(currentFilters());
    await approvalsStore.refreshPendingCount();
    shell.pushNotice("success", t("notices.approvalProcessed"));
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function denyRequest() {
  if (!selectedRequest.value) {
    return;
  }

  try {
    selectedRequest.value = await approvalsStore.deny(selectedRequest.value.request_id, reviewForm);
    await approvalsStore.loadApprovals(currentFilters());
    await approvalsStore.refreshPendingCount();
    shell.pushNotice("success", t("notices.requestDenied"));
  } catch (error) {
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function jumpToResource() {
  if (!selectedRequest.value) {
    return;
  }

  const request = selectedRequest.value;
  const result = (request.result_json ?? {}) as Record<string, unknown>;
  const projectSlug =
    request.project_ref ??
    selectedProjectSlug.value ??
    projectsStore.projects[0]?.slug;

  if (!projectSlug) {
    return;
  }

  if (request.action.startsWith("project.")) {
    await router.push({
      path: buildProjectWorkspacePath(projectSlug, "overview"),
    });
    return;
  }

  if (request.action.startsWith("version.")) {
    await router.push({
      path: buildProjectWorkspacePath(projectSlug, "versions"),
      query: mergeWorkspaceQuery({
        version:
          (typeof result.version_id === "string" && result.version_id) ||
          request.resource_ref,
      }),
    });
    return;
  }

  if (
    request.action.startsWith("task.") ||
    request.action === "note.create" ||
    request.action === "attachment.create"
  ) {
    const taskRef =
      request.task_ref ||
      (typeof result.task_id === "string" ? result.task_id : undefined) ||
      (request.action.startsWith("task.") ? request.resource_ref : undefined);

    await router.push({
      path: buildProjectWorkspacePath(projectSlug, "tasks"),
      query: mergeWorkspaceQuery({ task: taskRef }),
    });
  }
}

function statusClass(status: ApprovalStatus) {
  switch (status) {
    case "approved":
      return "status-pill status-pill-success";
    case "denied":
      return "status-pill status-pill-danger";
    case "failed":
      return "status-pill status-pill-danger";
    default:
      return "status-pill status-pill-warning";
  }
}
</script>

<template>
  <section class="workspace-section-grid">
    <aside class="workspace-list-pane">
      <div class="workspace-pane-stack">
        <section class="panel-section">
          <div class="mb-4 flex items-center justify-between gap-3">
            <div>
              <p class="section-label">{{ t("approvals.listKicker") }}</p>
              <h2 class="text-lg font-semibold text-[var(--text-main)]">{{ t("approvals.listTitle") }}</h2>
            </div>
            <span class="status-pill status-pill-warning">{{ approvalsStore.pendingCount }}</span>
          </div>

          <div class="space-y-4">
            <div>
              <p class="field-label mb-2">{{ t("approvals.scopeLabel") }}</p>
              <div class="filter-chip-group" role="group" :aria-label="t('approvals.scopeLabel')">
                <button
                  v-for="scope in approvalScopeOptions"
                  :key="scope"
                  class="filter-chip spotlight-surface"
                  :class="{ 'filter-chip-active': selectedScope === scope }"
                  type="button"
                  @click="setScope(scope)"
                >
                  {{ scopeLabel(scope) }}
                </button>
              </div>
            </div>

            <div>
              <p class="field-label mb-2">{{ t("approvals.stateLabel") }}</p>
              <div class="filter-chip-group" role="group" :aria-label="t('approvals.stateLabel')">
                <button
                  class="filter-chip spotlight-surface"
                  :class="{ 'filter-chip-active': !selectedState }"
                  type="button"
                  @click="setState(undefined)"
                >
                  {{ t("common.all") }}
                </button>
                <button
                  v-for="status in approvalStatusOptions"
                  :key="status"
                  class="filter-chip spotlight-surface"
                  :class="{ 'filter-chip-active': selectedState === status }"
                  type="button"
                  @click="setState(status)"
                >
                  {{ t(`status.approval.${status}`) }}
                </button>
              </div>
            </div>
          </div>
        </section>

        <section class="glass-panel p-5">
          <div v-if="approvalsStore.approvals.length === 0" class="empty-state">
            {{ t("approvals.emptyList") }}
          </div>
          <div v-else class="workspace-list-stack">
            <button
              v-for="request in approvalsStore.approvals"
              :key="request.request_id"
              v-spotlight
              class="list-row spotlight-surface"
              :class="{ 'list-row-active': selectedRequest?.request_id === request.request_id }"
              @click="selectRequest(request.request_id)"
            >
              <ShieldCheck :size="16" />
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm font-medium text-[var(--text-main)]">
                  {{ request.request_summary }}
                </p>
                <p class="truncate text-xs text-[var(--text-muted)]">{{ request.action }}</p>
                <p class="truncate text-[11px] text-[var(--text-muted)]">
                  {{
                    selectedScope === "all" || !selectedProject
                      ? request.project_name || request.project_ref || t("common.na")
                      : request.project_name || selectedProject?.name || selectedProjectSlug
                  }}
                </p>
              </div>
              <span :class="statusClass(request.status)">{{ t(`status.approval.${request.status}`) }}</span>
            </button>
          </div>
        </section>
      </div>
    </aside>

    <div class="workspace-inspector-pane">
      <div v-if="selectedRequest" class="workspace-pane-stack">
        <section class="glass-panel p-5">
          <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
            <div>
              <p class="section-label">{{ t("approvals.inspector") }}</p>
              <h2 class="text-2xl font-semibold text-[var(--text-main)]">
                {{ selectedRequest.request_summary }}
              </h2>
              <p class="mt-2 text-sm text-[var(--text-muted)]">
                {{
                  t("approvals.via", {
                    action: selectedRequest.action,
                    channel: t(`approval.requestedVia.${selectedRequest.requested_via}`),
                  })
                }}
              </p>
            </div>
            <span :class="statusClass(selectedRequest.status)">
              {{ t(`status.approval.${selectedRequest.status}`) }}
            </span>
          </div>

          <div class="grid gap-4 md:grid-cols-2">
            <section class="panel-section">
              <p class="section-label">{{ t("approvals.request") }}</p>
              <dl class="space-y-2 text-sm">
                <div>
                  <dt class="text-[var(--text-muted)]">{{ t("approvals.requestedAt") }}</dt>
                  <dd>{{ formatDateTime(selectedRequest.requested_at) }}</dd>
                </div>
                <div>
                  <dt class="text-[var(--text-muted)]">{{ t("approvals.requestedBy") }}</dt>
                  <dd>{{ selectedRequest.requested_by }}</dd>
                </div>
                <div>
                  <dt class="text-[var(--text-muted)]">{{ t("approvals.projectLabel") }}</dt>
                  <dd>{{ selectedRequest.project_name || selectedRequest.project_ref || t("common.na") }}</dd>
                </div>
                <div>
                  <dt class="text-[var(--text-muted)]">{{ t("approvals.resourceRef") }}</dt>
                  <dd>{{ selectedRequest.resource_ref }}</dd>
                </div>
                <div v-if="selectedRequest.task_ref">
                  <dt class="text-[var(--text-muted)]">{{ t("approvals.taskRef") }}</dt>
                  <dd>{{ selectedRequest.task_ref }}</dd>
                </div>
              </dl>
            </section>

            <section class="panel-section">
              <p class="section-label">{{ t("approvals.review") }}</p>
              <div class="space-y-3">
                <input
                  v-model="reviewForm.reviewed_by"
                  class="control-input"
                  :placeholder="t('approvals.placeholders.reviewedBy')"
                />
                <textarea
                  v-model="reviewForm.review_note"
                  class="control-textarea"
                  :placeholder="t('approvals.placeholders.reviewNote')"
                />
                <div class="flex flex-wrap gap-2">
                  <button
                    v-if="selectedRequest.status === 'pending'"
                    class="primary-action spotlight-surface"
                    @click="approveRequest"
                  >
                    <BadgeCheck :size="15" />
                    {{ t("approvals.approve") }}
                  </button>
                  <button
                    v-if="selectedRequest.status === 'pending'"
                    class="secondary-action spotlight-surface"
                    @click="denyRequest"
                  >
                    <ShieldOff :size="15" />
                    {{ t("approvals.deny") }}
                  </button>
                  <button class="secondary-action spotlight-surface" @click="jumpToResource">
                    <Sparkles :size="15" />
                    {{ t("approvals.jumpToResource") }}
                  </button>
                </div>
              </div>
            </section>
          </div>
        </section>

        <section class="grid gap-5 xl:grid-cols-2">
          <section class="panel-section">
            <p class="section-label">{{ t("approvals.payload") }}</p>
            <JsonBlock :value="selectedRequest.payload_json" />
          </section>
          <section class="panel-section">
            <p class="section-label">{{ t("approvals.outcome") }}</p>
            <div v-if="selectedRequest.result_json" class="mb-4">
              <JsonBlock :value="selectedRequest.result_json" />
            </div>
            <div v-if="selectedRequest.error_json">
              <JsonBlock :value="selectedRequest.error_json" />
            </div>
            <div v-if="!selectedRequest.result_json && !selectedRequest.error_json" class="empty-state">
              {{ t("approvals.emptyOutcome") }}
            </div>
          </section>
        </section>
      </div>

      <div v-else class="empty-state">
        {{ t("approvals.emptySelection") }}
      </div>
    </div>
  </section>
</template>
