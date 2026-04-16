# Agenta 实施迁移计划

## 文档定位

本文档回答一个旧草稿没有回答清楚的问题：

如何从当前仓库，走到目标架构。

## 当前起点

当前代码状态：

- `src/App.vue` 已演进为全局壳层，接入侧边栏、全局搜索与 Router 容器
- `src-tauri/src/lib.rs` 已切到共享应用装配入口
- `src-tauri/src/bin/agenta.rs`、`src-tauri/src/bin/agenta-cli.rs` 与 `src-tauri/src/bin/agenta-mcp.rs` 已存在
- 已落 `app / domain / storage / service / search / policy / interface / tauri_app / sync`
- 已接入 SQLite migration、附件落盘、FTS、审批回放、CLI、MCP `streamable_http` 与手动远程副本同步

因此，当前文档重点从“如何开始”转为“如何在首轮里程碑完成后继续推进”。

## 迁移原则

1. 先建立共享服务层，再接 CLI / MCP / Desktop。
2. 先把模块边界做对，再决定是否拆 workspace。
3. 默认功能先围绕 `SQLite + FTS5 + Attachment`，不把向量检索和 sidecar 提前塞进主线。
4. 每个阶段都必须有清晰完成标准，能单独验收。

## Phase 0：文档收敛与决策冻结

目标：

- 形成当前四份正式文档
- 明确权威口径
- 冻结第一阶段要做和不做的事情

完成标准：

- `dev_docs/` 根目录文档被接受为唯一权威
- `dev_docs/draft/` 明确降级为历史参考

## Phase 1：在现有 Rust package 内建立基础业务层

目标：

- 不拆 workspace
- 先在 `src-tauri` 内把领域、存储、服务和配置做起来

建议改动：

- 在 `src-tauri/src/` 下新增 `domain/`、`storage/`、`service/`、`search/`、`policy/`
- 增加 YAML 配置加载和路径解析
- 引入 SQLite 初始化与 migration
- 建立 `Project / Version / Task / TaskActivity / Attachment` 的最小服务
- 去掉 `greet` 示例逻辑

完成标准：

- 能通过 Rust 集成测试或命令入口完成项目、任务、备注和附件的最小读写
- 数据能稳定落到 SQLite 和附件目录
- 代码里已经不存在欢迎模板相关业务逻辑

## Phase 2：CLI 先落地

目标：

- 让 Agenta 先变成一个可脚本化、可回归测试的本地工具

建议改动：

- 新增 `src-tauri/src/bin/agenta-cli.rs`
- 接入 `clap`
- 实现 `project / version / task / note / attachment / search` 命令族
- 默认输出 JSON，保留 `--human` 或文本模式作为补充

完成标准：

- CLI 能完整操作同一份 SQLite 数据
- 输出结构稳定，可被脚本消费
- CLI 与服务层之间没有桌面专属逻辑泄漏

## Phase 3：MCP 接入

目标：

- 在不复制业务逻辑的前提下暴露 MCP tools

建议改动：

- 引入 `rmcp`
- 先实现 `streamable_http`
- 将 `project / version / task / note / attachment / search` 映射为对应 tools
- 统一错误模型和返回结构

完成标准：

- MCP tool action 与 CLI 命令族一一对应
- 调用链路仍然复用同一套服务层
- MCP 不直接操作存储层

## Phase 4：桌面 UI 持续业务化完善

目标：

- 在现有项目工作区与 Runtime 控制台基础上继续扩展真实桌面工作流

建议改动：

- 继续沿用现有 `vue-router + pinia + tailwindcss + @lucide/vue`
- 保持“全局壳层 + 项目工作区 + Runtime”信息结构
- 在已落地页面基础上继续补强：
  - 任务详情深度交互
  - 附件面板与物化操作
  - 搜索结果深链与上下文跳转
  - 更细的审批与运行时诊断反馈

当前阶段的建议是：

- 优先通过 Tauri command 调用共享服务层
- 暂不强制引入 sidecar

完成标准：

- UI 只是共享服务层的薄客户端
- 页面能覆盖核心 CRUD、检索和附件物化
- 不出现桌面专用业务分叉

## Phase 5：搜索、摘要和写策略补齐

目标：

- 让 MVP 从“能存数据”升级到“能被 Agent 真正消费”

建议改动：

- 建立 FTS5 索引表
- 写入和更新 `task_search_summary`
- 写入和更新 `task_context_digest`
- 写入和更新 `activity_search_summary`
- 落动作级写策略
- 为附件物化返回结构化结果

完成标准：

- 搜索能覆盖 task 和 activity
- 搜索结果自带可消费摘要
- 被策略拦截的动作会返回明确原因和下一步建议

## Phase 6：发布与安全收口

目标：

- 在功能稳定后，再处理发布物、sidecar 和 capability 收口

建议改动：

- 清理模板资源
- 明确正式配置文件位置
- 如果确实需要 sidecar，再引入 `tauri-plugin-shell`
- 同步收紧 `src-tauri/capabilities/`

完成标准：

- 默认桌面包不含多余权限
- sidecar 若存在，则已声明白名单和参数范围
- 发布流程与实际运行结构一致

## 当前已完成的实现状态

- 已完成正式文档默认值收口
- 已完成 Rust Foundation、配置、路径、migration 和五个核心对象
- 已完成 CLI 与 MCP `streamable_http` 闭环
- 已完成 FTS5、摘要字段与基础写策略
- 已完成项目工作区、版本/任务/审批页面、全局搜索与 Runtime 控制台
- 已完成面向 PostgreSQL 单远端的手动远程副本同步：`status / outbox / backfill / push / pull`
- 已完成基础单元测试与集成测试
- Desktop 已进入真实业务页面阶段，但仍需继续补强详情交互与更细的产品完成度

## Workspace 拆分触发条件

只有满足下面条件，才建议从单 package 升级到 workspace：

1. CLI 与 MCP 已经稳定存在。
2. 桌面端已不再是唯一入口。
3. 共享服务层边界已经被实代码验证。
4. 单 package 编译、发布或团队协作成本已经明显升高。

在这之前，强行拆 crate 的收益不高。

## 当前已冻结的默认值

1. CLI 与 MCP 仍是 canonical contract，Desktop 已接入首批真实页面但不承载独占逻辑。
2. 数据层直接以 `sqlx + sqlite` 进入 Phase 1。
3. 前端默认命令继续保持 `bun`。
4. 数据库与附件正式根目录默认使用系统应用数据目录，便携模式后续再补。
5. MCP 首发 transport 使用 `streamable_http`，`stdio` 作为后续补充。
6. 远程副本同步当前保持单远端、手动触发，不启用后台自动同步。
