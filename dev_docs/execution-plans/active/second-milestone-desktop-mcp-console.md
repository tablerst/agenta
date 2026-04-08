# 第二里程碑：Desktop 承载 MCP 与 Runtime 控制台基线

## 背景

当前仓库已经具备以下前提：

- 共享 `AppRuntime`
- 独立 `agenta-cli` / `agenta-mcp` 入口
- YAML-first 配置
- Tauri Desktop command bridge
- Runtime 页面已存在但仍偏静态状态页

第一里程碑已经证明 `core + app + cli + mcp` 主线可行，因此第二里程碑不回写首里程碑续篇，而是正式收口为新的 active 基线。

## 方案

### 命名与分发

- 桌面产品显示名固定为 `Agenta`
- 桌面二进制固定为 `agenta-desktop`
- CLI 正式入口固定为 `agenta`
- CLI 兼容别名保留为 `agenta-cli`
- Standalone MCP 保留为 `agenta-mcp`

### 生命周期与宿主

- Desktop 默认承载 MCP 生命周期与可视化日志
- MCP 默认不自动启动，由 Runtime 页面显式启动
- App 退出时优雅停止托管 MCP
- 本阶段不实现 tray、后台常驻、关窗保活、daemon 化

### 配置与日志

- YAML 保存默认值，Runtime 表单提供本次启动覆盖
- 仅在存在 `loaded_config_path` 时允许显式写回默认值
- MCP 状态机固定为 `stopped / starting / running / stopping / failed`
- MCP 日志与 app shell tracing 分离
- 日志 destinations 支持 `ui / stdout / file`
- Desktop 托管默认 `ui + file`
- Standalone `agenta-mcp` 默认 `stdout`

### Runtime 控制台

- Runtime 页面角色固定为“MCP 控制台”
- 包含状态卡、启动配置区、日志区、错误恢复区
- 日志使用结构化列表，不做伪终端样式

## 执行步骤

### Phase 1：文档与命名收口

- 新建第二里程碑 active 计划
- 同步 README、Quickstart、文档索引与正式口径
- 调整 Cargo/Tauri 命名，避免 Desktop 与 CLI 冲突

### Phase 2：共享 MCP Host 与 Supervisor

- 抽出共享 MCP host 启动逻辑
- Desktop 引入 `McpSupervisor`
- 暴露状态、启停、日志快照与事件接口

### Phase 3：配置与多路日志

- 扩展 YAML 配置到 `mcp.autostart` 与 `mcp.log.*`
- 支持宿主默认 destinations
- 支持 UI ring buffer、文件 JSONL、stdout

### Phase 4：Runtime 控制台 UI

- 将 Runtime 页面升级为 MCP 控制台
- 接入启动覆盖、状态刷新、日志快照、失败恢复
- 支持打开日志目录

### Phase 5：验证与收尾

- 保持 `cargo check --manifest-path src-tauri/Cargo.toml`
- 保持 `cargo test --manifest-path src-tauri/Cargo.toml`
- 保持 `bun run build`
- 截图与更细的手动验收留待后续补齐

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 新建 active 计划文件并归档为第二阶段基线 | 本文件 |
| [x] | 同步正式文档中的命名与分发口径 | README、`dev_docs/README.md`、`docs/cli-mcp-quickstart.md` 已更新 |
| [x] | 调整 Cargo/Tauri 构建与可执行命名 | `agenta-desktop` / `agenta` / `agenta-cli` / `agenta-mcp` |
| [x] | 抽取共享 MCP host 启动逻辑 | `build_mcp_router`、`start_mcp_host` |
| [x] | 实现 `McpSupervisor` 与 5 态状态机 | 含 session、错误、优雅关闭 |
| [x] | 扩展 Tauri 命令与事件接口 | `desktop_mcp_status`、`desktop_mcp_start`、`desktop_mcp_stop`、`desktop_mcp_logs_snapshot` |
| [x] | 扩展 YAML 配置模型 | 已加入 `autostart` 与 `mcp.log.*` |
| [x] | 实现 MCP 多路日志 | `ui / stdout / file` 已接通 |
| [x] | 暴露 `loaded_config_path` 到前端 | `desktop_status` 与 `RuntimeStatus` 已同步 |
| [x] | 将 Runtime 页面升级为 MCP 控制台 | 状态卡、配置区、日志区、错误恢复区已落地 |
| [x] | 补齐基础 Rust 验证 | `cargo test` 通过，CLI 新旧入口兼容保留 |
| [ ] | 更新截图与更细的手动验收记录 | 本轮未补截图 |

## 当前验收结论

- Desktop 启动后 MCP 默认为 `stopped`
- Runtime 页面可显式启动 / 停止托管 MCP
- 前端通过状态命令、日志快照与 Tauri 事件消费 MCP 生命周期
- CLI 正式入口切换到 `agenta`，兼容别名 `agenta-cli` 仍可用
- Standalone `agenta-mcp` 仍能独立启动
