# Agenta CLI Mode

Use this file when `operating-surfaces.md` selects CLI mode.

## Principles

- Primary executable: `agenta`.
- Compatibility alias: `agenta-cli`.
- Standalone MCP executable: `agenta-mcp`.
- Prefer `agenta` unless the user explicitly asks for a compatibility alias.
- CLI is a local scripting, batch operation, and acceptance-check boundary. It is not the default boundary when MCP tools are available and appropriate.

## Common Invocation

Installed binary:

```powershell
agenta --help
agenta --human project list
agenta --config agenta.local.yaml sync status
```

Repository development:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --human project list
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --config agenta.local.yaml sync status
```

## Common Commands

Projects, versions, and tasks:

```powershell
agenta project create --slug demo --name "Demo Project"
agenta version create --project demo --name "workspace-baseline-2026-04-17"
agenta task create --project demo --title "Map runtime search flow"
agenta task list --project demo
agenta task update --task <task-id> --status done
```

Notes and attachments:

```powershell
agenta note create --task <task-id> --note-kind finding --content "Verified key behavior."
agenta note list --task <task-id>
agenta attachment list --task <task-id>
```

Search:

```powershell
agenta search query --text localgpt --limit 10
agenta search query --project localgpt-langflow --task-code-prefix InitCtx- --limit 20
agenta search backfill --limit 1000 --batch-size 10
```

Sync:

```powershell
agenta sync status
agenta sync outbox list --limit 20
agenta sync backfill --limit 100
agenta sync push --limit 100
agenta sync pull --limit 100
```

## CLI Mode Guidance

- Use CLI mode for batch verification when it is the most stable boundary.
- Preserve command sequences when the same operation must be repeated.
- After each write, read back the result with the appropriate command.
- Follow `common-workflow.md` for task organization, note style, and status rules.
