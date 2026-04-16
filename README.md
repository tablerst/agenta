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
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- sync status`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- sync outbox list --limit 20`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- --help`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp -- --help`

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
- Status output redacts PostgreSQL credentials, and Runtime exposes the same manual sync actions inside Desktop

## Documentation

- Quickstart: [docs/cli-mcp-quickstart.md](docs/cli-mcp-quickstart.md)
- Latest archived execution plan: [dev_docs/execution-plans/archive/fifth-milestone-remote-replica-sync-foundation.md](dev_docs/execution-plans/archive/fifth-milestone-remote-replica-sync-foundation.md)
- Archived execution plans: [dev_docs/execution-plans/archive](dev_docs/execution-plans/archive)
- Product baseline: [dev_docs/baseline.md](dev_docs/baseline.md)
- Architecture notes: [dev_docs/architecture.md](dev_docs/architecture.md)
