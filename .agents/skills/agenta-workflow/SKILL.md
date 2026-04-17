---
name: agenta-workflow
description: Use when managing Agenta as a project/context ledger: initialize or reuse projects and baseline versions, organize module context tasks, restore task context, append findings/conclusions, verify task state, or close out work through either CLI or MCP.
argument-hint: 要在 Agenta 中推进的目标，例如：初始化项目、补齐模块上下文、恢复任务上下文、收口任务状态
user-invocable: true
disable-model-invocation: false
---

# Agenta Workflow

用于把 Agenta 当作“项目上下文账本”和“任务收口器”来使用，而不是只把它当一个待办列表。

## 何时使用

在以下场景使用这个 Skill：

- 需要为当前仓库初始化或复用 Agenta 项目
- 需要创建或复用版本基线，并把后续任务挂到正确版本下
- 需要把模块级探索沉淀成上下文任务、索引任务或结论笔记
- 需要恢复某个 Agenta 任务的历史上下文并继续推进
- 需要在多项并行探索后收口状态、结论和风险

## 工作方式

这个 Skill 只有两套主要操作模式：

1. CLI 模式
适用：本地脚本化、批量操作、快速验收，或当前环境没有更直接的 Agenta 工具。

2. MCP 模式
适用：当前环境已经暴露 Agenta MCP tools，或者任务本身就是围绕 tool contract / 集成边界展开。

不要预设 CLI 是默认方式。先判断当前最直接、最稳定的边界，再进入对应模式。

## 先读哪个参考

- 先读 `references/operating-surfaces.md`
  用于判断当前该走 CLI 还是 MCP。
- 再读 `references/common-workflow.md`
  这里放所有模式共用的项目复用、任务拆分、笔记沉淀、状态收口规则。
- 如果当前是 CLI 模式，再读 `references/cli-mode.md`
- 如果当前是 MCP 模式，再读 `references/mcp-mode.md`

## 产出目标

使用这个 Skill 后，应尽量产出以下一种或多种结果：

- 一个已初始化或已确认可复用的 Agenta 项目
- 一个稳定的默认版本基线
- 一组按恢复入口组织好的任务
- 绑定在任务上的 finding / conclusion 笔记
- 可信的任务状态与知识状态
- 一份适合后续恢复上下文的索引型任务

## 核心约束

- 先复用现有项目和版本，再创建新对象
- 任务组织优先服务“后续如何恢复上下文”，而不是只按目录平铺
- 显式使用 Agenta 一等字段：`task_code`、`task_kind`、`note_kind`
- 只读探索可以并行；写入、状态更新、回读确认尽量串行
- 每次写入后都要回读或以等价方式确认成功

## 直接可用的提示语

- `/agenta-workflow 初始化当前仓库的 Agenta 项目与基线版本`
- `/agenta-workflow 为这个仓库排一组模块初始化上下文任务`
- `/agenta-workflow 恢复这个 Agenta 任务的上下文并继续补充笔记`
- `/agenta-workflow 收口本轮任务，把状态和结论同步到 Agenta`
