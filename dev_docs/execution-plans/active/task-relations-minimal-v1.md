# Task 轻量关系层 V1

## 背景

当前 `Task` 只有自身生命周期状态，`blocked` 只是状态值，没有显式 blocker、父子拆分或依赖关系。基线文档将 `run graph / subagent graph / execution tree` 列为当前非目标，因此本阶段只补轻量任务关系，不把任务系统升级为通用执行图。

目标是让框架承担大部分关系维护工作：Agent 只需要知道任务可以有父子层级和 blocker，必要时提供少量 task id 或 relation id，不需要手工维护图结构、派生计数或状态回填。

## 方案

- 新增 `TaskRelation` 领域对象和 `task_relations` 表，支持 `parent_child` 与 `blocks` 两种关系。
- 关系方向固定：`parent_child` 中 source 是父任务、target 是子任务；`blocks` 中 source 是 blocker、target 是 blocked task。
- 关系只通过 `active -> resolved` 变更表达解除，不对外提供删除 API。
- 对外只暴露 task-centric helper：`task_create_child`、`task_attach_child`、`task_detach_child`、`task_add_blocker`、`task_resolve_blocker`。
- `task_get`、`task_list`、`task_context_get` 返回关系派生摘要，包含父任务、子任务数、open blockers、blocking 数和 `ready_to_start`。
- `task_context_digest` 纳入关系派生信息，降低 Agent 为理解任务状态而遍历关系图的需求。
- 关系变更追加 system activity；blocker 自动化只做最小安全回填：添加 blocker 时可置为 `blocked`，解除最后一个 blocker 时可恢复为 `ready`。

## 执行步骤

1. 数据模型与迁移：新增 relation 枚举、领域模型、SQLite migration、store CRUD 与关系查询。
2. Service 与同步：实现关系 helper、约束校验、摘要刷新、system activity、状态最小自动化，并把 `task_relation` 纳入 sync outbox/backfill/pull。
3. CLI/MCP/Desktop：新增对应命令和工具，Desktop 保留 action multiplexer 但详情读取收敛到 `get_context`。
4. 前端展示：扩展类型与 preview mock，在任务详情 overview 内展示父子和 blocker 关系，并提供绑定、解绑、添加 blocker、解除 blocker 操作。
5. 文档与验证：更新 quickstart、示例 policy、运行 Rust 与前端最低验证。

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 新增 `TaskRelation` 领域模型与 `task_relations` migration | 含 active 唯一关系与单父任务约束 |
| [x] | 实现本地 relation store 与任务派生统计 | 覆盖 parent、children、blocker、blocking 查询 |
| [x] | 实现 service helper 与最小自动化 | 包含 system activity、状态回填、摘要刷新 |
| [x] | 将 `task_relation` 纳入同步实体 | 覆盖 backfill、push/pull payload apply |
| [x] | 修正 `task_relation` 同步依赖顺序 | 明确 `project -> version -> task -> task_relation -> note -> attachment`，避免本地 FK apply 失败 |
| [x] | 扩展 MCP 工具面 | 保持显式工具名与单一意图 |
| [x] | 扩展 CLI 命令面 | 与 MCP helper 对齐 |
| [x] | 扩展 Desktop command 与前端 store | 详情读取使用单个 context payload |
| [x] | 补充任务详情关系展示与轻操作 | 不做 DAG 图视图 |
| [x] | 更新示例 policy 与 quickstart | 新 action 默认 auto |
| [x] | 完成 Rust 测试与前端构建验证 | 已通过 `cargo test --manifest-path src-tauri/Cargo.toml` 与 `npm run build` |

## 验收标准

- 创建或绑定父子任务后，`task_context_get` 能返回 parent/children，并且父子双方 digest 与活动记录更新。
- 添加 blocker 后，blocked task 在未关闭时进入 `blocked`，`open_blocker_count` 增加，`ready_to_start=false`。
- 解除最后一个 open blocker 后，blocked task 从 `blocked` 恢复为 `ready`。
- 尝试自环、重复 active relation、多个 active parent、parent cycle 时返回 conflict。
- Desktop 任务详情页能展示和操作轻量关系，且所有新增 UI 文案同时覆盖 `en` 与 `zh-CN`。
