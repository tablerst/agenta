# Agenta MCP Mode

Use this file when `operating-surfaces.md` selects MCP mode.

## Principles

- Read the available tools first.
- Trust the exposed tool descriptions, input schemas, and output schemas.
- Do not assume an old multiplexed `action + arguments.action` interface still exists.
- Each tool should map to one clear intent.

## Common Tool Groups

Projects:

- `project_create`
- `project_get`
- `project_list`
- `project_update`

Versions:

- `version_create`
- `version_get`
- `version_list`
- `version_update`

Tasks:

- `task_create`
- `task_create_child`
- `task_get`
- `task_list`
- `task_update`
- `task_attach_child`
- `task_detach_child`
- `task_add_blocker`
- `task_resolve_blocker`

Notes and attachments:

- `note_create`
- `note_list`
- `attachment_create`
- `attachment_get`
- `attachment_list`

Search:

- `search_query`

## MCP Usage Habits

- Use `project_list` or `project_get` first to decide whether to reuse an existing project.
- If a new version is supposed to be the active lane, verify and update `version.status` plus `project.default_version` before implementation starts.
- When restoring context, prefer `task_list` or `search_query`.
- Set `task_code` explicitly when creating numbered tasks.
- Set `task_kind` explicitly when creating context or index tasks.
- Set `note_kind` explicitly when writing notes.
- When one batch advances multiple adjacent tasks, issue explicit `task_update` and `note_create` calls for each affected task instead of only updating the first one.
- After serialized writes, read back the updated task or note state before moving on.

## MCP Mode Guidance

- If task, note, and search schemas are stable enough, do not fall back to shell commands unnecessarily.
- If the user task is about MCP integration, schemas, or tool contracts, stay in MCP mode.
- Follow `common-workflow.md` for task decomposition, note structure, and closeout rules.
