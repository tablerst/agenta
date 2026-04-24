# Agenta

Agenta is a local-first task and context service for agent hosts. The local desktop baseline, host hardening, regression gate, and the fifth remote-replica milestone are complete; the repository now includes Desktop workspace pages plus a manual PostgreSQL-backed remote replica sync flow.

## Distribution

- Desktop product name: `Agenta`
- Desktop binary: `agenta-desktop`
- Canonical CLI: `agenta`
- CLI compatibility alias: `agenta-cli`
- Standalone MCP binary: `agenta-mcp`

## Current Structure

- Shared Rust core, app runtime, CLI, and MCP server live under `src-tauri`
- Desktop commands and Runtime console live in the Tauri shell and `src/views/RuntimeView.vue`
- Runtime configuration is YAML-first and defaults to system application data directories
- Desktop-managed MCP honors `mcp.autostart`: manual start remains the default, persisted opt-in auto-start is supported, and the host stops gracefully with the app

## Commands

- `bun run dev`
- `bun run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- context init --project demo`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- sync status`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- sync outbox list --limit 20`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- --help`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp -- --help`
- `bun run release -- --dry-run`: preview release metadata and artifact paths.
- `bun run release`: build installers and versioned binary artifacts under `target/release-artifacts/`.

## Verification Baseline

Use this minimum regression gate for normal code changes:

- `bun run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`

## Runtime Configuration

Use `agenta.example.yaml` as the committed template. The MCP section now supports:

- `mcp.bind`
- `mcp.path`
- `mcp.autostart`
- `mcp.log.level`
- `mcp.log.destinations`
- `mcp.log.file.path`
- `mcp.log.ui.buffer_lines`

The project context section now supports:

- `project_context.paths`
- `project_context.manifest`

Agenta reads these paths as project-local context hints only. It does not own or synchronize the files inside those directories.

The sync foundation section now supports:

- `sync.enabled`
- `sync.mode`
- `sync.remote.id`
- `sync.remote.kind`
- `sync.remote.postgres.dsn`
- `sync.remote.postgres.max_conns`
- `sync.remote.postgres.min_conns`
- `sync.remote.postgres.max_conn_lifetime`

When `mcp.log.destinations` is omitted, defaults depend on the host:

- Desktop-managed MCP: `ui + file`
- Standalone `agenta-mcp`: `stdout`

Current sync defaults stay intentionally conservative:

- Only one global remote is modeled
- Sync uses manual `status / outbox / backfill / push / pull`; background auto-sync is still disabled
- Sync delivery/apply follows dependency order: `project -> version -> task -> task_relation -> note -> attachment`; any new synced entity must define its FK/apply ordering together with schema changes
- Status output redacts PostgreSQL credentials, and Runtime exposes the same manual sync actions inside Desktop

## Project Context Scoping

Agenta is a task ledger, not a project memory system. In multi-project environments:

- `task list` and `search query` no longer default to cross-project results
- if the current project context directory resolves a unique project, queries scope to that project automatically
- if only one project exists, queries still scope to it for compatibility
- if multiple projects exist and no unique scope can be resolved, Agenta returns `ambiguous_context`
- cross-project list/search must be explicit via `--all-projects` or `all_projects=true`

Agenta also exposes a unified `context_init` action through CLI, Desktop, and MCP:

- CLI: `agenta context init`
- Desktop: project overview action
- MCP: `context_init`

Use it when a project needs an initial or migrated context directory, especially when the target path does not match the default candidates.

## Search / Chroma Prerequisites

Vector search and `回填搜索索引` depend on a reachable Chroma backend when `search.vector.enabled: true`.

- If `search.vector.autostart_sidecar: true`, Desktop will try to run `chroma` locally. This only works when the Chroma CLI is installed and available on `PATH`.
- If you prefer to run Chroma yourself, start a local server first and keep `search.vector.endpoint` pointed at that server.
- If neither the CLI nor a running server is available, search backfill jobs may be queued but processing will fail until Chroma becomes reachable.
- Queue/runs/failures can be inspected locally via `agenta search status` or the Desktop Runtime search-index panel. Failed jobs can be retried with `agenta search retry-failed`, and expired processing leases can be recovered with `agenta search recover-stale`; embeddings remain local-only derived state and are not replicated through sync.
- SearchV2 release, rollback, and verification guidance lives in [docs/search-v2-release.md](docs/search-v2-release.md).

Official Chroma references:

- CLI install: <https://docs.trychroma.com/docs/cli/install>
- Run local server: <https://docs.trychroma.com/docs/cli/run>

## Documentation

- Quickstart: [docs/cli-mcp-quickstart.md](docs/cli-mcp-quickstart.md)
- CLI reference: [docs/cli-reference.md](docs/cli-reference.md)
- SearchV2 release guide: [docs/search-v2-release.md](docs/search-v2-release.md)
- Latest archived execution plan: [dev_docs/execution-plans/archive/fifth-milestone-remote-replica-sync-foundation.md](dev_docs/execution-plans/archive/fifth-milestone-remote-replica-sync-foundation.md)
- Archived execution plans: [dev_docs/execution-plans/archive](dev_docs/execution-plans/archive)
- Product baseline: [dev_docs/baseline.md](dev_docs/baseline.md)
- Architecture notes: [dev_docs/architecture.md](dev_docs/architecture.md)
