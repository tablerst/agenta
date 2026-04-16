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

创建任务：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- `
  task create --project demo --title "Ship runtime console" --summary "Desktop-hosted MCP baseline"
```

CLI 默认输出 JSON。

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

- `project_create`：创建项目
- `project_get`：按 UUID 或 slug 读取项目
- `project_list`：列出项目
- `project_update`：更新项目
- `version_create`：为项目创建版本
- `version_get`：读取版本
- `version_list`：列出版本，可按项目过滤
- `version_update`：更新版本
- `task_create`：创建任务
- `task_create_child`：在父任务下创建子任务
- `task_get`：读取任务
- `task_list`：列出任务，可按项目、版本、状态过滤
- `task_update`：更新任务
- `task_attach_child`：把已有任务绑定为子任务
- `task_detach_child`：解除父子任务关系
- `task_add_blocker`：为任务添加 blocker
- `task_resolve_blocker`：解除任务 blocker
- `note_create`：为任务追加备注
- `note_list`：列出任务备注
- `attachment_create`：为任务添加附件
- `attachment_get`：读取附件
- `attachment_list`：列出任务附件
- `search_query`：搜索本地任务与任务活动

当前对外 contract 约束：

- 每个 Tool 对应单一意图，不再要求客户端传递 `arguments.action`
- `tools/list` 中会直接暴露字段说明、必填约束与可用枚举值
- `status` / `priority` / `kind` 等字段已直接进入 JSON Schema
- `*_get` / `*_list` / `search_query` 显式标记为只读

接入建议：

- 客户端应优先读取 `tools/list` 中的 `description`、`inputSchema`、`outputSchema`、`annotations`
- 不要假设仍存在旧的 `project` / `version` / `task` / `note` / `attachment` / `search` 多路复用工具
