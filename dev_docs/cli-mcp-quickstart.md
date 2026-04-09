# Agenta CLI / MCP 快速开始

## 命名

- Desktop：`agenta-desktop`
- CLI 主命令：`agenta`
- CLI 兼容别名：`agenta-cli`
- Standalone MCP：`agenta-mcp`

## 配置

Agenta 按以下顺序加载配置：

1. `--config <path>`
2. `AGENTA_CONFIG`
3. 当前目录下的 `agenta.local.yaml`
4. 内建默认值

示例模板见 `agenta.example.yaml`。

当前 MCP 配置键：

- `mcp.bind`
- `mcp.path`
- `mcp.autostart`
- `mcp.log.level`
- `mcp.log.destinations`
- `mcp.log.file.path`
- `mcp.log.ui.buffer_lines`

## CLI

推荐命令：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help
```

兼容别名：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- --help
```

常见示例：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- `
  project create --slug demo --name "Demo Project"
```

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- `
  task create --project demo --title "Ship runtime console"
```

## Desktop Runtime MCP 控制台

启动桌面：

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-desktop
```

本阶段约束：

- MCP 默认 `stopped`
- 由 Runtime 页面显式启动
- 退出 App 时优雅停止
- 不包含 tray、后台常驻、daemon 化

## Standalone MCP

启动：

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

## MCP 工具面

当前 MCP 发布面使用显式工具名，不再采用 `action` 多路复用：

- `project_create` / `project_get` / `project_list` / `project_update`
- `version_create` / `version_get` / `version_list` / `version_update`
- `task_create` / `task_get` / `task_list` / `task_update`
- `note_create` / `note_list`
- `attachment_create` / `attachment_get` / `attachment_list`
- `search_query`

约束：

- 工具命名遵循 `^[A-Za-z][A-Za-z0-9_]{0,63}$`
- 不使用点号 `.`，以保证跨 provider 最小兼容性
- `tools/list` 会直接暴露字段说明、必填项与枚举值

已废弃的旧 MCP 工具名：

- `project`
- `version`
- `task`
- `note`
- `attachment`
- `search`
