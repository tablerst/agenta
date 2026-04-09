# Agenta Technical Architecture

## 1. 文档定位

这份文档定义当前仓库的目标技术架构，并明确哪些决策现在就要执行，哪些只做预留。

- 产品边界以 [baseline.md](/e:/JetBrains/RustRover/agenta/dev_docs/baseline.md) 为准
- 依赖与构建口径以 [deps_build.md](/e:/JetBrains/RustRover/agenta/dev_docs/deps_build.md) 为准
- 迁移顺序以 [migration_plan.md](/e:/JetBrains/RustRover/agenta/dev_docs/migration_plan.md) 为准

## 2. 当前状态与目标状态

### 2.1 当前状态

仓库当前已经具备：

- Vue 3 桌面状态壳
- 单 `src-tauri` Rust crate
- 共享业务层、SQLite migration、附件落盘
- CLI 与 MCP `streamable_http` 入口
- Bun 驱动的前端开发与构建配置

仓库当前仍然没有：

- Rust workspace
- 独立 app crate / core crate 拆分
- 真实 Desktop 业务页面
- 默认启用的向量后端或 sidecar

### 2.2 目标状态

目标不是“一次性重写成大系统”，而是收敛成一个 Rust Core + 多入口适配层的本地应用。

目标能力分层：

- Core：领域模型、服务、策略、错误模型
- Storage：SQLite 与文件存储实现
- CLI：本地命令入口
- MCP：Agent 宿主入口
- Desktop：桌面观察与操作壳

## 3. 架构原则

### 3.1 Core 与 Adapter 分离

核心业务不感知入口协议。

这意味着：

- `TaskService` 不知道自己被 CLI 还是 MCP 调用
- 检索与附件逻辑不能散落到 UI 中
- CLI、MCP、Desktop 不能各自维护一套业务规则

### 3.2 Desktop 不是系统中心

Desktop 必须是壳，不是业务唯一宿主。

执行含义：

- MVP 先保证 CLI 与 MCP 可用
- Desktop 只消费 core 能力
- 不允许把关键业务逻辑只写在 Tauri command 里

### 3.3 先做稳定主线，再做增强能力

当前优先级必须是：

1. SQLite + 文件系统
2. CLI + MCP
3. FTS5 + 摘要
4. Policy
5. Desktop
6. 向量检索与 sidecar

## 4. 推荐代码组织

### 4.1 短期实施形态

在当前仓库阶段，不建议一开始就为“理想结构”做大搬迁。

短期做法：

- 继续保留单仓库
- 继续保留 `src-tauri` 作为 Rust 主入口
- 先在 `src-tauri` 内建立共享领域、存储、service 与配置模块
- 等 CLI、MCP、检索主线跑通后，再决定是否拆成 workspace

这一步的核心目标不是目录好看，而是先把业务主线做成。

### 4.2 长期目标目录形态

建议逐步迁移为如下结构：

```text
/
  Cargo.toml
  crates/
    agenta-core/
    agenta-storage-sqlite/
  apps/
    agenta-cli/
    agenta-mcp/
  src/
  src-tauri/
  dev_docs/
```

### 4.3 模块职责

`crates/agenta-core`

- 领域对象
- 服务接口
- 写策略
- 搜索抽象
- 错误模型
- 上下文解析

`crates/agenta-storage-sqlite`

- SQLx 连接管理
- schema migration
- SQLite repository 实现
- FTS5 索引维护
- 附件元数据落库

`apps/agenta-cli`

- 参数解析
- 命令分发
- JSON 与文本输出

`apps/agenta-mcp`

- MCP server
- tool schema
- action dispatch
- MCP 请求上下文到 core context 的映射

`src-tauri`

- 桌面壳
- 本地进程管理
- 配置与窗口状态
- 对 core/CLI/MCP 的调用适配

### 4.4 过渡原则

迁移不是一步到位。过渡期间允许：

- 先在 `src-tauri` 内抽出领域模块
- 先在单 crate 内增加 CLI 与 MCP 入口
- Desktop 暂时直接调用 core，而不是立刻引入 sidecar
- 等核心 contract 稳定后再上移到 workspace crate

但不允许：

- 新业务继续深埋在临时入口或桌面专属代码里
- 为了赶 UI 把 CLI 与 MCP 继续延后

## 5. 存储设计

### 5.1 主存储

当前主存储正式定为：

- 元数据：SQLite
- 附件实体：本地文件系统

原因：

- 匹配本地单用户产品形态
- 便于与 CLI、MCP、Desktop 共用同一状态
- 部署和调试成本低

### 5.2 数据访问层

数据库访问统一走 SQLx。

原则：

- Core 不直接依赖 SQLx
- SQLite 实现在 storage crate 中
- PostgreSQL 只作为未来接口预留，不进入 MVP 默认运行时

### 5.3 搜索与摘要

MVP 必须落地：

- FTS5
- `task_search_summary`
- `task_context_digest`
- `activity_search_summary`

预留但不默认启用：

- 向量召回
- RRF 融合
- rerank

## 6. 接口设计

### 6.1 CLI

CLI 是最稳定的调试与自动化入口。

要求：

- 命令族与业务对象一一对应
- 默认支持 JSON 输出
- 文本人类输出只是附加层

### 6.2 MCP

MCP 层保持“显式工具名 + 单一意图”模型，不再使用 `action` 多路复用。

示例：

- `project_create` / `project_get` / `project_list` / `project_update`
- `version_create` / `version_get` / `version_list` / `version_update`
- `task_create` / `task_get` / `task_list` / `task_update`
- `note_create` / `note_list`
- `attachment_create` / `attachment_get` / `attachment_list`
- `search_query`

### 6.3 MCP transport 策略

当前默认口径：

- MVP 主 transport：`streamable_http`
- 后续补充 transport：`stdio`

原因：

- 首个里程碑要求 CLI 与 MCP 同步形成真实业务闭环
- 需要优先验证独立 MCP 服务入口，而不是把 MCP 能力耦合进 Desktop
- 保留 `stdio` 扩展点，但当前默认实现围绕 HTTP transport 收敛

如果未来确认本地宿主对 `stdio` 有更强适配需求，再补独立 `stdio` transport。

## 7. Desktop 设计

### 7.1 当前定位

Tauri 负责：

- 窗口与前端承载
- 用户观察与操作
- 本地配置展示
- 后续 sidecar 管理

Tauri 不负责：

- 独占业务逻辑
- 唯一数据写入路径
- 独占的检索与策略实现

### 7.2 实施原则

Desktop 的合理落地顺序：

1. 先保留现有 scaffold，可运行即可
2. Core 稳定后接入真实数据
3. UI 只消费已稳定的 CLI/core contract
4. Sidecar 只在确有必要时引入

## 8. 前端技术策略

当前前端仍基于 Vue 3。

正式建议：

- 路由：`vue-router`
- 状态：`pinia`
- 样式体系：`tailwindcss`
- 图标：`@lucide/vue`

可选但不设为 MVP 硬前提：

- `shadcn-vue`
- `@vueuse/core`

这样做的原因很直接：

- 当前仓库还没有任何真实信息架构
- 先把路由、状态和基础样式定下来即可
- 组件分发体系要在真实页面边界出现后再收敛

## 9. 配置原则

新增配置面统一遵循 YAML-first：

- 可提交模板使用 `*.example.yaml`
- 机器本地覆盖使用 `*.local.yaml`
- 支持环境变量注入
- 明确 schema 与默认值

不建议一开始就散落读取大量环境变量。

数据根目录默认策略：

- 生产模式使用系统应用数据目录
- 开发模式允许显式覆盖
- 便携模式后续单独设计，不在当前默认路径中

## 10. 不做的技术决策

以下内容当前不正式拍板：

- 默认集成 PostgreSQL
- 默认启用向量后端
- 默认引入 sidecar 常驻进程
- 为 UI 提前绑定大型组件库

这些都应等主线跑通后，再以单独文档或 ADR 追加。
