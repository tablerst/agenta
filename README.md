# Agenta

Agenta (Agentic + Agenda) is a local-first task and context service for agent hosts. The current milestone keeps the repository as a single Tauri package while establishing a shared Rust core, SQLite storage, a CLI, and an MCP HTTP server.

## Current Milestone

- Shared Rust core inside `src-tauri`
- SQLite metadata storage and local attachment storage
- YAML-first runtime configuration with system app data as the default root
- CLI entrypoint in `src-tauri/src/bin/agenta-cli.rs`
- MCP `streamable_http` entrypoint in `src-tauri/src/bin/agenta-mcp.rs`
- Desktop shell wired back to the shared contract with project/task/approval/runtime views

## Commands

- `bun run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- --help`
- `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp -- --help`

## Quick Start

1. Create a local config from `agenta.example.yaml` if you want an explicit data location.
2. Create a project with the CLI:
   `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- project create --slug demo --name "Demo Project"`
3. Add a task:
   `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-cli -- task create --project demo --title "First task"`
4. Start the MCP server:
   `cargo run --manifest-path src-tauri/Cargo.toml --bin agenta-mcp`

For the stable command/tool surface and end-to-end examples, see [docs/cli-mcp-quickstart.md](E:\JetBrains\RustRover\agenta\docs\cli-mcp-quickstart.md).

## Configuration

Agenta uses YAML-first runtime configuration:

- committed template: `agenta.example.yaml`
- machine-local override: `agenta.local.yaml`

If no override is provided, runtime data defaults to the system application data directory.

The config shape currently supports:

- `paths.data_dir`
- `paths.database_path`
- `paths.attachments_dir`
- `mcp.bind`
- `mcp.path`
- `policy.default`
- `policy.actions`

## Current Verification

The current implementation has been verified with:

- `bun run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`
