# Agenta 依赖与打包策略

## 文档定位

本文档只定义当前阶段真正要引入的依赖和打包原则。

原则很简单：

- 先引入支撑当前阶段目标的依赖
- 不把远期依赖提前锁死进默认构建
- 不把“将来可能用到”写成“现在必须装上”

## 包管理器口径

- 前端默认包管理器：`bun`
- 兼容命令：保留 `npm`
- Rust 构建：`cargo`

保持 `bun` 的理由只有一个：当前 `src-tauri/tauri.conf.json` 已经以 `bun run dev` 和 `bun run build` 为默认命令。
如果未来要统一切到 `npm` 或 `pnpm`，必须在同一改动里同步更新 `tauri.conf.json`。

## 前端依赖策略

## 现在保留

- `vue`
- `@tauri-apps/api`
- `@tauri-apps/plugin-opener`

这些依赖已经足够支撑当前 scaffold 和第一轮重构。

## Phase 1 UI 开始时再加

- `vue-router`
- `tailwindcss`
- `@tailwindcss/postcss`
- `postcss`
- `lucide-vue-next`

原因：

- `vue-router` 只有在桌面 UI 不再是单页欢迎屏时才有必要
- Tailwind 只有在开始做真实布局和设计系统时才有价值
- `lucide-vue-next` 适合作为轻量图标集，但不需要早于真实界面落地

## 按需再加

- `pinia`
- `shadcn-vue`
- `@vueuse/core`

原则：

- `pinia` 只在出现跨页面、跨面板共享状态时引入
- `shadcn-vue` 只在 Tailwind 基础和组件命名约定已经稳定后引入
- `@vueuse/core` 只为真实重复模式买单，不为“可能有用”买单

## Rust 依赖策略

## Foundation 阶段必选

- `serde`
- `serde_json`
- `serde_yaml`
- `thiserror`
- `uuid`
- `time`
- `tracing`
- `tracing-subscriber`

这些依赖用于类型化配置、统一错误、标识、时间字段和日志。

## 数据层默认选择

当前默认建议是 `sqlx`，但只启用 SQLite 所需最小特性。

建议起步 feature：

```toml
sqlx = { version = "...", default-features = false, features = [
  "runtime-tokio",
  "sqlite",
  "migrate",
  "uuid",
  "time"
] }
```

当前阶段不默认启用：

- `postgres`
- `macros`

理由：

- 当前仓库没有 PostgreSQL 路线的即时需求
- `query!` 宏和离线校验流程会增加早期迁移复杂度
- 等 schema 稳定、查询模型稳定后，再决定是否引入 `macros` 和 `cargo sqlx prepare`

## CLI 阶段再加

- `tokio`
- `clap`

`tokio` 随数据库层进入，`clap` 随 CLI binary 进入。

## MCP 阶段再加

- `rmcp`
- `rmcp-actix-web`
- `actix-web`

MCP 不进入默认 Phase 1 依赖，是因为当前仓库还没有任何 MCP 入口代码。
等共享服务层稳定后再接入，返工最小。

## 默认不进主构建

- `sqlite-vec`
- `lancedb`
- `tauri-plugin-shell`

原因：

- 向量检索不是当前 MVP 默认能力
- sidecar 不是当前阶段刚需
- `tauri-plugin-shell` 一旦引入，就要同步处理 capability 和命令白名单

## 打包策略

## 默认桌面包

默认桌面包只包含：

- Tauri 主程序
- Vue 前端
- SQLite 主存储能力
- FTS5 检索

默认桌面包不包含：

- 外部 sidecar
- 向量数据库
- 额外 shell 执行权限

## 增强包触发条件

只有出现下面任一条件，才考虑增强包：

- 向量检索进入正式需求
- 桌面端必须管理独立后台进程
- 某项能力必须通过 sidecar 才能隔离生命周期

一旦进入增强包：

- 必须显式声明 `externalBin`
- 必须新增 `src-tauri/capabilities/*.json` 白名单
- 必须为 sidecar 参数做限制

## 当前建议结论

1. 继续使用 `bun` 作为前端默认包管理器。
2. 前端只提前锁定 `vue-router`、Tailwind 和 Lucide 图标，不把 `pinia`、`shadcn-vue` 强行写成 Phase 1 必装。
3. Rust 数据层默认选 `sqlx + sqlite`，但不提前打开 `postgres` 和 `macros`。
4. MCP、vector、sidecar 相关依赖一律按阶段引入，不进入默认主构建。
