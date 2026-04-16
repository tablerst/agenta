# Agenta Migration Plan

## 1. 文档目标

这份文档描述如何把当前仓库从默认 Tauri scaffold 迁移到可交付的 Agenta 架构。

它不讨论远期愿景，只讨论当前仓库下一步该怎么做。

## 2. 当前起点

仓库当前状态：

- 一个已落地全局壳层、项目工作区、审批中心、全局搜索与 Runtime 控制台的 Vue/Tauri 桌面入口
- 一个单 `src-tauri` Rust package
- 已落数据库、CLI、MCP、正式领域模型与 Desktop 命令桥
- 已有 migration、附件落盘、FTS、审批回放与手动远程副本同步能力

这意味着迁移主线已经走过“从 0 到 1”的阶段，下一步要避免在 Desktop 或新增能力上重新引入分叉。

当前仍需遵循两个原则：

- 先形成业务主线，再回头完善桌面壳
- 每一阶段都能独立验证，不允许大爆炸式重构

## 3. 目标终点

迁移完成后的最小可交付状态：

- 有明确的领域模型与 SQLite schema
- 有 CLI 命令族
- 有 MCP 工具族
- 有附件与检索能力
- 有动作级写策略
- Desktop 能消费真实数据，而不是示例页面

## 4. 阶段计划

### Phase 0: 文档与决策冻结

目标：

- 冻结产品基线
- 冻结技术主线
- 冻结依赖与构建口径

交付物：

- [baseline.md](baseline.md)
- [tech.md](tech.md)
- [deps_build.md](deps_build.md)
- 当前文档

退出条件：

- 团队不再以 `dev_docs/draft/` 作为实施依据

### Phase 1: 建立 Rust 主线

目标：

- 不破坏当前 scaffold 的前提下建立共享业务核心
- 锁定 `SQLx + SQLite`
- 建立系统应用数据目录与 YAML 配置主线

建议动作：

1. 继续保留 `src-tauri` 作为 Rust 主入口
2. 在 `src-tauri` 内建立领域、存储、service、config 模块
3. 确定数据库、附件与配置的默认落盘目录
4. 引入 SQLx migration 与最小测试基线

退出条件：

- 核心模块不再与示例 `greet` 逻辑耦合
- 可以在单 crate 内独立验证核心业务层

### Phase 2: 落 SQLite schema 与 service

目标：

- 落地五个核心对象
- 建立迁移与 repository
- 打通附件元数据与文件布局

建议动作：

1. 定义 schema 与索引
2. 建立 SQLx migration
3. 实现 project/version/task/activity/attachment repository
4. 实现统一错误模型
5. 实现 service contract

退出条件：

- 可以通过 Rust 集成测试验证核心对象的增删改查
- 附件元数据与实体存储形成闭环

### Phase 3: 补 CLI

目标：

- 提供最稳定的自动化与调试入口

建议动作：

1. 先以 `src-tauri/src/bin/agenta-cli.rs` 或等价方式补 CLI 入口
2. 落命令族 `project/version/task/note/attachment/search`
3. 默认输出 JSON
4. 补最小文本输出模式

退出条件：

- 可以不用 Desktop 直接管理数据
- 可以通过 CLI 完成主要业务路径调试

### Phase 4: 补 MCP

目标：

- 让 Agent 宿主通过稳定工具面接入 Agenta

建议动作：

1. 在单 crate 或共享 core 上补 MCP 入口
2. 首先实现 `streamable_http` transport
3. 落显式工具名 schema
4. 建立统一返回骨架

退出条件：

- MCP 工具可以覆盖与 CLI 对齐的核心动作
- MCP 不再依赖 Desktop 存在

### Phase 5: 补检索与写策略

目标：

- 让系统从“能记账”变成“能消费上下文”

建议动作：

1. 落地 `task_search_summary`
2. 落地 `task_context_digest`
3. 落地 `activity_search_summary`
4. 建立 FTS5 表与 search service
5. 建立动作级写策略存储与返回

退出条件：

- 搜索结果能直接为 Agent 提供可消费摘要
- 关键写动作可以被策略阻断或要求人工确认

### Phase 6: 评估 workspace 拆分

目标：

- 在核心 contract 已验证后，再判断是否值得拆 workspace

建议动作：

1. 评估 `src-tauri` 单 crate 是否已成为明显协作瓶颈
2. 若需要，再拆 `agenta-core` 与 `agenta-storage-sqlite`
3. 将 CLI 与 MCP 提升为独立 app crate

退出条件：

- 拆分带来清晰收益，而不是只带来目录变化

### Phase 7: 继续扩展 Desktop

目标：

- 在已落地的工作区与 Runtime 页面基础上补强真实观察与操作界面

建议动作：

1. 在既有 `vue-router` 与 `pinia` 基础上继续补强任务详情、附件与检索深链
2. 保持“Desktop 只消费共享 contract”的薄客户端原则
3. 为审批、Runtime 与同步链路补更多可观察性与错误反馈
4. 继续收敛页面间的导航、滚动和上下文切换语义

退出条件：

- Desktop 消费真实 contract
- UI 不包含独占业务逻辑

### Phase 8: 评估增强项

仅在主线稳定后再进入：

- `streamable_http`
- sidecar
- 向量检索
- PostgreSQL
- 更复杂的桌面分发

## 5. 每阶段最低验证

### Rust

- `cargo check --workspace`
- `cargo test --workspace`

在 workspace 未建立前：

- `cargo check --manifest-path src-tauri/Cargo.toml`

### Frontend

- `bun run build`

### Desktop

- `bun run tauri dev`

## 6. 主要风险

### 6.1 过早做 UI

如果先做页面，很容易把业务逻辑重新写回 `src-tauri` 和前端状态里。

### 6.2 过早做 workspace、HTTP 或 sidecar

如果在没有真实使用压力前就引入 workspace、HTTP transport、actix 或 sidecar，会显著放大重构、进程治理、权限和打包复杂度。

### 6.3 过早做向量检索

如果 FTS5、摘要、对象模型都还没稳定，就引入向量后端，工程复杂度会远超收益。

## 7. 当前开放问题

以下问题暂按默认值处理，但仍值得尽快确认：

1. 远程副本同步是否继续保持手动 `backfill / push / pull`，还是进入后台自动同步阶段。
2. MCP 首发 `streamable_http` 的宿主约束已成立，后续是否补 `stdio` 仍需评估。
3. Desktop 下一轮优先补任务详情/附件面板/搜索深链，还是优先补更多 Runtime 与 sync 可观察性。
4. 是否需要支持仓库内便携模式，而不是只使用系统应用数据目录。

## 8. 推荐的立即执行项

如果下一步直接开工，建议按这个顺序：

1. 先把文档口径与当前实现重新对齐，消除“状态壳 / sync 只读诊断”等过期表述。
2. 继续补 Desktop 的任务详情、附件、检索深链与审批交互，保持薄客户端原则。
3. 在真实使用压力下补更细的错误码、筛选能力和人类友好输出。
4. 评估是否需要补 `stdio` 作为 MCP 次级 transport，以及是否进入后台自动同步阶段。
5. Desktop 与 sync 主线稳定后再评估 workspace 拆分，而不是提前拆目录。
