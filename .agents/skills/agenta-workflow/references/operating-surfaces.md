# Agenta Operating Surfaces

本文件只负责一件事：判断当前该走 CLI 还是 MCP。

## 选择顺序

优先选择当前环境里**最直接、最稳定、最少额外翻译层**的 Agenta 边界。

### 1. MCP 模式

优先使用 MCP，如果满足任一情况：

- 当前环境已经直接暴露 Agenta MCP tools
- 任务关注 tool contract、schema、集成兼容性或宿主行为
- 用户明确要求通过 MCP / tools 方式操作

进入后继续读：`mcp-mode.md`

### 2. CLI 模式

使用 CLI，如果满足任一情况：

- 当前环境没有更直接的 Agenta 工具
- 需要本地脚本化、批量操作或快速验收
- 需要稳定复现一组本地命令
- 用户明确要求命令行方式

进入后继续读：`cli-mode.md`

## 不要这样做

- 不要因为“统一”而默认绕到 CLI
- 不要在已提供 MCP tools 的环境里再手工拼 shell 命令
- 不要把“调用入口”当成核心目标；真正的目标是把项目、版本、任务、笔记和状态组织正确

## 无论哪种模式都要读

模式选定后，都继续读：`common-workflow.md`
