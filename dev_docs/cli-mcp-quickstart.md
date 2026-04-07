# Agenta CLI / MCP Quickstart

本文档只描述当前已经实现并通过验证的对外使用面。

## 1. 配置

Agenta 使用 YAML-first 配置：

- 模板：`agenta.example.yaml`
- 本机覆盖：`agenta.local.yaml`

如果不传 `--config`，且当前目录不存在 `agenta.local.yaml`，则数据库与附件默认落在系统应用数据目录。

最小配置示例：

```yaml
paths:
  data_dir: ./local-data
  database_path: ./local-data/agenta.sqlite3
  attachments_dir: ./local-data/attachments

mcp:
  bind: 127.0.0.1:8787
  path: /mcp

policy:
  default: auto
```

## 2. CLI 快速开始

查看帮助：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- --help
```

创建项目：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  project create --slug demo --name "Demo Project"
```

创建版本：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  version create --project demo --name v1
```

创建任务：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  task create --project demo --title "Ship first contract" --summary "CLI + MCP"
```

创建备注：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  note create --task <task_id> --content "First note"
```

上传附件：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  attachment create --task <task_id> --path .\sample.log --summary "build log"
```

按附件 ID 直取附件：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  attachment get --attachment <attachment_id>
```

搜索：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  search query --text dashboard
```

说明：

- CLI 默认输出 JSON。
- 传 `--human` 时会输出摘要加结果体。
- `note list` 现在只返回 `note` 类型活动，不再混入 `attachment_ref` 等其他 activity。

## 3. CLI 命令面

当前命令族：

- `project create|get|list|update`
- `version create|get|list|update`
- `task create|get|list|update`
- `note create|list`
- `attachment create|get|list`
- `search query`

当前主要引用字段：

- 项目：`--project <project_id_or_slug>`
- 版本：`--version <version_id>`
- 任务：`--task <task_id>`
- 附件：`--attachment <attachment_id>`

## 4. MCP 快速开始

启动 MCP 服务：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp
```

默认健康检查：

```text
GET http://127.0.0.1:8787/health
```

默认 MCP 挂载点：

```text
http://127.0.0.1:8787/mcp
```

当前 transport 为 `streamable_http`。

## 5. MCP 工具面

当前工具：

- `project`
- `version`
- `task`
- `note`
- `attachment`
- `search`

每个工具都使用 `action + structured arguments` 形式。

### `project`

支持：

- `action=create`
- `action=get`
- `action=list`
- `action=update`

创建项目示例：

```json
{
  "name": "project",
  "arguments": {
    "action": "create",
    "slug": "demo",
    "name": "Demo Project"
  }
}
```

### `task`

支持：

- `action=create`
- `action=get`
- `action=list`
- `action=update`

### `note`

支持：

- `action=create`
- `action=list`

说明：

- `note.list` 只返回 note 项。

### `attachment`

支持：

- `action=create`
- `action=get`
- `action=list`

按附件 ID 直取示例：

```json
{
  "name": "attachment",
  "arguments": {
    "action": "get",
    "attachment": "<attachment_id>"
  }
}
```

### `search`

支持：

- `action=query`

示例：

```json
{
  "name": "search",
  "arguments": {
    "action": "query",
    "text": "dashboard",
    "limit": 10
  }
}
```

## 6. 返回骨架

成功响应：

```json
{
  "ok": true,
  "action": "task.create",
  "result": {},
  "summary": "Created task",
  "warnings": []
}
```

失败响应：

```json
{
  "ok": false,
  "error": {
    "code": "not_found",
    "message": "resource not found: task ...",
    "details": {}
  }
}
```

当前常见错误码：

- `invalid_arguments`
- `invalid_action`
- `not_found`
- `conflict`
- `policy_blocked`
- `requires_human_review`
- `internal_error`
