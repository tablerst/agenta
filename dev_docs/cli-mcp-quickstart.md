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
