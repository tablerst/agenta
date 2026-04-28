# Agenta Docs Index

当前正式文档以第五阶段归档结果为基线。当前仓库已经落地项目工作区、Runtime 控制台，以及面向 PostgreSQL 单远端的手动远程副本同步骨架；`execution-plans/active/` 目前为空，后续新工作流应另起 active 施工单。

## 正式文档

- [baseline.md](baseline.md)：产品基线、里程碑边界与默认决策
- [architecture.md](architecture.md)：当前可执行架构、入口关系与 Desktop 托管方式
- [tech.md](tech.md)：技术实现与模块组织
- [migration_plan.md](migration_plan.md)：迁移和阶段推进背景
- [../docs/cli-mcp-quickstart.md](../docs/cli-mcp-quickstart.md)：Desktop / CLI / MCP 快速开始
- [../docs/cli-reference.md](../docs/cli-reference.md)：CLI 命令参考、搜索回填和同步命令说明

## 执行计划

- 当前暂无 active 施工单
- [execution-plans/archive/fifth-milestone-remote-replica-sync-foundation.md](execution-plans/archive/fifth-milestone-remote-replica-sync-foundation.md)：第五阶段归档
- [execution-plans/archive/fourth-milestone-test-baseline-and-regression-hardening.md](execution-plans/archive/fourth-milestone-test-baseline-and-regression-hardening.md)：第四阶段归档
- [execution-plans/archive/third-milestone-doc-alignment-and-desktop-host-hardening.md](execution-plans/archive/third-milestone-doc-alignment-and-desktop-host-hardening.md)：第三阶段归档
- [execution-plans/archive/first-milestone-core-cli-mcp.md](execution-plans/archive/first-milestone-core-cli-mcp.md)：第一里程碑归档
- [execution-plans/archive/second-milestone-desktop-mcp-console.md](execution-plans/archive/second-milestone-desktop-mcp-console.md)：第二里程碑归档
- [execution-plans/archive/mcp-tool-contract-min-compat-refactor.md](execution-plans/archive/mcp-tool-contract-min-compat-refactor.md)：MCP Tool Contract 重构归档
- [execution-plans/archive/desktop-project-workspace-scroll-refactor.md](execution-plans/archive/desktop-project-workspace-scroll-refactor.md)：桌面工作区重构归档

## 当前默认决策

- Desktop 产品名保持 `Agenta`，二进制为 `agenta-desktop`
- CLI 正式入口为 `agenta`，`agenta-cli` 作为兼容别名保留
- Standalone MCP 继续使用 `agenta-mcp`
- Desktop 默认承载 MCP 生命周期与 Runtime 控制台；`mcp.autostart=true` 时 setup 后自动拉起，否则保持手动启动
- MCP 发布面继续保持显式工具名 contract，不回退到 `action` 多路复用
- YAML-first 配置继续生效，MCP 日志按宿主类型套用默认 destinations
- 当前远程副本同步能力保持单远端 PostgreSQL 与手动 `status / outbox / backfill / push / pull`；Desktop 支持 `sync.auto.enabled=true` 后的 opt-in 自动同步，冲突先检测并暂停，尚未引入冲突解决 UI
