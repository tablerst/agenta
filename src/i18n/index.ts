import { createI18n } from "vue-i18n";

import type { AppLocale } from "../lib/types";
import { messages } from "./messages";

export const fallbackLocale: AppLocale = "en";

export function normalizeLocale(value: string | null | undefined): AppLocale {
  if (!value) {
    return fallbackLocale;
  }

  return value.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

export function detectBrowserLocale(): AppLocale {
  if (typeof navigator === "undefined") {
    return fallbackLocale;
  }

  return normalizeLocale(navigator.languages.find(Boolean) ?? navigator.language);
}

export function resolveLocaleTag(locale: AppLocale): string {
  return locale === "zh-CN" ? "zh-CN" : "en-US";
}

export const i18n = createI18n({
  fallbackLocale,
  legacy: false,
  locale: detectBrowserLocale(),
  messages,
});

export function getCurrentLocale(): AppLocale {
  return normalizeLocale(String(i18n.global.locale.value));
}

export function applyLocale(locale: string | null | undefined): AppLocale {
  const nextLocale = normalizeLocale(locale);
  i18n.global.locale.value = nextLocale;

  if (typeof document !== "undefined") {
    document.documentElement.lang = resolveLocaleTag(nextLocale);
  }

  return nextLocale;
}

applyLocale(getCurrentLocale());
