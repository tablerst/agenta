# Agenta Desktop / CLI / MCP 快速开始

本文档反映当前正式基线：第二里程碑已完成，当前活跃工作流聚焦文档收口与 Desktop 宿主增强，Desktop 继续负责托管 MCP 生命周期与可视化日志。

## 1. 命名与入口

- 桌面产品名：`Agenta`
- 桌面二进制：`agenta-desktop`
- CLI 正式入口：`agenta`
- CLI 兼容别名：`agenta-cli`
- Standalone MCP：`agenta-mcp`

## 2. 配置

Agenta 采用 YAML-first 配置：

- 模板：`agenta.example.yaml`
- 本地覆盖：`agenta.local.yaml`

项目上下文目录配置如下：

```yaml
project_context:
  paths:
    - .dev_doc/.agenta
    - .agenta
    - dev_docs/.agenta
  manifest: project.yaml
```

`project.yaml` 是项目级提示文件，不是 Agenta 托管的长期记忆。Codex、Claude Code 等 Agent 应先读取仓库里的 `AGENTS.md`、`README.md`、架构说明、执行计划和本地 skill，再用 Agenta 恢复任务级台账。推荐最小内容：

```yaml
project: demo
instructions: README.md
memory_dir: memory
# entry_task_code: InitCtx-00 # optional task-lane recovery entry
```

MCP 配置面如下：

```yaml
mcp:
  bind: 127.0.0.1:8787
  path: /mcp
  autostart: false
  log:
    level: info
    # 未显式声明 destinations 时按宿主类型套默认值：
    # Desktop 托管 => [ui, file]
    # Standalone agenta-mcp => [stdout]
    file:
      path: ./local-data/logs/mcp.jsonl
    ui:
      buffer_lines: 1000
```

当未提供 `--config` 且当前目录不存在 `agenta.local.yaml` 时，数据库、附件和 MCP 文件日志默认落到系统应用数据目录。

`mcp.autostart` 的当前口径如下：

- `false`：Desktop 启动后保持 `stopped`，由 Runtime 页面显式启动
- `true`：Desktop 完成 setup 后自动拉起托管 MCP；若自动拉起失败，应用保持可用，Runtime 进入 `failed`

## 3. CLI 快速开始

查看帮助：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help
```

兼容别名仍可使用：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- --help
```

创建项目：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- `
  project create --slug demo --name "Demo Project"
```

初始化项目上下文目录：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- `
  context init --project demo
```

创建任务：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- `
  task create --project demo --title "Ship runtime console" --summary "Desktop-hosted MCP baseline"
```

恢复任务上下文：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- `
  task context --task <task-id> --notes-limit 5 --attachments-limit 3
```

CLI 默认输出 JSON。

更完整的 CLI 命令面、搜索回填、Chroma 前置条件和用户主动同步命令说明见 [CLI Reference](cli-reference.md)。

## 4. Desktop Runtime 控制台

桌面启动后，Runtime 页面现在是 MCP 控制台，默认行为如下：

- `mcp.autostart=false` 时，Desktop 启动后 MCP 默认为 `stopped`
- `mcp.autostart=true` 时，Desktop setup 完成后自动拉起托管 MCP
- 自动拉起失败不会终止 App，本轮仍通过现有日志与 `failed` 状态暴露错误
- 退出 App 时优雅停止
- 本阶段不包含 tray、后台常驻、关窗保活或 daemon 化

Runtime 控制台支持：

- 查询 MCP 五态状态机：`stopped / starting / running / stopping / failed`
- 本次启动覆盖：`bind`、`path`、`autostart`、`log level`、`destinations`、`file path`、`ui buffer`
- 显式保存为默认值，仅在存在 `loaded_config_path` 时可用
- 结构化日志快照与实时增量事件
- 失败后的就地恢复与打开日志目录

## 5. Standalone MCP

启动独立 MCP：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp
```

默认健康检查：

```text
GET http://127.0.0.1:8787/health
```

默认挂载点：

```text
http://127.0.0.1:8787/mcp
```

Standalone `agenta-mcp` 默认走 `stdout` 日志；若显式配置 `mcp.log.destinations`，则按配置覆盖。

## 6. MCP 工具面

当前 MCP 发布面已经切换为显式工具名，不再使用 `action + structured arguments` 多路复用模型。

命名约束：

- 所有工具名遵循最小兼容规则：`^[A-Za-z][A-Za-z0-9_]{0,63}$`
- 不使用点号 `.`、横杠 `-` 或空格

当前工具清单：

- `context_init`：初始化或更新项目上下文 manifest
- `project_create`：创建项目
- `project_get`：按 UUID 或 slug 读取项目
- `project_list`：列出项目
- `project_update`：更新项目
- `version_create`：为项目创建版本
- `version_get`：读取版本
- `version_list`：列出版本，可按项目过滤
- `version_update`：更新版本
- `task_create`：创建任务，支持 `task_code`、`task_kind`
- `task_create_child`：在父任务下创建子任务
- `task_get`：读取任务
- `task_list`：列出任务，支持 `project/version/status/kind/task_code_prefix/title_prefix/all_projects` 过滤，以及 `sort_by/sort_order` 排序；返回 `summary`
- `task_update`：更新任务，支持 `task_code`、`task_kind`
- `task_attach_child`：把已有任务绑定为子任务
- `task_detach_child`：解除父子任务关系
- `task_add_blocker`：为任务添加 blocker
- `task_resolve_blocker`：解除任务 blocker
- `note_create`：为任务追加备注，支持 `note_kind=scratch|finding|conclusion`
- `note_list`：列出任务备注
- `attachment_create`：为任务添加附件
- `attachment_get`：读取附件
- `attachment_list`：列出任务附件
- `search_query`：用结构化过滤 + 可选 query 搜索任务与任务活动；支持 `project/version/task_kind/task_code_prefix/title_prefix/all_projects`
- `search_evidence_get`：按 `evidence_chunk_id` 或 `evidence_attachment_id` 读取 `search_query` 返回命中的二跳证据正文

不属于 Agent 默认 MCP 工具面的能力：

- sync、release、runtime 运维操作保留在 Desktop 或用户主动 CLI 路径中，不进入默认 `tools/list`

当前对外 contract 约束：

- 每个 Tool 对应单一意图，不再要求客户端传递 `arguments.action`
- `tools/list` 中会直接暴露字段说明、必填约束与可用枚举值
- `status` / `priority` / `kind` 等字段已直接进入 JSON Schema
- `*_get` / `*_list` / `search_query` / `search_evidence_get` 显式标记为只读

任务恢复相关的推荐调用：

- 恢复某个版本下的编号任务组：`task_list(project=..., version=..., sort_by=task_code, sort_order=asc)`
- 直接按编号前缀拉一组任务：`search_query(project=..., version=..., task_code_prefix="InitCtx-")`
- 已知入口任务时先轻量读：`task_context_get(task=..., include_notes=false, include_attachments=false, recent_activity_limit=5)`
- 需要逐步展开时再加限额：`task_context_get(task=..., notes_limit=5, attachments_limit=3)`
- 搜索命中后需要完整证据时：`search_evidence_get(chunk_id=...)` 或 `search_evidence_get(attachment_id=...)`
- 只看上下文任务：`task_list(..., kind=context)`
- 判断沉淀状态时优先看：`task.task_context_digest`、`task.task_search_summary`、`task.latest_note_summary`、`task.knowledge_status`、`task_list.summary`

多项目环境下的默认行为：

- `task_list`、`search_query` 在未显式传 `project` 时，不会默认跨项目返回结果
- 如果当前项目上下文目录能解析出唯一项目，Agenta 会自动使用该项目范围
- 如果存在多个项目且无法唯一解析，调用会返回 `ambiguous_context`
- 只有显式传 `all_projects=true` 或 CLI `--all-projects` 时，才会跨项目查询

上下文初始化建议：

- Agent 或客户端先调用 `context_init`
- 如果项目上下文目录位置不固定，显式传 `context_dir` 或 `workspace_root`
- Desktop、CLI 和 MCP 都应复用这同一个动作，而不是各自手写目录规则
- 不要为了项目级长期上下文强制写入 `entry_task_code`；只有某个任务泳道确实需要稳定恢复入口时才写

接入建议：

- 客户端应优先读取 `tools/list` 中的 `description`、`inputSchema`、`outputSchema`、`annotations`
- 不要假设仍存在旧的 `project` / `version` / `task` / `note` / `attachment` / `search` 多路复用工具
- Agent 应先读取项目文件，再调用 Agenta 的 task ledger tools；Agenta 不负责维护项目全局记忆
