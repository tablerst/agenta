# Agenta

Agenta is a local-first task and context service for agent hosts. The current milestone keeps the repository as a single Tauri package while establishing a shared Rust core, SQLite storage, a CLI, and an MCP HTTP server.

## Current Milestone

- Shared Rust core inside `src-tauri`
- SQLite metadata storage and local attachment storage
- CLI entrypoint in `src-tauri/src/bin/agenta.rs`
- MCP `streamable_http` entrypoint in `src-tauri/src/bin/agenta-mcp.rs`
- Desktop shell kept thin and intentionally delayed as a primary delivery surface

## Commands

- `bun run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta -- --help`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp -- --help`

## Configuration

Agenta uses YAML-first runtime configuration:

- committed template: `agenta.example.yaml`
- machine-local override: `agenta.local.yaml`

If no override is provided, runtime data defaults to the system application data directory.
