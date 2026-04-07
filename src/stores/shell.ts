import { computed, ref } from "vue";
import { defineStore } from "pinia";

import type { ThemeMode } from "../lib/types";

type NoticeKind = "info" | "success" | "error";

interface Notice {
  kind: NoticeKind;
  message: string;
}

const DARK_QUERY = "(prefers-color-scheme: dark)";

export const useShellStore = defineStore("shell", () => {
  const theme = ref<ThemeMode>("system");
  const searchOpen = ref(false);
  const sidebarCollapsed = ref(false);
  const notice = ref<Notice | null>(null);

  const resolvedTheme = computed<"dark" | "light">(() => {
    if (theme.value === "system") {
      return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
    }
    return theme.value;
  });

  function applyTheme() {
    document.documentElement.setAttribute("data-theme", resolvedTheme.value);
  }

  function initialize() {
    applyTheme();
    window.matchMedia(DARK_QUERY).addEventListener("change", () => {
      if (theme.value === "system") {
        applyTheme();
      }
    });
  }

  function setTheme(nextTheme: ThemeMode) {
    theme.value = nextTheme;
    applyTheme();
  }

  function cycleTheme() {
    setTheme(
      theme.value === "dark"
        ? "light"
        : theme.value === "light"
          ? "system"
          : "dark",
    );
  }

  function openSearch() {
    searchOpen.value = true;
  }

  function closeSearch() {
    searchOpen.value = false;
  }

  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value;
  }

  function pushNotice(kind: NoticeKind, message: string) {
    notice.value = { kind, message };
    window.setTimeout(() => {
      if (notice.value?.message === message) {
        notice.value = null;
      }
    }, 3200);
  }

  return {
    notice,
    resolvedTheme,
    searchOpen,
    sidebarCollapsed,
    theme,
    applyTheme,
    closeSearch,
    cycleTheme,
    initialize,
    openSearch,
    pushNotice,
    setTheme,
    toggleSidebar,
  };
});
