import { ref } from "vue";
import { defineStore } from "pinia";

import { desktopBridge } from "../lib/desktop";
import type { ApprovalRequest, ApprovalStatus } from "../lib/types";

export const useApprovalsStore = defineStore("approvals", () => {
  const approvals = ref<ApprovalRequest[]>([]);
  const loading = ref(false);
  const pendingCount = ref(0);

  async function loadApprovals(status?: ApprovalStatus) {
    loading.value = true;
    try {
      const envelope = await desktopBridge.approval({
        action: "list",
        status,
      });
      approvals.value = envelope.result as ApprovalRequest[];
      return approvals.value;
    } finally {
      loading.value = false;
    }
  }

  async function loadApproval(requestId: string) {
    const envelope = await desktopBridge.approval({
      action: "get",
      request_id: requestId,
    });
    return envelope.result as ApprovalRequest;
  }

  async function refreshPendingCount() {
    const items = await loadApprovals("pending");
    pendingCount.value = items.length;
    return pendingCount.value;
  }

  async function approve(requestId: string, payload: Record<string, unknown>) {
    const envelope = await desktopBridge.approval({
      action: "approve",
      request_id: requestId,
      ...payload,
    });
    return envelope.result as ApprovalRequest;
  }

  async function deny(requestId: string, payload: Record<string, unknown>) {
    const envelope = await desktopBridge.approval({
      action: "deny",
      request_id: requestId,
      ...payload,
    });
    return envelope.result as ApprovalRequest;
  }

  return {
    approvals,
    approve,
    deny,
    loadApproval,
    loadApprovals,
    loading,
    pendingCount,
    refreshPendingCount,
  };
});
