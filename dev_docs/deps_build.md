# Agenta Dependency and Build Strategy

## 1. 文档定位

这份文档只回答三个问题：

1. 现在应该依赖什么
2. 哪些依赖不能默认带上
3. 构建与打包应如何分阶段收敛

本文件不再沿用旧草稿中的拼接文本，当前版本是重写后的正式口径。

## 2. 依赖决策原则

### 2.1 先按模块分依赖，不按“项目一次装满”分依赖

不要把未来可能需要的依赖一次性全塞进当前工程。

正确做法：

- Core 只带领域和通用基础库
- Storage 才带 SQLx
- CLI 才带命令行库
- MCP 只带当前 transport 所需依赖
- Desktop 只带 Tauri 与桌面必需插件

### 2.2 默认构建只覆盖 MVP 主路径

默认构建必须只包含：

- SQLite
- FTS5
- CLI
- MCP `streamable_http`
- Desktop 基础壳

默认构建不应包含：

- PostgreSQL 驱动
- 向量后端
- 额外 sidecar

## 3. Rust 依赖口径

### 3.1 `agenta-core`

建议依赖：

- `serde`
- `serde_json`
- `thiserror`
- `uuid`
- `time`
- `tracing`

职责：

- 领域对象
- service trait 与用例
- 错误模型
- 策略模型

### 3.2 `agenta-storage-sqlite`

建议依赖：

- `tokio`
- `sqlx`
- `tracing`

SQLx 口径：

- 关闭默认 features
- 只启用 SQLite 主线路径所需特性
- 在确有需要时再增开 PostgreSQL

当前推荐启用的 SQLx feature 范围：

- `runtime-tokio`
- `sqlite`
- `macros`
- `migrate`
- `uuid`
- `time`

暂不默认启用：

- `postgres`
- 其他数据库驱动

### 3.3 `agenta-cli`

建议依赖：

- `clap`
- `serde_json`
- `tracing`
- `tracing-subscriber`

职责：

- 参数解析
- 输出格式控制
- 命令到 service 的映射

### 3.4 `agenta-mcp`

建议依赖：

- `rmcp`
- `serde`
- `serde_json`
- `tracing`
- `tracing-subscriber`

当前首发 MCP 即按 HTTP transport 落地，因此本阶段直接纳入：

- `rmcp-actix-web`
- `actix-web`

### 3.5 `src-tauri`

当前保留：

- `tauri`
- `tauri-build`
- `tauri-plugin-opener`

只有在确实需要 sidecar 或 shell 调用时，再增开对应插件与 capability。

## 4. 前端依赖口径

### 4.1 当前应尽快补上的基础依赖

建议正式补入：

- `vue-router`
- `pinia`

这两项直接对应真实应用最基础的信息架构与状态管理。

### 4.2 在 UI 开工时补入的依赖

建议在桌面 UI 真正开始开发时再补入：

- `tailwindcss`
- `@lucide/vue`

### 4.3 可选依赖

仅在真实页面和组件边界清晰后再评估：

- `shadcn-vue`
- `@vueuse/core`

这里不把 `shadcn-vue` 设为当前硬依赖，原因是：

- 现在还没有页面骨架
- 组件分发体系是 UI 阶段决策，不应反向驱动 Core 与 CLI

## 5. Feature 策略

建议尽早引入项目级 feature，而不是直接暴露底层库 feature。

推荐 feature 命名：

- `db-sqlite`
- `db-postgres`
- `mcp-stdio`
- `mcp-streamable-http`
- `search-fts`
- `search-vector-sqlite-vec`
- `search-vector-lancedb`
- `desktop-sidecar`

默认 feature 组合建议：

- `db-sqlite`
- `mcp-streamable-http`
- `search-fts`

非默认 feature：

- `db-postgres`
- `mcp-stdio`
- `search-vector-sqlite-vec`
- `search-vector-lancedb`
- `desktop-sidecar`

## 6. 构建策略

### 6.1 当前仓库阶段

当前仍使用：

- `bun run dev`
- `bun run build`
- `bun run tauri dev`
- `bun run tauri build`

这是因为仓库还没迁移到 Rust workspace，而且 `src-tauri/tauri.conf.json` 也已经绑定 Bun 作为默认构建入口。

### 6.2 迁移后的基础验证

在单 crate 过渡阶段，基础验证应至少包括：

- `bun run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`

迁移到 workspace 后，再升级为：

- `bun run build`
- `cargo check --workspace`
- `cargo test --workspace`

### 6.3 SQLx 开发链路

SQLx 主线建议同时定下：

- 使用迁移目录管理 schema
- 构建时通过 `migrate!()` 嵌入迁移
- 在 CI 中维护离线校验元数据

推荐补充命令：

- `cargo sqlx prepare --workspace`

## 7. 打包策略

### 7.1 默认发行物

默认发行物只包含：

- Desktop 基础壳
- Core
- SQLite
- FTS5
- CLI 主能力
- MCP `streamable_http` 主能力

### 7.2 增强发行物

只有在向量检索确实进入产品路线后，再考虑增强发行物：

- 可选 sidecar
- 可选向量数据库
- 单独 capability 白名单

### 7.3 当前禁止事项

当前不允许：

- 把 PostgreSQL 驱动塞进默认包
- 把 LanceDB 或其他重型后端塞进默认桌面包
- 因为未来 UI 想象提前堆满前端依赖

## 8. 本阶段的正式结论

当前正式依赖与构建口径如下：

- 数据访问主线统一为 SQLx
- 默认数据库只做 SQLite
- 默认 MCP 首发做 `streamable_http`
- `stdio` 作为后续补充 transport
- 前端先补 `vue-router` 与 `pinia`
- Tailwind、Lucide、shadcn-vue 延后到 UI 实施阶段
- 前端默认包管理继续使用 `bun`

如果后续要改动这些默认值，应先更新本文件，再调整工程依赖。
