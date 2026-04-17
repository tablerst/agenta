# Agenta MCP Mode

当 `operating-surfaces.md` 判定当前应走 MCP 时，使用本文件。

## 基本原则

- 先读取 `tools/list`
- 优先相信 MCP 暴露的 tool description、input schema、output schema
- 不要假设仍然存在旧的 `action + arguments.action` 多路复用接口
- 每个 tool 应对应单一意图

## 常见工具组

项目：

- `project_create`
- `project_get`
- `project_list`
- `project_update`

版本：

- `version_create`
- `version_get`
- `version_list`
- `version_update`

任务：

- `task_create`
- `task_create_child`
- `task_get`
- `task_list`
- `task_update`
- `task_attach_child`
- `task_detach_child`
- `task_add_blocker`
- `task_resolve_blocker`

备注 / 附件：

- `note_create`
- `note_list`
- `attachment_create`
- `attachment_get`
- `attachment_list`

搜索：

- `search_query`

## MCP 模式下的使用习惯

- 先用 `project_list` / `project_get` 判断是否复用现有项目
- 恢复上下文时，优先用 `task_list` 或 `search_query`
- 创建编号任务时，显式填写 `task_code`
- 创建上下文 / 索引任务时，显式填写 `task_kind`
- 写笔记时，显式填写 `note_kind`

## 模式内建议

- 如果 task / note / search 的 schema 已经足够稳定，不要额外退回 shell 调 CLI
- 如果用户任务本身就是验证 MCP 接入、schema 或 tool contract，始终留在 MCP 模式
- 真正的任务拆分、笔记结构和收口规则，仍然按 `common-workflow.md`
