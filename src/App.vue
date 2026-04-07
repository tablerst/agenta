<script setup lang="ts">
const milestones = [
  "共享 Rust 业务层已接管数据、配置与写入路径",
  "CLI 与 MCP 将作为首个可验收入口先行闭环",
  "桌面 UI 仅保留运行壳，等真实 contract 稳定后再接回",
];
</script>

<template>
  <main class="shell">
    <section class="hero">
      <p class="eyebrow">Agenta Desktop Shell</p>
      <h1>Core / CLI / MCP 正在落地</h1>
      <p class="lede">
        当前桌面端只保留最小运行壳。首个正式里程碑以 Rust Core、SQLite、
        Attachment、CLI 与 MCP `streamable_http` 闭环为准。
      </p>
    </section>

    <section class="panel">
      <h2>当前状态</h2>
      <ul>
        <li v-for="item in milestones" :key="item">{{ item }}</li>
      </ul>
    </section>
  </main>
</template>

<style scoped>
.shell {
  min-height: 100vh;
  padding: 56px 24px;
  display: grid;
  gap: 24px;
  background:
    radial-gradient(circle at top left, rgba(210, 232, 219, 0.9), transparent 42%),
    linear-gradient(180deg, #f4efe3 0%, #ebe3d2 100%);
}

.hero,
.panel {
  max-width: 760px;
  margin: 0 auto;
  width: 100%;
  padding: 28px;
  border: 1px solid rgba(74, 63, 44, 0.12);
  border-radius: 24px;
  background: rgba(255, 251, 244, 0.82);
  box-shadow: 0 20px 40px rgba(69, 53, 25, 0.08);
  backdrop-filter: blur(12px);
}

.eyebrow {
  margin: 0 0 12px;
  font-size: 0.85rem;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: #7d6843;
}

h1,
h2 {
  margin: 0;
  color: #2f2414;
}

h1 {
  font-size: clamp(2.2rem, 4vw, 4rem);
  line-height: 1;
}

.lede {
  margin: 16px 0 0;
  font-size: 1.05rem;
  line-height: 1.7;
  color: #4c3d28;
}

ul {
  margin: 18px 0 0;
  padding-left: 20px;
  color: #4c3d28;
  line-height: 1.7;
}

li + li {
  margin-top: 8px;
}
</style>

<style>
:root {
  font-family: "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
  color: #2f2414;
  background: #f4efe3;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

body {
  margin: 0;
}

* {
  box-sizing: border-box;
}
</style>
