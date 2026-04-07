# CLI / MCP Quickstart

This document describes the current stable first-milestone surface for Agenta.

## Runtime Config

Agenta loads config in this order:

1. `--config <path>`
2. `AGENTA_CONFIG`
3. `agenta.local.yaml` in the current working directory
4. built-in defaults

Committed template: `agenta.example.yaml`

Current config keys:

- `paths.data_dir`
- `paths.database_path`
- `paths.attachments_dir`
- `mcp.bind`
- `mcp.path`
- `policy.default`
- `policy.actions`

If no config is provided, the database and attachments default to the system application data directory.

## CLI Surface

Binary:

`cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- ...`

Top-level command groups:

- `project`
- `version`
- `task`
- `note`
- `attachment`
- `search`

Current commands:

- `project create|get|list|update`
- `version create|get|list|update`
- `task create|get|list|update`
- `note create|list`
- `attachment create|get|list`
- `search query`

Current CLI conventions:

- Default output is JSON.
- Add `--human` for summary-first output.
- Object references use the existing human-facing identifier for that command:
  `project` uses project slug or project id.
  `version` uses version id.
  `task` uses task id.
  `attachment` get uses attachment id.
- `search query` accepts `--text` and also supports `--query` as an alias.

### Example Flow

Create a project:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  project create --slug demo --name "Demo Project"
```

Create a version:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  version create --project demo --name "v1"
```

Create a task:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  task create --project demo --title "Wire the CLI"
```

Add a note:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  note create --task <task-id> --content "First milestone note"
```

Attach a file:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  attachment create --task <task-id> --path .\sample.log --summary "Build log"
```

Fetch an attachment directly:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  attachment get --attachment <attachment-id>
```

Search:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- `
  search query --query "milestone"
```

## MCP Surface

Binary:

`cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp`

Current transport:

- `streamable_http`

Default bind path:

- bind address from `mcp.bind`
- mount path from `mcp.path`

Default config value today:

- `127.0.0.1:8787`
- `/mcp`

Health endpoint:

- `GET /health`

Current MCP tools:

- `project`
- `version`
- `task`
- `note`
- `attachment`
- `search`

Current MCP action matrix:

- `project`: `create|get|list|update`
- `version`: `create|get|list|update`
- `task`: `create|get|list|update`
- `note`: `create|list`
- `attachment`: `create|get|list`
- `search`: `query`

Current tool argument conventions:

- `project.get` accepts `project` and also supports `slug`.
- `search.query` accepts `text` and also supports `query`.
- `attachment.get` uses `attachment_id`.
- `note.list` returns only note activities, not the full activity timeline.

### Example MCP Tool Calls

Create a project:

```json
{
  "name": "project",
  "arguments": {
    "action": "create",
    "slug": "demo",
    "name": "Demo Project"
  }
}
```

Create a task:

```json
{
  "name": "task",
  "arguments": {
    "action": "create",
    "project": "demo",
    "title": "Wire MCP contract"
  }
}
```

Fetch an attachment:

```json
{
  "name": "attachment",
  "arguments": {
    "action": "get",
    "attachment_id": "<attachment-id>"
  }
}
```

Search:

```json
{
  "name": "search",
  "arguments": {
    "action": "query",
    "query": "milestone",
    "limit": 10
  }
}
```

## Response Shape

CLI success envelope:

```json
{
  "ok": true,
  "action": "task.create",
  "result": {},
  "summary": "Created task",
  "warnings": []
}
```

CLI error envelope:

```json
{
  "ok": false,
  "error": {
    "code": "not_found",
    "message": "resource not found: task ...",
    "details": {}
  }
}
```

MCP tool responses carry the same payload in structured content.

## Current Limits

- Desktop is not yet the primary interaction surface.
- `version`, `task`, and `attachment` direct lookup currently rely on ids except for task/project list filters.
- MCP `stdio` is not the default first transport.
- The desktop shell has not yet been reconnected to full project/task views.
