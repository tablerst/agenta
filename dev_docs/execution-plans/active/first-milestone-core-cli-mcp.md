# Agenta 首轮落地执行计划

## 背景

当前仓库已经完成首轮主线落地：

- Rust 侧已经建立共享业务骨架
- SQLite、附件落盘、CLI、MCP `streamable_http` 已接通
- Desktop 已从模板欢迎页切换为里程碑状态壳
- 根文档与 `dev_docs/` 已按 `CLI + MCP` 首发口径收敛

当前文档的作用不再是“准备开工”，而是记录首轮里程碑已完成的内容，并明确下一阶段应如何推进。

## 方案

当前阶段统一按以下口径实施：

- 首个里程碑交付 `Core + SQLite + Attachment + CLI + MCP`
- MCP 首发 transport 使用 `streamable_http`
- 数据库与附件默认落在系统应用数据目录
- 前端默认包管理保持 `bun`
- Desktop 继续保留为可运行壳，不进入首个里程碑主交付

实现路径：

1. 先统一正式文档与执行单，冻结默认决策。
2. 在 `src-tauri` 内建立 `app / domain / storage / service / search / policy / interface / tauri_app` 模块边界。
3. 先落配置、路径、SQLite migration、五个核心对象和统一结果骨架。
4. 先补 CLI，再补独立 MCP HTTP 入口，复用同一套服务层。
5. CLI/MCP 主闭环跑通后，再补 FTS5、摘要字段和写策略。
6. 最后再把 Desktop 接回真实 contract。

当前实现结果：

- 已完成 1 至 5，并补上了基础集成测试
- Desktop 仍然只到“薄壳 + 状态占位”阶段，尚未接回真实业务界面

## 执行步骤

### Phase 0：文档与默认值收口

- 更新 `dev_docs/` 正式文档中的默认决策
- 新增本执行计划作为当前唯一施工单
- 明确 `streamable_http` 覆盖此前 `stdio` 默认值

完成标准：

- 正式文档之间不再出现 MCP 首发 transport 冲突
- 团队可直接按本文启动实现

### Phase 1：Foundation 与共享业务骨架

- 更新 `src-tauri/Cargo.toml` 依赖，补 `serde_yaml`、`thiserror`、`uuid`、`time`、`tracing`、`tokio`、`sqlx`
- 建立应用装配、配置加载和路径解析
- 引入系统应用数据目录主线和 YAML-first 配置
- 去除 `greet` 风格的示例业务依赖

完成标准：

- 可以独立初始化运行时目录、配置和数据库连接
- 共享业务层与 Tauri 示例逻辑解耦

### Phase 2：Schema、Repository、Service

- 建立五个核心对象的 SQLite schema 与 migration
- 实现 repository 与统一写路径
- 实现统一错误模型与成功/失败 JSON 骨架
- 打通附件元数据与文件落盘

完成标准：

- 通过 Rust 测试验证最小 CRUD 和附件落盘闭环

### Phase 3：CLI 与 MCP 主闭环

- 新增 CLI binary，固定命令族 `project / version / task / note / attachment / search`
- 默认输出 JSON，文本输出只做补充
- 新增独立 MCP HTTP binary 或等价入口
- MCP tools 与 CLI 命令族对齐，并复用同一套 service

完成标准：

- 不依赖 Desktop，也能通过 CLI 和 MCP 操作同一份 SQLite 数据
- MCP 默认只监听本机回环地址

### Phase 4：搜索、摘要与写策略

- 落 FTS5、`task_search_summary`、`task_context_digest`、`activity_search_summary`
- 实现统一 `search` service
- 实现动作级写策略 `auto | require_human | deny`

完成标准：

- 搜索结果可直接消费
- 策略阻断结果结构化可回放

### Phase 5：Desktop 接回真实 contract

- 保持当前 Desktop 可运行
- 用真实数据与共享服务替换欢迎页示例链路
- 不引入 Desktop 独占业务逻辑

完成标准：

- Desktop 只作为共享 contract 的薄客户端

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 收口首个里程碑默认决策 | 已确定 CLI+MCP 优先、HTTP MCP 优先、系统应用数据目录、`bun`、Desktop 延后 |
| [x] | 同步正式文档默认值 | 已更新 `baseline.md`、`tech.md`、`deps_build.md`、`migration_plan.md`、`README.md` |
| [x] | 建立 Rust Foundation 依赖与模块骨架 | 已落 `app / domain / storage / service / search / policy / interface / tauri_app` |
| [x] | 建立 YAML 配置与系统应用数据目录主线 | 已落 `agenta.example.yaml`、`agenta.local.yaml` 约定与系统目录默认值 |
| [x] | 落 SQLite migration 与五个核心对象 schema | 已落 migration、FTS 与五个核心对象 |
| [x] | 落 repository、service、统一错误与结果骨架 | 已完成共享 service、CLI/MCP 统一成功/失败骨架 |
| [x] | 新增 CLI 入口并跑通最小闭环 | 已落 `agenta-cli`，并有 CLI 集成验证 |
| [x] | 新增 MCP `streamable_http` 入口并跑通最小闭环 | 已落 `agenta-mcp`，并有 MCP 集成验证 |
| [x] | 补 FTS5、摘要字段与 `search` service | 已完成 task/activity 检索主线 |
| [x] | 补动作级写策略 | 已落默认策略与结构化阻断返回 |
| [ ] | 用真实 contract 替换 Desktop 欢迎页链路 | 当前仅替换为状态壳，尚未进入真实业务页面 |
| [x] | 收敛 CLI/MCP 对外 contract 并补使用文档 | 已补 `attachment.get`、`note.list` 过滤语义与 CLI/MCP 快速手册 |
| [ ] | 规划并落地 Desktop 第一批真实页面 | 建议作为下一阶段主任务 |
