# Agenta Product Baseline

## 1. 文档定位

这份文档是 Agenta 当前的唯一产品基线文档。

- `dev_docs/draft/` 下的内容只保留为历史草稿，不再作为实施依据
- 本文定义 MVP 范围、核心对象、能力边界和验收口径
- 技术实现细节以 [tech.md](/e:/JetBrains/RustRover/agenta/dev_docs/tech.md) 为准
- 从当前仓库迁移到目标架构的步骤以 [migration_plan.md](/e:/JetBrains/RustRover/agenta/dev_docs/migration_plan.md) 为准

## 2. 当前项目现实

当前仓库已完成首轮基础实现：

- 前端已替换为里程碑状态壳
- Rust 侧已具备共享业务层、CLI 与 MCP HTTP 入口
- 已落 SQLite、附件、检索与基础写策略
- 仍未拆 workspace，Desktop 也还不是首个业务化交付面

因此本基线同时面向两件事：

- 约束未来要做成什么
- 约束当前实现继续沿同一主线演进，而不是重新分叉

## 3. 产品定位

Agenta 是一个本地优先、单用户、面向 Agent 宿主的任务与上下文管理服务。

它的目标不是替代宿主，也不是接管整个执行系统。它负责提供：

- 项目与版本视图
- 任务与活动时间线
- 附件与图片上下文
- 可消费的检索结果
- 受控写入策略

Agenta 不负责：

- 宿主级沙箱
- 命令审批系统本体
- 多 Agent 执行图编排
- 云端协作与同步

## 4. Canonical 能力面

Agenta 的 canonical 能力面只有两条：

- CLI
- MCP Tools

桌面端是观察与操作壳，不是系统中心。

这意味着：

- 任何核心业务动作都必须先能通过 CLI 或 MCP 完成
- Desktop 不能承载独占逻辑
- UI 可以后补，但 Core、CLI、MCP 不能被 UI 绑架

## 5. MVP 范围

当前正式 MVP 包含以下能力。

### 5.1 领域对象

必须落地这五个核心对象：

- `Project`
- `Version`
- `Task`
- `TaskActivity`
- `Attachment`

### 5.2 数据与存储

必须落地：

- SQLite 作为唯一权威元数据源
- 本地文件系统作为附件实体存储
- 结构化 schema 与迁移机制

### 5.3 接口能力

必须落地：

- CLI 命令族
- MCP 工具族
- 统一 JSON 输出骨架
- 统一错误模型

### 5.4 检索能力

必须落地：

- FTS5 全文检索
- `task_search_summary`
- `task_context_digest`
- `activity_search_summary`
- 统一 `search` service

### 5.5 写入控制

必须落地：

- 动作级写策略
- `auto | require_human | deny` 三档策略
- 策略命中后的结构化返回

## 6. 明确非目标

以下内容不进入当前 MVP：

- 多用户协作
- 云端同步
- 复杂 RBAC
- Proposal / Approval 工作流引擎
- run graph / subagent graph / execution tree 建模
- 向量检索默认启用
- PostgreSQL 作为首发运行时
- Tauri UI 作为首个生产里程碑的主交付

## 7. 核心对象模型

### 7.1 Project

Project 是顶层业务隔离边界。

建议字段：

- `project_id`
- `slug`
- `name`
- `description`
- `status`: `active | archived`
- `default_version_id`
- `created_at`
- `updated_at`

约束：

- `project_id` 是内部稳定主键
- `slug` 是 CLI 与 MCP 的人类可读引用

### 7.2 Version

Version 是项目下的轻量任务归属桶，不是完整 release object。

建议字段：

- `version_id`
- `project_id`
- `name`
- `description`
- `status`: `planning | active | closed | archived`
- `created_at`
- `updated_at`

约束：

- MVP 只要求它承担任务归属与任务过滤
- 不扩展成发布编排或变更审批中心

### 7.3 Task

Task 是系统的核心工作对象。

建议字段：

- `task_id`
- `project_id`
- `version_id`
- `title`
- `summary`
- `description`
- `task_search_summary`
- `task_context_digest`
- `status`
- `priority`
- `created_by`
- `updated_by`
- `created_at`
- `updated_at`
- `closed_at`

建议状态：

- `draft`
- `ready`
- `in_progress`
- `blocked`
- `done`
- `cancelled`

建议优先级：

- `low`
- `normal`
- `high`
- `critical`

### 7.4 TaskActivity

TaskActivity 是任务的轻量活动时间线。

建议字段：

- `activity_id`
- `task_id`
- `kind`
- `content`
- `activity_search_summary`
- `created_by`
- `created_at`
- `metadata_json`

建议类型：

- `note`
- `status_change`
- `system`
- `attachment_ref`

边界：

- 它是时间线，不是执行图
- 它记录有价值的业务上下文，不承担宿主级 trace 语义

### 7.5 Attachment

Attachment 是 MVP 正式核心对象，不是以后再补的增强项。

建议字段：

- `attachment_id`
- `task_id`
- `kind`
- `mime`
- `original_filename`
- `original_path`
- `storage_path`
- `sha256`
- `size_bytes`
- `summary`
- `created_by`
- `created_at`

建议类型：

- `screenshot`
- `image`
- `log`
- `report`
- `patch`
- `artifact`
- `other`

## 8. CLI 与 MCP 口径

CLI 与 MCP 必须共用相同的业务语义。

### 8.1 CLI 命令族

MVP 保持以下命令族：

- `project`
- `version`
- `task`
- `note`
- `attachment`
- `search`

### 8.2 MCP 工具族

MVP 保持以下工具族：

- `project`
- `version`
- `task`
- `note`
- `attachment`
- `search`

每个工具均采用：

- `action`
- 结构化参数
- 统一输出骨架

### 8.3 统一输出骨架

成功返回至少包含：

- `ok`
- `action`
- 主体对象字段或结果列表
- `summary`
- `warnings`

错误返回至少包含：

- `ok: false`
- `error.code`
- `error.message`
- `error.details`

建议错误码：

- `not_found`
- `ambiguous_context`
- `invalid_action`
- `invalid_arguments`
- `policy_blocked`
- `requires_human_review`
- `conflict`
- `internal_error`

## 9. 写策略模型

Agenta 采用动作级写策略，不引入重型审批流。

策略级别只有三档：

- `auto`
- `require_human`
- `deny`

示例动作键：

- `project.create`
- `project.update`
- `version.create`
- `task.create`
- `task.update`
- `task.set_status.done`
- `task.move_to_version`
- `note.add`
- `attachment.add`
- `attachment.materialize`

规则：

- 宿主负责系统级权限与审批
- Agenta 只负责业务动作是否允许落账
- 策略命中结果必须进入结构化输出

## 10. MVP 验收口径

以下条件同时成立，才算达到当前 MVP：

1. 可以通过 CLI 和 MCP 对 `Project / Version / Task / TaskActivity / Attachment` 做最小闭环操作。
2. SQLite schema、迁移、基础索引、附件文件布局全部可用。
3. 可以对 Task 与 TaskActivity 做 FTS5 检索，并返回结构化结果。
4. 写策略可配置、可命中、可返回明确阻断原因。
5. 所有主路径都有统一 JSON 输出与统一错误模型。
6. Desktop 即使尚未完成，也不能阻塞 CLI 与 MCP 主路径交付。

## 11. 当前默认决策

这份基线先按以下默认值收敛：

- 首发 MCP 以 `streamable_http` 为主
- `stdio` 作为后续补充 transport，不阻塞 MVP
- Desktop 不进入首个生产里程碑的必交项
- 向量检索保留接口，不进入默认发行物
- Phase 1 直接锁定 `SQLx + SQLite`
- 前端包管理默认保持 `bun`
- 数据库与附件默认落在系统应用数据目录，便携模式后续再补

若这些默认值变化，应先更新本文，再改技术实现。
