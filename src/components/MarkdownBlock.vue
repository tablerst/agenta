<script setup lang="ts">
import DOMPurify from "dompurify";
import { marked } from "marked";
import { computed } from "vue";

const props = defineProps<{
  content: string | null | undefined;
}>();

const renderedContent = computed(() => {
  const source = props.content?.trim();

  if (!source) {
    return "";
  }

  const html = marked.parse(source, {
    async: false,
    breaks: true,
    gfm: true,
  });

  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
  });
});
</script>

<template>
  <div class="markdown-block" v-html="renderedContent" />
</template>
