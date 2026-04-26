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
```

Repository development:

```powershell
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help
cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --human project list
```

## Common Commands

Context:

```powershell
agenta context init --project demo
```

Projects, versions, and tasks:

```powershell
agenta project list
agenta project create --slug demo --name "Demo Project"
agenta version list --project demo
agenta version create --project demo --name "workspace-baseline-2026-04-17"
agenta project update --project demo --default-version <version-id>
agenta task create --project demo --title "Map runtime search flow"
agenta task context --task <task-id> --notes-limit 5 --attachments-limit 3
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

User-operated sync, outside the default Agent workflow:

```powershell
agenta sync status
agenta sync outbox list --limit 20
agenta sync backfill --limit 100
agenta sync push --limit 100
agenta sync pull --limit 100
```

## CLI Mode Guidance

- Use CLI mode for batch verification when it is the most stable boundary.
- Use CLI mode when the user explicitly asks for command-line operation or the Agent Host cannot access MCP tools.
- Run commands from the target project root unless the user or config explicitly points elsewhere.
- Preserve command sequences when the same operation must be repeated.
- After each write, read back the result with the appropriate command.
- Follow `common-workflow.md` for task organization, note style, and status rules.
