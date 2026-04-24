import { ref } from "vue";
import { defineStore } from "pinia";

import { desktopBridge } from "../lib/desktop";
import type { GlobalSearchFilters, SearchResponse } from "../lib/types";

export const useSearchStore = defineStore("search", () => {
  const query = ref("");
  const loading = ref(false);
  const results = ref<SearchResponse | null>(null);

  async function runSearch(nextQuery: string, filters: GlobalSearchFilters = {}) {
    query.value = nextQuery;
    if (!nextQuery.trim()) {
      results.value = null;
      return null;
    }

    loading.value = true;
    try {
      const envelope = await desktopBridge.search({
        action: "query",
        query: nextQuery,
        limit: 8,
        all_projects: true,
        ...filters,
      });
      results.value = envelope.result as SearchResponse;
      return results.value;
    } finally {
      loading.value = false;
    }
  }

  function clear() {
    query.value = "";
    results.value = null;
  }

  return {
    clear,
    loading,
    query,
    results,
    runSearch,
  };
});
