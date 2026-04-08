# Agenta

Agenta is a local-first task and context service for agent hosts. The current active baseline is the second milestone: the desktop app now carries the MCP host lifecycle and exposes a Runtime console for launch control, structured logs, and failure recovery.

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
- Desktop-managed MCP defaults to manual start from the Runtime console and stops gracefully with the app

## Commands

- `bun run dev`
- `bun run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- --help`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp -- --help`

## Runtime Configuration

Use `agenta.example.yaml` as the committed template. The MCP section now supports:

- `mcp.bind`
- `mcp.path`
- `mcp.autostart`
- `mcp.log.level`
- `mcp.log.destinations`
- `mcp.log.file.path`
- `mcp.log.ui.buffer_lines`

When `mcp.log.destinations` is omitted, defaults depend on the host:

- Desktop-managed MCP: `ui + file`
- Standalone `agenta-mcp`: `stdout`

## Documentation

- Quickstart: [docs/cli-mcp-quickstart.md](/e:/JetBrains/RustRover/agenta/docs/cli-mcp-quickstart.md)
- Active execution plans: [dev_docs/execution-plans/active](/e:/JetBrains/RustRover/agenta/dev_docs/execution-plans/active)
- Product baseline: [dev_docs/baseline.md](/e:/JetBrains/RustRover/agenta/dev_docs/baseline.md)
- Architecture notes: [dev_docs/architecture.md](/e:/JetBrains/RustRover/agenta/dev_docs/architecture.md)
