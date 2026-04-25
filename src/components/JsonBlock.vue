<script setup lang="ts">
import { computed } from "vue";
import VueJsonPretty from "vue-json-pretty";

type JsonData = string | number | boolean | null | JsonData[] | { [key: string]: JsonData };

const props = withDefaults(defineProps<{
  value: unknown;
  deep?: number;
  maxHeight?: string;
  rootPath?: string;
}>(), {
  deep: 2,
  maxHeight: "min(56vh, 560px)",
  rootPath: "root",
});

const data = computed<JsonData>(() => normalizeJsonData(props.value));
const blockStyle = computed(() => ({
  "--json-block-max-height": props.maxHeight,
}));

function normalizeJsonData(value: unknown): JsonData {
  if (value === undefined) {
    return null;
  }

  if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return value;
  }

  if (typeof value === "bigint") {
    return value.toString();
  }

  if (Array.isArray(value)) {
    return value.map((item) => normalizeJsonData(item));
  }

  if (typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, item]) => [key, normalizeJsonData(item)]),
    );
  }

  return String(value);
}
</script>

<template>
  <div class="json-block" :style="blockStyle">
    <VueJsonPretty
      :data="data"
      :deep="deep"
      :show-double-quotes="false"
      :show-icon="true"
      :show-key-value-space="true"
      :show-length="true"
      :show-line="true"
      :show-line-number="false"
      :root-path="rootPath"
    />
  </div>
</template>
