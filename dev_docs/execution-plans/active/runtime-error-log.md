# Release 错误日志落盘

## 背景

当前仓库已有 `mcp.log.*`，但它只服务 MCP host/session 日志。Desktop release 在 Windows 下没有控制台，CLI 也主要依赖 `stderr`，因此启动失败、Tauri command 错误、panic 或 standalone MCP 启动失败缺少稳定落盘入口。

## 方案

新增应用级 JSONL 错误日志 `paths.error_log`，默认 `<data_dir>/logs/error.log`。该日志覆盖 Desktop、CLI、standalone MCP 的外层错误边界和 panic hook，不改变现有 MCP session 日志语义。

日志事件包含时间、入口、组件、动作、错误码、消息、详情和构建信息；panic 事件补充 payload、location 和 backtrace。写入前递归脱敏常见敏感字段，如 password、token、secret、api_key、dsn。

## 执行步骤

1. 扩展配置模型：在 `paths` 下增加 `error_log`，补默认值、相对路径解析和环境变量展开测试。
2. 新增应用级 error log helper：负责 JSONL append、父目录创建、脱敏和 panic hook。
3. 接入入口边界：Desktop bootstrap/Tauri command/background autostart、CLI parse/execute、standalone MCP parse/bootstrap/start/wait。
4. 暴露桌面可见性：`desktop_status` 返回 error log 路径，Runtime 页面展示并支持打开所在目录。
5. 同步模板、README、quickstart 和回归测试。

## TODO 追踪

| 状态 | 项目 | 备注 |
| --- | --- | --- |
| [x] | 配置模型与默认路径 | 已新增 `paths.error_log` 与解析测试 |
| [x] | JSONL error log helper | 已新增 writer、脱敏和 panic hook |
| [x] | CLI / MCP / Desktop 边界接入 | 已覆盖主入口与 Desktop command 错误 |
| [x] | Runtime 页面可见性 | 已展示路径并支持打开目录 |
| [x] | 文档与示例配置 | 已同步模板、README 和 quickstart |
| [x] | 验证命令 | `cargo check`、`cargo test`、`bun run build` 均已通过 |
