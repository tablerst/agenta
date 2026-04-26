import { computed, ref } from "vue";
import { defineStore } from "pinia";

import { applyLocale, detectBrowserLocale, normalizeLocale } from "../i18n";
import type { AppLocale, ThemeMode } from "../lib/types";

type NoticeKind = "info" | "success" | "error";

interface Notice {
  kind: NoticeKind;
  message: string;
  sticky: boolean;
}

const DARK_QUERY = "(prefers-color-scheme: dark)";
const COMPACT_QUERY = "(max-width: 1040px)";
const LOCALE_STORAGE_KEY = "agenta.locale";
const THEME_STORAGE_KEY = "agenta.theme";
const SIDEBAR_STORAGE_KEY = "agenta.sidebar_collapsed";

export const useShellStore = defineStore("shell", () => {
  const locale = ref<AppLocale>(detectBrowserLocale());
  const theme = ref<ThemeMode>("system");
  const searchOpen = ref(false);
  const searchTrigger = ref<HTMLElement | null>(null);
  const sidebarCollapsed = ref(false);
  const mobileSidebarOpen = ref(false);
  const isCompactViewport = ref(false);
  const notice = ref<Notice | null>(null);

  let initialized = false;

  const resolvedTheme = computed<"dark" | "light">(() => {
    if (theme.value === "system") {
      return window.matchMedia(DARK_QUERY).matches ? "dark" : "light";
    }
    return theme.value;
  });

  const sidebarCondensed = computed(() => !isCompactViewport.value && sidebarCollapsed.value);

  function applyTheme() {
    document.documentElement.setAttribute("data-theme", resolvedTheme.value);
  }

  function syncViewport() {
    isCompactViewport.value = window.matchMedia(COMPACT_QUERY).matches;
    if (!isCompactViewport.value) {
      mobileSidebarOpen.value = false;
    }
  }

  function persistTheme() {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme.value);
  }

  function persistLocale() {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, locale.value);
  }

  function persistSidebarState() {
    window.localStorage.setItem(SIDEBAR_STORAGE_KEY, String(sidebarCollapsed.value));
  }

  function initialize() {
    if (initialized) {
      syncViewport();
      applyTheme();
      applyLocale(locale.value);
      return;
    }
    initialized = true;

    locale.value = normalizeLocale(window.localStorage.getItem(LOCALE_STORAGE_KEY) ?? detectBrowserLocale());

    const storedTheme = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (storedTheme === "dark" || storedTheme === "light" || storedTheme === "system") {
      theme.value = storedTheme;
    }
    sidebarCollapsed.value = window.localStorage.getItem(SIDEBAR_STORAGE_KEY) === "true";

    syncViewport();
    applyTheme();
    applyLocale(locale.value);

    window.matchMedia(DARK_QUERY).addEventListener("change", () => {
      if (theme.value === "system") {
        applyTheme();
      }
    });
    window.matchMedia(COMPACT_QUERY).addEventListener("change", syncViewport);
  }

  function setTheme(nextTheme: ThemeMode) {
    theme.value = nextTheme;
    persistTheme();
    applyTheme();
  }

  function setLocale(nextLocale: AppLocale) {
    locale.value = normalizeLocale(nextLocale);
    persistLocale();
    applyLocale(locale.value);
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

  function openSearch(trigger?: HTMLElement | null) {
    searchTrigger.value = trigger ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
    searchOpen.value = true;
  }

  function closeSearch() {
    searchOpen.value = false;
  }

  function clearNotice() {
    notice.value = null;
  }

  function restoreSearchFocus() {
    const target = searchTrigger.value;
    searchTrigger.value = null;
    if (target?.isConnected) {
      target.focus({ preventScroll: true });
    }
  }

  function toggleSidebar() {
    if (isCompactViewport.value) {
      mobileSidebarOpen.value = !mobileSidebarOpen.value;
      return;
    }

    sidebarCollapsed.value = !sidebarCollapsed.value;
    persistSidebarState();
  }

  function closeSidebar() {
    mobileSidebarOpen.value = false;
  }

  function pushNotice(kind: NoticeKind, message: string, sticky = kind === "error") {
    notice.value = { kind, message, sticky };
    if (!sticky) {
      window.setTimeout(() => {
        if (notice.value?.message === message) {
          notice.value = null;
        }
      }, 3200);
    }
  }

  return {
    notice,
    locale,
    resolvedTheme,
    searchOpen,
    sidebarCollapsed,
    sidebarCondensed,
    theme,
    mobileSidebarOpen,
    isCompactViewport,
    applyTheme,
    closeSearch,
    closeSidebar,
    clearNotice,
    cycleTheme,
    initialize,
    openSearch,
    pushNotice,
    restoreSearchFocus,
    setLocale,
    setTheme,
    toggleSidebar,
  };
});
