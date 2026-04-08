# Agenta Docs Index

当前正式文档以第二里程碑基线为准，重点围绕 Desktop 托管 MCP、CLI 主命令和 Standalone MCP 的分工收口。

## 正式文档

- [baseline.md](/e:/JetBrains/RustRover/agenta/dev_docs/baseline.md)：产品基线、里程碑边界与默认决策
- [architecture.md](/e:/JetBrains/RustRover/agenta/dev_docs/architecture.md)：当前可执行架构、入口关系与 Desktop 托管方式
- [tech.md](/e:/JetBrains/RustRover/agenta/dev_docs/tech.md)：技术实现与模块组织
- [migration_plan.md](/e:/JetBrains/RustRover/agenta/dev_docs/migration_plan.md)：迁移和阶段推进背景
- [../docs/cli-mcp-quickstart.md](/e:/JetBrains/RustRover/agenta/docs/cli-mcp-quickstart.md)：Desktop / CLI / MCP 快速开始

## 执行计划

- [execution-plans/active/second-milestone-desktop-mcp-console.md](/e:/JetBrains/RustRover/agenta/dev_docs/execution-plans/active/second-milestone-desktop-mcp-console.md)：第二里程碑 active 基线
- [execution-plans/archive/first-milestone-core-cli-mcp.md](/e:/JetBrains/RustRover/agenta/dev_docs/execution-plans/archive/first-milestone-core-cli-mcp.md)：第一里程碑归档

## 当前默认决策

- Desktop 产品名保持 `Agenta`，二进制为 `agenta-desktop`
- CLI 正式入口为 `agenta`，`agenta-cli` 作为兼容别名保留
- Standalone MCP 继续使用 `agenta-mcp`
- Desktop 默认承载 MCP 生命周期与 Runtime 控制台，但 MCP 默认为手动启动
- YAML-first 配置继续生效，MCP 日志按宿主类型套用默认 destinations
