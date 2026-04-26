---
name: agenta-workflow
description: "Use when managing Agenta as a task-level project ledger: reuse projects and versions, organize implementation tasks, restore task context, advance adjacent tasks in phase-sized batches, append findings/conclusions, synchronize Agenta notes/statuses with code and local plans, verify task state, or close out work through either CLI or MCP."
---

# Agenta Workflow

Use Agenta as a task-level ledger and closeout surface, not as the project's long-term memory system.

## When To Use

Use this skill when the work needs one or more of these outcomes:

- Initialize or reuse an Agenta project for the current repository.
- Create or reuse a stable baseline version and attach later tasks to it.
- Turn module-level exploration into reusable context tasks, index tasks, or conclusion notes.
- Restore historical context from an Agenta task and continue the work.
- Close out state, conclusions, and risks after parallel exploration or implementation.

Project source files come first. Agent hosts should read repository-maintained context such as `AGENTS.md`, `README.md`, architecture notes, execution plans, and local skill files before using Agenta to recover task-level ledger state. Agenta notes should reference those files when useful; they should not duplicate or replace them.

## Operating Modes

This skill has two first-class operating modes:

1. CLI mode

Use this for local scripting, batch operations, quick verification, user-requested command-line workflows, or when Agenta MCP tools are unavailable but the `agenta` CLI is available.

2. MCP mode

Use this when Agenta MCP tools are available in the current environment, or when the work is about tool contracts or integration boundaries.

Do not assume CLI is the default, and do not treat MCP as the only valid entry point. Choose the most direct and stable boundary for the current environment before proceeding.

## Default Loop

1. Select MCP or CLI mode from `references/operating-surfaces.md`.
2. Restore or initialize the Agenta project and active version.
3. Restore any relevant task/index context before making changes.
4. Do the requested work and run the appropriate verification.
5. Sync code/verifications, local execution plans, and Agenta task notes/statuses.
6. Read back every Agenta write before reporting completion.

## References To Read

- Read `references/operating-surfaces.md` first to decide between CLI and MCP.
- Read `references/common-workflow.md` next for shared rules around project reuse, task decomposition, note capture, and status closeout.
- If using CLI mode, read `references/cli-mode.md`.
- If using MCP mode, read `references/mcp-mode.md`.

## Expected Outputs

After using this skill, produce one or more of these artifacts:

- A confirmed reusable Agenta project.
- A stable default baseline version.
- A set of tasks organized around task-level recovery.
- Findings or conclusion notes bound to Agenta tasks.
- Trustworthy task state and knowledge state.
- An index-style task only when a task lane genuinely needs a reusable recovery entry.

## Constraints

- Reuse existing projects and versions before creating new ones.
- Organize tasks around how future contributors will restore context, not only around directory structure.
- Use first-class Agenta fields explicitly: `task_code`, `task_kind`, and `note_kind`.
- When a single implementation batch advances multiple adjacent tasks, update every affected task and note rather than pretending only one task moved.
- Treat local execution plans and Agenta task state as one workflow surface; do not let code, plan docs, and task notes drift for long.
- Parallelize read-only exploration when useful, but keep writes, status updates, and read-back verification serialized.
- Confirm every write by reading back the task, note, attachment, or equivalent state.

## Prompt Examples

- `Use $agenta-workflow to initialize the Agenta project and baseline version for this repository.`
- `Use $agenta-workflow to create module context tasks for this repository.`
- `Use $agenta-workflow to restore this Agenta task context and append follow-up notes.`
- `Use $agenta-workflow to close out this round of work and sync conclusions to Agenta.`
