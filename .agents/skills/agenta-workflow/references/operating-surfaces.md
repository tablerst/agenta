# Agenta Operating Surfaces

This file has one purpose: decide whether to use CLI mode or MCP mode.

## Selection Order

Choose the most direct, stable Agenta boundary with the fewest translation layers in the current environment.

### 1. MCP Mode

Prefer MCP mode if any condition is true:

- Agenta MCP tools are directly available in the current environment.
- The task focuses on tool contracts, schemas, integration compatibility, or host behavior.
- The user explicitly asks to operate through MCP or tools.

After choosing MCP mode, read `mcp-mode.md`.

### 2. CLI Mode

Use CLI mode if any condition is true:

- No more direct Agenta tool boundary is available.
- The task needs local scripting, batch operations, or quick acceptance checks.
- The task needs a stable command sequence that can be repeated.
- The user explicitly asks for command-line operation.

After choosing CLI mode, read `cli-mode.md`.

## Avoid

- Do not default to CLI only for consistency.
- Do not hand-build shell commands when MCP tools are already available and appropriate.
- Do not treat the invocation surface as the goal. The goal is to organize projects, versions, tasks, notes, and state correctly.

## Always Read

After selecting a mode, also read `common-workflow.md`.
