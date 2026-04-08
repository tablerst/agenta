# Agenta 架构说明

## 当前定位

Agenta 当前继续保持单一 `src-tauri` package，而不是提前拆成 workspace。第二里程碑的正式架构口径固定为：

- 代码层：`core + app + cli + mcp`
- 分发层：`Desktop 主体 / CLI 主命令 / Standalone MCP 可选`
- Desktop 负责 MCP 生命周期托管与 Runtime 控制台
- CLI 与 MCP 继续复用同一份共享服务层与数据模型

## 入口关系

当前二进制关系如下：

- `agenta-desktop`
  - Tauri 桌面主程序
  - 持有共享 `AppRuntime`
  - 持有 `McpSupervisor`
  - 通过 Tauri command 和 event 暴露 Runtime 控制台能力
- `agenta`
  - CLI 正式入口
  - 与 `agenta-cli` 指向同一套命令实现
- `agenta-cli`
  - CLI 兼容别名
- `agenta-mcp`
  - Standalone MCP 入口
  - 复用共享 MCP host 启动逻辑

## Desktop 托管 MCP

第二里程碑内，Desktop 与 MCP 的关系固定如下：

- MCP 默认不自动启动，Runtime 页面显式启动
- App 退出时优雅停止托管 MCP
- 不做 tray、关窗保活、后台常驻或 daemon 化
- Runtime 页面通过状态查询、日志快照和 Tauri 增量事件消费 MCP 生命周期

## MCP Host 结构

共享 MCP host 由 app 层统一提供：

- `build_mcp_router`
  - 生成 `/health` 与挂载点 router
- `start_mcp_host`
  - 绑定地址
  - 启动 streamable HTTP server
  - 提供可优雅停止的运行句柄
- `McpSessionLogger`
  - 生成结构化 MCP 日志事件
  - 按宿主类型写入 `ui / stdout / file`
- `McpSupervisor`
  - 负责 `stopped / starting / running / stopping / failed`
  - 持有 session 元数据、最近错误、UI ring buffer 与 Tauri 事件转发

## 配置与日志

MCP 相关配置继续走 YAML-first：

- `mcp.bind`
- `mcp.path`
- `mcp.autostart`
- `mcp.log.level`
- `mcp.log.destinations`
- `mcp.log.file.path`
- `mcp.log.ui.buffer_lines`

宿主默认值：

- Desktop 托管 MCP：`ui + file`
- Standalone `agenta-mcp`：`stdout`

日志模型要求：

- MCP 日志与 app shell tracing 分离
- UI 通过 ring buffer snapshot + 增量事件读取，不抓取 stdout
- 文件日志落为 JSONL，默认路径 `<data_dir>/logs/mcp.jsonl`

## 不在本阶段内

以下能力明确留到后续阶段：

- tray / 常驻后台
- sidecar / daemon 化
- 多 session 历史
- 日志轮转
- `stdio` 扩展 transport
- Desktop 独占业务逻辑
