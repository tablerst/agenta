# Agenta Docs Index

当前正式文档以已归档的前三阶段为基线，当前 active workstream 聚焦测试状态收敛与回归保护，重点围绕默认验证命令稳定性与核心主线回归基线推进。

## 正式文档

- [baseline.md](/e:/JetBrains/RustRover/agenta/dev_docs/baseline.md)：产品基线、里程碑边界与默认决策
- [architecture.md](/e:/JetBrains/RustRover/agenta/dev_docs/architecture.md)：当前可执行架构、入口关系与 Desktop 托管方式
- [tech.md](/e:/JetBrains/RustRover/agenta/dev_docs/tech.md)：技术实现与模块组织
- [migration_plan.md](/e:/JetBrains/RustRover/agenta/dev_docs/migration_plan.md)：迁移和阶段推进背景
- [../docs/cli-mcp-quickstart.md](/e:/JetBrains/RustRover/agenta/docs/cli-mcp-quickstart.md)：Desktop / CLI / MCP 快速开始

## 执行计划

- [execution-plans/active/fourth-milestone-test-baseline-and-regression-hardening.md](/e:/JetBrains/RustRover/agenta/dev_docs/execution-plans/active/fourth-milestone-test-baseline-and-regression-hardening.md)：第四阶段 active 施工单
- [execution-plans/archive/third-milestone-doc-alignment-and-desktop-host-hardening.md](/e:/JetBrains/RustRover/agenta/dev_docs/execution-plans/archive/third-milestone-doc-alignment-and-desktop-host-hardening.md)：第三阶段归档
- [execution-plans/archive/first-milestone-core-cli-mcp.md](/e:/JetBrains/RustRover/agenta/dev_docs/execution-plans/archive/first-milestone-core-cli-mcp.md)：第一里程碑归档
- [execution-plans/archive/second-milestone-desktop-mcp-console.md](/e:/JetBrains/RustRover/agenta/dev_docs/execution-plans/archive/second-milestone-desktop-mcp-console.md)：第二里程碑归档
- [execution-plans/archive/mcp-tool-contract-min-compat-refactor.md](/e:/JetBrains/RustRover/agenta/dev_docs/execution-plans/archive/mcp-tool-contract-min-compat-refactor.md)：MCP Tool Contract 重构归档
- [execution-plans/archive/desktop-project-workspace-scroll-refactor.md](/e:/JetBrains/RustRover/agenta/dev_docs/execution-plans/archive/desktop-project-workspace-scroll-refactor.md)：桌面工作区重构归档

## 当前默认决策

- Desktop 产品名保持 `Agenta`，二进制为 `agenta-desktop`
- CLI 正式入口为 `agenta`，`agenta-cli` 作为兼容别名保留
- Standalone MCP 继续使用 `agenta-mcp`
- Desktop 默认承载 MCP 生命周期与 Runtime 控制台；`mcp.autostart=true` 时 setup 后自动拉起，否则保持手动启动
- MCP 发布面继续保持显式工具名 contract，不回退到 `action` 多路复用
- YAML-first 配置继续生效，MCP 日志按宿主类型套用默认 destinations
