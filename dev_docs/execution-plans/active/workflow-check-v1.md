# Workflow Check V1 执行计划

## 背景

Agent 已经知道要写 note、读回、同步执行计划和 Agenta 台账，但缺少一个轻量只读入口来判断当前台账是否健康、哪些表面已经漂移、下一步应该先做什么。现有 `task_list`、`task_context_get` 和 `search_query` 返回字段较完整，但需要模型自行推理恢复入口、缺口和建议，容易漏闭环。

## 方案

新增 Agent-facing 的 `workflow_check` 原语，首版覆盖 service、MCP、CLI、`agenta-workflow` skill、测试与本计划。它只读，不新增 SQL migration，不改变现有任务和搜索工具输出契约，也不进入桌面 UI。

`workflow_check` 输出稳定的 digest、缺口、warning 和 recommended next actions，并附带 scope、surface status、open tasks、recovery candidates、feedback inbox 与 active execution plan 关联状态。

## 执行步骤

1. 建立 service 层输入输出类型和查询逻辑，默认从 `.agenta/project.yaml` 推断 project，从项目 default version 推断 version。
2. 暴露 MCP `workflow_check` 工具，标记只读，并保证输出顶部字段稳定。
3. 增加 CLI `agenta workflow check`，普通模式输出 JSON envelope，`--human` 输出 10 行以内摘要。
4. 更新 `agenta-workflow` skill 为 `bootstrap -> restore -> execute -> verify -> closeout` 阶段化状态机，并规定 closeout 的 `ledger_delta` 输出。
5. 补充 service、MCP schema、CLI integration 和 skill validation 测试。
6. 完成验证后同步 Agenta 任务 `WorkflowCheck-00` 与 `WorkflowCtx-01`。

## TODO 追踪

| 状态 | 事项 | 备注 |
| --- | --- | --- |
| [x] | 创建 Agenta 任务 `WorkflowCheck-00` | 已挂到 `v0.1.3-release` |
| [x] | 新增 service `workflow_check` | 已实现只读健康判断、缺口、建议、恢复候选和执行计划关联检查 |
| [x] | 新增 MCP 工具 | 已暴露 `workflow_check`，schema 覆盖 digest、missing surfaces、next actions 等稳定字段 |
| [x] | 新增 CLI 命令与 human 输出 | 已新增 `agenta workflow check` 与 10 行以内 `--human` 摘要 |
| [x] | 更新 `agenta-workflow` skill | 已补 `bootstrap -> restore -> execute -> verify -> closeout` 状态机和 `ledger_delta` 要求 |
| [x] | 补充测试 | 已补 service 健康规则、CLI 输出、MCP schema/调用覆盖 |
| [x] | 运行格式化与验证命令 | `cargo fmt`、`cargo check`、workflow/CLI/MCP 定向测试、skill validator、全量 `cargo test` 已通过 |
| [x] | 同步 Agenta closeout | `WorkflowCheck-00` 已写结论并标记 done；`WorkflowCtx-01` 已补可复用结论并读回 |
