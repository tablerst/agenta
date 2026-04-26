# Agenta MCP Mode

Use this file when `operating-surfaces.md` selects MCP mode.

## Principles

- Read the available tools first.
- Trust the exposed tool descriptions, input schemas, and output schemas.
- Do not assume an old multiplexed `action + arguments.action` interface still exists.
- Each tool should map to one clear intent.
- Keep the Agent MCP surface task-critical. Runtime, release, and remote sync operations are user/desktop/CLI concerns and should not be expected in the default Agent tool list.

## Common Tool Groups

Context:

- `context_init`

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
- `task_context_get`
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

Activities, search, and recovery:

- `activity_list`
- `search_query`
- `search_evidence_get`

## MCP Usage Habits

- Use `project_list` or `project_get` first to decide whether to reuse an existing project.
- If a new version is supposed to be the active lane, verify and update `version.status` plus `project.default_version` before implementation starts.
- After project/version initialization, run `context_init` when the workspace context manifest needs to exist or be refreshed.
- When restoring context, prefer `task_context_get` if a task id is known; use `include_notes=false`, `include_attachments=false`, and a small `recent_activity_limit` for a lightweight first pass, then add `notes_limit` / `attachments_limit` when expanding.
- When search returns `evidence_chunk_id` or `evidence_attachment_id`, use `search_evidence_get` for the second-hop evidence text instead of pulling an entire task context.
- Treat `search_query.meta.retrieval_mode` as task-bucket-only: `structured_only`, `lexical_only`, or `hybrid`. Activity hits are currently lexical-only; semantic fallback details live in `semantic_attempted`, `semantic_used`, `semantic_error`, and `semantic_candidate_count`.
- Set `task_code` explicitly when creating numbered tasks.
- Set `task_kind` explicitly when creating context or index tasks.
- Set `note_kind` explicitly when writing notes.
- When one batch advances multiple adjacent tasks, issue explicit `task_update` and `note_create` calls for each affected task instead of only updating the first one.
- After serialized writes, read back the updated task, note, version, or project state before moving on. For task notes, `task_context_get` is usually the most useful readback.

## MCP Mode Guidance

- If task, note, and search schemas are stable enough, do not fall back to shell commands unnecessarily.
- If the user task is about MCP integration, schemas, or tool contracts, stay in MCP mode.
- Follow `common-workflow.md` for task decomposition, note structure, and closeout rules.
