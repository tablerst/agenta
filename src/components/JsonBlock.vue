<script setup lang="ts">
import { computed } from "vue";

import { prettyJson } from "../lib/format";

const props = defineProps<{
  value: unknown;
}>();

const text = computed(() => prettyJson(props.value));
</script>

<template>
  <pre class="json-block">{{ text }}</pre>
</template>
