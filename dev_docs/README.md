# Agenta Docs Index

当前正式文档只认下面几份：

- [baseline.md](/e:/JetBrains/RustRover/agenta/dev_docs/baseline.md): 产品基线、MVP 范围、核心对象、能力边界
- [tech.md](/e:/JetBrains/RustRover/agenta/dev_docs/tech.md): 技术架构、代码组织、存储与接口策略
- [deps_build.md](/e:/JetBrains/RustRover/agenta/dev_docs/deps_build.md): 依赖、feature、构建与打包口径
- [migration_plan.md](/e:/JetBrains/RustRover/agenta/dev_docs/migration_plan.md): 从当前 scaffold 到目标架构的迁移顺序

`dev_docs/draft/` 只保留为历史草稿，不再作为实施依据。

辅助使用文档：

- [cli-mcp-quickstart.md](/e:/JetBrains/RustRover/agenta/docs/cli-mcp-quickstart.md): 当前 CLI / MCP 对外使用面、配置和示例流程

当前默认决策：

- MVP 先交付 CLI + MCP，不把 Desktop 设为首个必交项
- Rust 主线锁定 `SQLx + SQLite`
- MCP 首发以 `streamable_http` 为主，`stdio` 延后补充
- 默认构建不包含向量后端或 sidecar
- 前端包管理保持 `bun`
- 数据库与附件默认落在系统应用数据目录
