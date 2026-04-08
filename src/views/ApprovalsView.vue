<script setup lang="ts">
import { BadgeCheck, ShieldCheck, ShieldOff, Sparkles } from "@lucide/vue";
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import JsonBlock from "../components/JsonBlock.vue";
import { formatDesktopError } from "../lib/errorMessage";
import { coerceString, formatDateTime } from "../lib/format";
import { approvalStatusOptions } from "../lib/options";
import type { ApprovalRequest, ApprovalStatus } from "../lib/types";
import { useApprovalsStore } from "../stores/approvals";
import { useShellStore } from "../stores/shell";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const approvalsStore = useApprovalsStore();
const { t } = useI18n({ useScope: "global" });

const reviewForm = reactive({
  reviewed_by: "desktop",
  review_note: "",
});

const selectedState = computed(() => coerceString(route.query.approvalState as string | undefined));
const selectedRequestId = computed(() => coerceString(route.query.request as string | undefined));
const selectedRequest = ref<ApprovalRequest | null>(null);

watch(
  selectedState,
  async (state) => {
    await approvalsStore.loadApprovals((state || undefined) as ApprovalStatus | undefined);
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

    const firstRequest = approvalsStore.approvals[0];
    const hasSelection = firstRequest
      ? approvalsStore.approvals.some((item) => item.request_id === selectedRequestId.value)
      : false;

    if (!firstRequest) {
      return;
    }

    if (!hasSelection) {
      await router.replace({
        path: "/approvals",
        query: {
          ...route.query,
          request: firstRequest.request_id,
        },
      });
    }
  },
  { immediate: true },
);

onMounted(async () => {
  await approvalsStore.refreshPendingCount();
  await approvalsStore.loadApprovals((selectedState.value || undefined) as ApprovalStatus | undefined);
  const fallbackRequest = selectedRequest.value ?? approvalsStore.approvals[0] ?? null;
  if (!selectedRequest.value && fallbackRequest) {
    await router.replace({
      path: "/approvals",
      query: {
        ...route.query,
        request: fallbackRequest.request_id,
      },
    });
  }
});

async function setState(state?: ApprovalStatus) {
  await router.push({
    path: "/approvals",
    query: {
      approvalState: state,
      request: undefined,
    },
  });
}

async function selectRequest(requestId: string) {
  await router.push({
    path: "/approvals",
    query: {
      approvalState: selectedState.value || undefined,
      request: requestId,
    },
  });
}

async function approveRequest() {
  if (!selectedRequest.value) {
    return;
  }
  try {
    selectedRequest.value = await approvalsStore.approve(selectedRequest.value.request_id, reviewForm);
    await approvalsStore.loadApprovals((selectedState.value || undefined) as ApprovalStatus | undefined);
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
    await approvalsStore.loadApprovals((selectedState.value || undefined) as ApprovalStatus | undefined);
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
  if (request.action.startsWith("project.") || request.action.startsWith("version.")) {
    const result = (request.result_json ?? {}) as Record<string, unknown>;
    await router.push({
      path: "/projects",
      query: {
        project: (result.slug as string | undefined) ?? request.resource_ref,
        version: result.version_id as string | undefined,
      },
    });
    return;
  }

  if (
    request.action.startsWith("task.") ||
    request.action === "note.create" ||
    request.action === "attachment.create"
  ) {
    const result = (request.result_json ?? {}) as Record<string, unknown>;
    await router.push({
      path: "/tasks",
      query: {
        task: (result.task_id as string | undefined) ?? request.resource_ref,
      },
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
  <section class="page-grid">
    <aside class="page-list px-4 py-5">
      <div class="mb-4 flex items-center justify-between">
        <div>
          <p class="section-label">{{ t("approvals.listKicker") }}</p>
          <h1 class="text-lg font-semibold text-[var(--text-main)]">{{ t("approvals.listTitle") }}</h1>
        </div>
        <span class="status-pill status-pill-warning">{{ approvalsStore.pendingCount }}</span>
      </div>

      <div class="mb-4 flex flex-wrap gap-2">
        <button class="secondary-action spotlight-surface" @click="setState(undefined)">{{ t("common.all") }}</button>
        <button
          v-for="status in approvalStatusOptions"
          :key="status"
          class="secondary-action spotlight-surface"
          @click="setState(status)"
        >
          {{ t(`status.approval.${status}`) }}
        </button>
      </div>

      <div v-if="approvalsStore.approvals.length === 0" class="empty-state">
        {{ t("approvals.emptyList") }}
      </div>
      <div v-else>
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
          </div>
          <span :class="statusClass(request.status)">{{ t(`status.approval.${request.status}`) }}</span>
        </button>
      </div>
    </aside>

    <div class="page-detail">
      <div v-if="selectedRequest" class="space-y-5">
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
            <span :class="statusClass(selectedRequest.status)">{{ t(`status.approval.${selectedRequest.status}`) }}</span>
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
                  <dt class="text-[var(--text-muted)]">{{ t("approvals.resourceRef") }}</dt>
                  <dd>{{ selectedRequest.resource_ref }}</dd>
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
