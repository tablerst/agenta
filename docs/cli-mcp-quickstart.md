# Agenta Desktop / CLI / MCP 快速开始

本文档反映第二里程碑基线：Desktop 负责托管 MCP 生命周期与可视化日志，CLI 与 Standalone MCP 仍保持独立入口。

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

- Desktop 启动时 MCP 默认为 `stopped`
- 由 Runtime 页面显式启动
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

当前工具族保持不变：

- `project`
- `version`
- `task`
- `note`
- `attachment`
- `search`

每个工具继续采用 `action + structured arguments` 模型，并返回统一结构化结果。
