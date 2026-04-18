---
name: agenta-workflow
description: "Use when managing Agenta as a project/context ledger: initialize or reuse projects and baseline versions, organize module context tasks, restore task context, append findings/conclusions, verify task state, or close out work through either CLI or MCP."
---

# Agenta Workflow

Use Agenta as a project context ledger and task closeout surface, not just as a todo list.

## When To Use

Use this skill when the work needs one or more of these outcomes:

- Initialize or reuse an Agenta project for the current repository.
- Create or reuse a stable baseline version and attach later tasks to it.
- Turn module-level exploration into reusable context tasks, index tasks, or conclusion notes.
- Restore historical context from an Agenta task and continue the work.
- Close out state, conclusions, and risks after parallel exploration or implementation.

## Operating Modes

This skill has two operating modes:

1. CLI mode

Use this for local scripting, batch operations, quick verification, or when Agenta MCP tools are unavailable.

2. MCP mode

Use this when Agenta MCP tools are available in the current environment, or when the work is about tool contracts or integration boundaries.

Do not assume CLI is the default. Choose the most direct and stable boundary for the current environment before proceeding.

## References To Read

- Read `references/operating-surfaces.md` first to decide between CLI and MCP.
- Read `references/common-workflow.md` next for shared rules around project reuse, task decomposition, note capture, and status closeout.
- If using CLI mode, read `references/cli-mode.md`.
- If using MCP mode, read `references/mcp-mode.md`.

## Expected Outputs

After using this skill, produce one or more of these artifacts:

- A confirmed reusable Agenta project.
- A stable default baseline version.
- A set of tasks organized around future context recovery.
- Findings or conclusion notes bound to Agenta tasks.
- Trustworthy task state and knowledge state.
- An index-style task suitable for restoring future context.

## Constraints

- Reuse existing projects and versions before creating new ones.
- Organize tasks around how future contributors will restore context, not only around directory structure.
- Use first-class Agenta fields explicitly: `task_code`, `task_kind`, and `note_kind`.
- Parallelize read-only exploration when useful, but keep writes, status updates, and read-back verification serialized.
- Confirm every write by reading back the task, note, attachment, or equivalent state.

## Prompt Examples

- `Use $agenta-workflow to initialize the Agenta project and baseline version for this repository.`
- `Use $agenta-workflow to create module context tasks for this repository.`
- `Use $agenta-workflow to restore this Agenta task context and append follow-up notes.`
- `Use $agenta-workflow to close out this round of work and sync conclusions to Agenta.`
