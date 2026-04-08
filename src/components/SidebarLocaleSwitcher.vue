<script setup lang="ts">
import { Check, ChevronUp, Globe2 } from "@lucide/vue";
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import { localeOptions } from "../lib/options";
import type { AppLocale } from "../lib/types";
import { useShellStore } from "../stores/shell";

const shell = useShellStore();
const { locale, t } = useI18n({ useScope: "global" });

const menuOpen = ref(false);
const buttonEl = ref<HTMLElement | null>(null);
const flyoutEl = ref<HTMLElement | null>(null);

function getLocaleLabelKey(localeValue: AppLocale) {
  return localeValue === "zh-CN" ? "locale.zh-CN" : "locale.en";
}

const currentLocaleLabel = computed(() => {
  void locale.value;
  return t(getLocaleLabelKey(shell.locale));
});

function closeMenu() {
  menuOpen.value = false;
}

function toggleMenu() {
  menuOpen.value = !menuOpen.value;
}

function selectLocale(nextLocale: AppLocale) {
  shell.setLocale(nextLocale);
  closeMenu();
}

function handlePointerDown(event: MouseEvent) {
  const target = event.target as Node | null;
  if (!target) {
    return;
  }

  if (buttonEl.value?.contains(target) || flyoutEl.value?.contains(target)) {
    return;
  }

  closeMenu();
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeMenu();
  }
}

watch(menuOpen, (isOpen) => {
  if (isOpen) {
    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeydown);
    return;
  }

  document.removeEventListener("mousedown", handlePointerDown);
  document.removeEventListener("keydown", handleKeydown);
});

watch(
  [() => shell.sidebarCondensed, () => shell.mobileSidebarOpen],
  () => {
    closeMenu();
  },
);

onBeforeUnmount(() => {
  document.removeEventListener("mousedown", handlePointerDown);
  document.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <div class="sidebar-setting-menu">
    <button
      ref="buttonEl"
      class="secondary-action sidebar-setting-trigger spotlight-surface"
      :class="{ 'justify-center px-0': shell.sidebarCondensed }"
      :aria-expanded="menuOpen ? 'true' : 'false'"
      :aria-label="t('sidebar.languageLabel', { language: currentLocaleLabel })"
      :title="t('sidebar.languageLabel', { language: currentLocaleLabel })"
      aria-haspopup="menu"
      @click="toggleMenu"
    >
      <Globe2 :size="15" />
      <template v-if="!shell.sidebarCondensed">
        <span class="flex-1 text-left">
          {{ t("sidebar.languageLabel", { language: currentLocaleLabel }) }}
        </span>
        <ChevronUp
          :size="14"
          class="text-[var(--text-muted)] transition-transform"
          :class="{ 'rotate-180': menuOpen }"
        />
      </template>
    </button>

    <div
      v-if="menuOpen"
      ref="flyoutEl"
      class="sidebar-setting-flyout"
      :class="{ 'sidebar-setting-flyout-collapsed': shell.sidebarCondensed }"
      role="menu"
      :aria-label="t('sidebar.languageMenuLabel')"
    >
      <button
        v-for="option in localeOptions"
        :key="option"
        class="sidebar-setting-option spotlight-surface"
        :class="{ 'sidebar-setting-option-active': shell.locale === option }"
        role="menuitemradio"
        :aria-checked="shell.locale === option"
        @click="selectLocale(option)"
      >
        <span class="flex-1 text-left">{{ t(getLocaleLabelKey(option)) }}</span>
        <Check v-if="shell.locale === option" :size="14" />
      </button>
    </div>
  </div>
</template>
