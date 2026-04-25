<script setup lang="ts">
import { Check, ChevronDown, X } from "@lucide/vue";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

export interface AppSelectOption {
  value: string;
  label: string;
  description?: string;
  disabled?: boolean;
}

type AppSelectOptionInput = string | AppSelectOption;
type SelectModelValue = string | string[];

const props = withDefaults(
  defineProps<{
    ariaLabel?: string;
    clearable?: boolean;
    disabled?: boolean;
    labelFor?: (value: string) => string;
    modelValue: SelectModelValue;
    multiple?: boolean;
    options: readonly AppSelectOptionInput[];
    placeholder?: string;
    size?: "default" | "compact";
    variant?: "default" | "quiet";
  }>(),
  {
    ariaLabel: undefined,
    clearable: false,
    disabled: false,
    labelFor: undefined,
    multiple: false,
    placeholder: "",
    size: "default",
    variant: "default",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: SelectModelValue];
}>();

const { t } = useI18n({ useScope: "global" });
const rootEl = ref<HTMLElement | null>(null);
const isOpen = ref(false);
const activeIndex = ref(0);
const listboxId = `app-select-${Math.random().toString(36).slice(2)}`;

const normalizedOptions = computed<AppSelectOption[]>(() =>
  props.options.map((option) => {
    if (typeof option === "string") {
      return {
        value: option,
        label: props.labelFor?.(option) ?? option,
      };
    }
    return option;
  }),
);

const selectedValues = computed(() => {
  if (Array.isArray(props.modelValue)) {
    return props.modelValue.filter((value) => value !== "");
  }

  return props.modelValue ? [props.modelValue] : [];
});

const selectableOptions = computed(() =>
  normalizedOptions.value.filter((option) => !option.disabled && option.value !== ""),
);

const selectableValues = computed(() => selectableOptions.value.map((option) => option.value));

const isAllSelected = computed(() =>
  selectableValues.value.length > 0 && selectableValues.value.every((value) => selectedValues.value.includes(value)),
);

const selectedOptions = computed(() => {
  if (props.multiple) {
    return selectedValues.value.map((value) => {
      return normalizedOptions.value.find((option) => option.value === value) ?? { value, label: value };
    });
  }

  const value = Array.isArray(props.modelValue) ? props.modelValue[0] ?? "" : props.modelValue;
  const option = normalizedOptions.value.find((item) => item.value === value);
  if (option) {
    return [option];
  }

  return value ? [{ value, label: value }] : [];
});

const hasSelection = computed(() => {
  if (Array.isArray(props.modelValue)) {
    return props.modelValue.some((value) => value !== "");
  }
  return props.modelValue !== "";
});

const displayLabel = computed(() => {
  if (selectedOptions.value.length === 0) {
    return props.placeholder;
  }

  if (props.multiple && selectedOptions.value.length > 1) {
    return t("common.select.selectedCount", { count: selectedOptions.value.length });
  }

  return selectedOptions.value[0]?.label ?? props.placeholder;
});

const triggerLabel = computed(() => props.ariaLabel ?? props.placeholder ?? displayLabel.value);

function optionId(index: number) {
  return `${listboxId}-option-${index}`;
}

function isSelected(value: string) {
  if (!props.multiple && value === "" && !hasSelection.value) {
    return true;
  }
  return selectedValues.value.includes(value);
}

function firstEnabledIndex() {
  const selectedIndex = normalizedOptions.value.findIndex((option) => !option.disabled && isSelected(option.value));
  if (selectedIndex >= 0) {
    return selectedIndex;
  }

  const enabledIndex = normalizedOptions.value.findIndex((option) => !option.disabled);
  return enabledIndex >= 0 ? enabledIndex : 0;
}

function openMenu() {
  if (props.disabled) {
    return;
  }
  activeIndex.value = firstEnabledIndex();
  isOpen.value = true;
}

function closeMenu() {
  isOpen.value = false;
}

function toggleMenu() {
  if (isOpen.value) {
    closeMenu();
    return;
  }
  openMenu();
}

function moveActive(step: number) {
  const options = normalizedOptions.value;
  if (options.length === 0) {
    return;
  }

  let nextIndex = activeIndex.value;
  for (let attempts = 0; attempts < options.length; attempts += 1) {
    nextIndex = (nextIndex + step + options.length) % options.length;
    if (!options[nextIndex]?.disabled) {
      activeIndex.value = nextIndex;
      return;
    }
  }
}

function setActiveIndex(index: number, option: AppSelectOption) {
  if (!option.disabled) {
    activeIndex.value = index;
  }
}

function chooseOption(option: AppSelectOption) {
  if (option.disabled) {
    return;
  }

  if (props.multiple) {
    const nextValues = new Set(selectedValues.value);
    if (nextValues.has(option.value)) {
      nextValues.delete(option.value);
    } else {
      nextValues.add(option.value);
    }
    emit("update:modelValue", Array.from(nextValues));
    return;
  }

  emit("update:modelValue", option.value);
  closeMenu();
}

function clearSelection() {
  emit("update:modelValue", props.multiple ? [] : "");
}

function selectAllOptions() {
  emit("update:modelValue", selectableValues.value);
}

function clearAllOptions() {
  emit("update:modelValue", []);
}

function handleTriggerKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown") {
    event.preventDefault();
    if (!isOpen.value) {
      openMenu();
      return;
    }
    moveActive(1);
    return;
  }

  if (event.key === "ArrowUp") {
    event.preventDefault();
    if (!isOpen.value) {
      openMenu();
      return;
    }
    moveActive(-1);
    return;
  }

  if (event.key === "Home") {
    event.preventDefault();
    activeIndex.value = firstEnabledIndex();
    return;
  }

  if (event.key === "End") {
    event.preventDefault();
    const reversedIndex = [...normalizedOptions.value].reverse().findIndex((option) => !option.disabled);
    if (reversedIndex >= 0) {
      activeIndex.value = normalizedOptions.value.length - 1 - reversedIndex;
    }
    return;
  }

  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    if (!isOpen.value) {
      openMenu();
      return;
    }

    const option = normalizedOptions.value[activeIndex.value];
    if (option) {
      chooseOption(option);
    }
    return;
  }

  if (event.key === "Escape" || event.key === "Tab") {
    closeMenu();
  }
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (!isOpen.value || !rootEl.value) {
    return;
  }
  if (!rootEl.value.contains(event.target as Node)) {
    closeMenu();
  }
}

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
});
</script>

<template>
  <div
    ref="rootEl"
    class="app-select"
    :class="[
      `app-select-${variant}`,
      `app-select-${size}`,
      {
        'app-select-disabled': disabled,
        'app-select-has-clear': clearable && hasSelection,
        'app-select-multiple': multiple,
        'app-select-open': isOpen,
      },
    ]"
  >
    <div class="app-select-trigger-shell">
      <button
        class="app-select-trigger"
        type="button"
        :aria-activedescendant="isOpen ? optionId(activeIndex) : undefined"
        :aria-controls="listboxId"
        :aria-expanded="isOpen"
        :aria-haspopup="'listbox'"
        :aria-label="triggerLabel"
        :disabled="disabled"
        @click="toggleMenu"
        @keydown="handleTriggerKeydown"
      >
        <span
          class="app-select-value"
          :class="{ 'app-select-placeholder': selectedOptions.length === 0 }"
        >
          {{ displayLabel }}
        </span>
        <ChevronDown :size="15" aria-hidden="true" class="app-select-chevron" />
      </button>
      <button
        v-if="clearable && hasSelection"
        class="app-select-clear"
        type="button"
        :aria-label="t('common.select.clear')"
        @click.stop="clearSelection"
      >
        <X :size="13" aria-hidden="true" />
      </button>
    </div>

    <div v-if="isOpen" class="app-select-menu">
      <div v-if="multiple && selectableOptions.length > 0" class="app-select-bulk-actions">
        <button
          class="app-select-bulk-button"
          type="button"
          :disabled="isAllSelected"
          @click="selectAllOptions"
        >
          {{ t("common.select.selectAll") }}
        </button>
        <button
          class="app-select-bulk-button"
          type="button"
          :disabled="!hasSelection"
          @click="clearAllOptions"
        >
          {{ t("common.select.clearAll") }}
        </button>
      </div>

      <div
        :id="listboxId"
        class="app-select-options"
        role="listbox"
        :aria-multiselectable="multiple ? 'true' : undefined"
      >
        <button
          v-for="(option, index) in normalizedOptions"
          :id="optionId(index)"
          :key="option.value"
          class="app-select-option"
          :class="{
            'app-select-option-active': activeIndex === index,
            'app-select-option-selected': isSelected(option.value),
          }"
          :aria-selected="isSelected(option.value)"
          :disabled="option.disabled"
          role="option"
          type="button"
          @click="chooseOption(option)"
          @mousemove="setActiveIndex(index, option)"
        >
          <span class="app-select-check" aria-hidden="true">
            <Check v-if="isSelected(option.value)" :size="13" />
          </span>
          <span class="app-select-option-copy">
            <span class="app-select-option-label">{{ option.label }}</span>
            <span v-if="option.description" class="app-select-option-description">
              {{ option.description }}
            </span>
          </span>
        </button>

        <div v-if="normalizedOptions.length === 0" class="app-select-empty">
          {{ t("common.select.noOptions") }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app-select {
  position: relative;
  width: 100%;
  min-width: 0;
  max-width: 100%;
}

.app-select-trigger-shell {
  position: relative;
}

.app-select-trigger {
  display: flex;
  width: 100%;
  min-height: 36px;
  min-width: 0;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 0 10px 0 12px;
  color: var(--text-main);
  font-size: 13px;
  line-height: 1.35;
  background: var(--accent-soft);
  box-shadow: 0 0 0 1px transparent;
  text-align: left;
  transition:
    background 140ms ease,
    border-color 140ms ease,
    box-shadow 140ms ease,
    color 140ms ease;
}

.app-select-trigger:hover {
  border-color: color-mix(in srgb, var(--text-muted) 20%, var(--border-color));
  background: color-mix(in srgb, var(--accent-soft) 88%, var(--bg-panel));
}

.app-select-trigger:focus-visible {
  outline: none;
  border-color: var(--text-muted);
  box-shadow:
    inset 0 1px 0 var(--border-highlight),
    0 0 0 2px color-mix(in srgb, var(--text-muted) 14%, transparent);
}

.app-select-compact .app-select-trigger {
  min-height: 32px;
  padding-right: 8px;
  padding-left: 10px;
  font-size: 13px;
}

.app-select-quiet .app-select-trigger {
  min-height: 32px;
  padding: 0 0 10px;
  border: 0;
  border-bottom: 1px solid var(--border-color);
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.app-select-quiet .app-select-trigger:hover {
  border-bottom-color: color-mix(in srgb, var(--text-muted) 34%, var(--border-color));
  background: transparent;
}

.app-select-quiet .app-select-trigger:focus-visible {
  border-bottom-color: var(--text-main);
  box-shadow: 0 1px 0 color-mix(in srgb, var(--text-main) 24%, transparent);
}

.app-select-quiet.app-select-compact .app-select-trigger {
  padding-bottom: 8px;
}

.app-select-has-clear .app-select-trigger {
  padding-right: 54px;
}

.app-select-value {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-select-placeholder {
  color: color-mix(in srgb, var(--text-muted) 82%, transparent);
}

.app-select-chevron {
  flex: 0 0 auto;
  color: var(--text-muted);
  transition: transform 140ms ease;
}

.app-select-open .app-select-chevron {
  transform: rotate(180deg);
}

.app-select-clear {
  position: absolute;
  top: 50%;
  right: 28px;
  display: inline-flex;
  width: 18px;
  height: 18px;
  transform: translateY(-50%);
  align-items: center;
  justify-content: center;
  border-radius: 5px;
  color: var(--text-muted);
  transition:
    background 140ms ease,
    color 140ms ease;
}

.app-select-clear:hover {
  color: var(--text-main);
  background: color-mix(in srgb, var(--accent-soft) 74%, transparent);
}

.app-select-menu {
  position: absolute;
  z-index: 80;
  top: calc(100% + 6px);
  right: 0;
  left: 0;
  display: grid;
  min-height: 0;
  max-height: 260px;
  gap: 4px;
  overflow: hidden;
  padding: 4px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: color-mix(in srgb, var(--bg-panel) 94%, transparent);
  box-shadow:
    inset 0 1px 0 var(--border-highlight),
    0 18px 46px color-mix(in srgb, var(--shadow-color) 18%, transparent);
  backdrop-filter: blur(18px);
  animation: app-select-enter 140ms ease;
}

.app-select-options {
  display: grid;
  min-height: 0;
  gap: 2px;
  overflow-y: auto;
}

.app-select-bulk-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 0 4px;
  border-bottom: 1px solid color-mix(in srgb, var(--border-color) 72%, transparent);
}

.app-select-bulk-button {
  display: inline-flex;
  min-height: 28px;
  flex: 1 1 0;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: 6px;
  padding: 0 8px;
  color: var(--text-muted);
  font-size: 12px;
  transition:
    background 120ms ease,
    border-color 120ms ease,
    color 120ms ease;
}

.app-select-bulk-button:not(:disabled):hover,
.app-select-bulk-button:focus-visible {
  outline: none;
  color: var(--text-main);
  border-color: color-mix(in srgb, var(--text-muted) 12%, var(--border-color));
  background: color-mix(in srgb, var(--accent-soft) 78%, transparent);
}

.app-select-bulk-button:disabled {
  opacity: 0.46;
}

html[data-theme="dark"] .app-select-menu {
  background: color-mix(in srgb, var(--bg-elevated) 96%, transparent);
  box-shadow:
    inset 0 1px 0 var(--border-highlight),
    0 20px 52px rgba(0, 0, 0, 0.36);
}

.app-select-option {
  display: flex;
  width: 100%;
  min-height: 32px;
  align-items: center;
  gap: 8px;
  border: 1px solid transparent;
  border-radius: 6px;
  padding: 6px 8px;
  color: var(--text-muted);
  text-align: left;
  transition:
    background 120ms ease,
    border-color 120ms ease,
    color 120ms ease;
}

.app-select-option:not(:disabled):hover,
.app-select-option-active {
  color: var(--text-main);
  border-color: color-mix(in srgb, var(--text-muted) 12%, var(--border-color));
  background: color-mix(in srgb, var(--accent-soft) 78%, transparent);
}

.app-select-option-selected {
  color: var(--text-main);
  border-color: color-mix(in srgb, var(--accent-color) 14%, var(--border-color));
  background: linear-gradient(180deg, var(--accent-soft), color-mix(in srgb, var(--accent-soft) 52%, transparent));
  box-shadow: inset 0 1px 0 var(--border-highlight);
}

.app-select-option:disabled {
  opacity: 0.48;
}

.app-select-check {
  display: inline-flex;
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  color: var(--text-main);
}

.app-select-multiple .app-select-check {
  border: 1px solid color-mix(in srgb, var(--text-muted) 18%, var(--border-color));
  border-radius: 4px;
}

.app-select-multiple .app-select-option-selected .app-select-check {
  border-color: color-mix(in srgb, var(--text-main) 34%, var(--border-color));
  background: color-mix(in srgb, var(--text-main) 9%, transparent);
}

.app-select-option-copy {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.app-select-option-label,
.app-select-option-description {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-select-option-label {
  font-size: 13px;
  line-height: 1.25;
}

.app-select-option-description {
  font-size: 11px;
  color: var(--text-muted);
}

.app-select-empty {
  padding: 10px 8px;
  color: var(--text-muted);
  font-size: 12px;
  text-align: center;
}

.app-select-disabled {
  opacity: 0.58;
}

@keyframes app-select-enter {
  from {
    opacity: 0;
    transform: translateY(-3px);
  }

  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .app-select-menu,
  .app-select-chevron {
    animation: none;
    transition: none;
  }
}
</style>
