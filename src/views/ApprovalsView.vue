<script setup lang="ts">
import { BadgeCheck, ShieldCheck, ShieldOff, Sparkles } from "@lucide/vue";
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

import JsonBlock from "../components/JsonBlock.vue";
import { coerceString, formatDateTime } from "../lib/format";
import type { ApprovalRequest, ApprovalStatus } from "../lib/types";
import { useApprovalsStore } from "../stores/approvals";
import { useShellStore } from "../stores/shell";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const approvalsStore = useApprovalsStore();

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

onMounted(async () => {
  await approvalsStore.refreshPendingCount();
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
    shell.pushNotice("success", "Approval processed.");
  } catch (error) {
    shell.pushNotice("error", (error as Error).message);
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
    shell.pushNotice("success", "Request denied.");
  } catch (error) {
    shell.pushNotice("error", (error as Error).message);
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
          <p class="section-label">Approvals</p>
          <h1 class="text-lg font-semibold text-[var(--text-main)]">Human review center</h1>
        </div>
        <span class="status-pill status-pill-warning">{{ approvalsStore.pendingCount }}</span>
      </div>

      <div class="mb-4 flex flex-wrap gap-2">
        <button class="secondary-action spotlight-surface" @click="setState(undefined)">all</button>
        <button class="secondary-action spotlight-surface" @click="setState('pending')">pending</button>
        <button class="secondary-action spotlight-surface" @click="setState('approved')">approved</button>
        <button class="secondary-action spotlight-surface" @click="setState('denied')">denied</button>
        <button class="secondary-action spotlight-surface" @click="setState('failed')">failed</button>
      </div>

      <div v-if="approvalsStore.approvals.length === 0" class="empty-state">
        No approval requests for this state.
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
          <span :class="statusClass(request.status)">{{ request.status }}</span>
        </button>
      </div>
    </aside>

    <div class="page-detail">
      <div v-if="selectedRequest" class="space-y-5">
        <section class="glass-panel p-5">
          <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
            <div>
              <p class="section-label">Approval Inspector</p>
              <h2 class="text-2xl font-semibold text-[var(--text-main)]">
                {{ selectedRequest.request_summary }}
              </h2>
              <p class="mt-2 text-sm text-[var(--text-muted)]">
                {{ selectedRequest.action }} via {{ selectedRequest.requested_via }}
              </p>
            </div>
            <span :class="statusClass(selectedRequest.status)">{{ selectedRequest.status }}</span>
          </div>

          <div class="grid gap-4 md:grid-cols-2">
            <section class="panel-section">
              <p class="section-label">Request</p>
              <dl class="space-y-2 text-sm">
                <div>
                  <dt class="text-[var(--text-muted)]">Requested at</dt>
                  <dd>{{ formatDateTime(selectedRequest.requested_at) }}</dd>
                </div>
                <div>
                  <dt class="text-[var(--text-muted)]">Requested by</dt>
                  <dd>{{ selectedRequest.requested_by }}</dd>
                </div>
                <div>
                  <dt class="text-[var(--text-muted)]">Resource ref</dt>
                  <dd>{{ selectedRequest.resource_ref }}</dd>
                </div>
              </dl>
            </section>

            <section class="panel-section">
              <p class="section-label">Review</p>
              <div class="space-y-3">
                <input v-model="reviewForm.reviewed_by" class="control-input" placeholder="reviewed by" />
                <textarea v-model="reviewForm.review_note" class="control-textarea" placeholder="Review note" />
                <div class="flex flex-wrap gap-2">
                  <button
                    v-if="selectedRequest.status === 'pending'"
                    class="primary-action spotlight-surface"
                    @click="approveRequest"
                  >
                    <BadgeCheck :size="15" />
                    Approve
                  </button>
                  <button
                    v-if="selectedRequest.status === 'pending'"
                    class="secondary-action spotlight-surface"
                    @click="denyRequest"
                  >
                    <ShieldOff :size="15" />
                    Deny
                  </button>
                  <button class="secondary-action spotlight-surface" @click="jumpToResource">
                    <Sparkles :size="15" />
                    Jump to resource
                  </button>
                </div>
              </div>
            </section>
          </div>
        </section>

        <section class="grid gap-5 xl:grid-cols-2">
          <section class="panel-section">
            <p class="section-label">Payload Snapshot</p>
            <JsonBlock :value="selectedRequest.payload_json" />
          </section>
          <section class="panel-section">
            <p class="section-label">Outcome</p>
            <div v-if="selectedRequest.result_json" class="mb-4">
              <JsonBlock :value="selectedRequest.result_json" />
            </div>
            <div v-if="selectedRequest.error_json">
              <JsonBlock :value="selectedRequest.error_json" />
            </div>
            <div v-if="!selectedRequest.result_json && !selectedRequest.error_json" class="empty-state">
              No replay result stored yet.
            </div>
          </section>
        </section>
      </div>

      <div v-else class="empty-state">
        Select an approval request to inspect payload, review note, and replay outcome.
      </div>
    </div>
  </section>
</template>
