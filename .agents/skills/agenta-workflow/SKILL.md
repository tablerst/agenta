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

## Default State Machine

Use this workflow as a lightweight state machine. The tool-side `workflow_check` result supplies facts; this skill defines when to call it and how to close the loop.

1. `bootstrap`
   - Select MCP or CLI mode from `references/operating-surfaces.md`.
   - Read repository context first: root agent instructions, README, architecture notes, active execution plans, and local skills.
   - Run `workflow_check` when available to confirm project/version scope, context manifest, feedback route, recovery candidates, open tasks, and execution-plan linkage before substantial work.
2. `restore`
   - Restore or initialize the Agenta project and active version.
   - Read the relevant task, context, or index task before making changes.
   - Minimum output: current project/version/task scope, chosen recovery entry, and any warnings or missing surfaces.
3. `execute`
   - Do the requested code, documentation, or investigation work.
   - Keep adjacent tasks together when they share one implementation batch.
   - Minimum output: affected tasks and files, plus the implementation or investigation conclusion.
4. `verify`
   - Run the appropriate verification commands.
   - Update any local execution plan that exists.
   - Minimum output: commands run, results, and any residual risks.
5. `closeout`
   - Append notes and update statuses for every directly affected Agenta task.
   - Read back every Agenta write before reporting completion.
   - Produce a `ledger_delta` in the final report: updated tasks, notes, verification commands, remaining risks, and the next recovery entry.
   - If Agenta itself, this skill, or the selected operating surface caused friction, submit concise Agent feedback through `references/feedback-loop.md`.

## References To Read

- Read `references/operating-surfaces.md` first to decide between CLI and MCP.
- Read `references/common-workflow.md` next for shared rules around project reuse, task decomposition, note capture, and status closeout.
- If using CLI mode, read `references/cli-mode.md`.
- If using MCP mode, read `references/mcp-mode.md`.
- Read `references/feedback-loop.md` when an Agent should report Agenta workflow, tool, documentation, or usability feedback.

## Expected Outputs

After using this skill, produce one or more of these artifacts:

- A confirmed reusable Agenta project.
- A stable default baseline version.
- A set of tasks organized around task-level recovery.
- Findings or conclusion notes bound to Agenta tasks.
- Agent feedback notes routed to a configured feedback inbox task when the Agenta workflow itself needs improvement.
- Trustworthy task state and knowledge state.
- An index-style task only when a task lane genuinely needs a reusable recovery entry.
- A `ledger_delta` at closeout for substantive work: task ids/codes updated, note kinds written, verification commands, remaining risks, and next recovery entry.

## Constraints

- Reuse existing projects and versions before creating new ones.
- Organize tasks around how future contributors will restore context, not only around directory structure.
- Use first-class Agenta fields explicitly: `task_code`, `task_kind`, and `note_kind`.
- When a single implementation batch advances multiple adjacent tasks, update every affected task and note rather than pretending only one task moved.
- Treat local execution plans and Agenta task state as one workflow surface; do not let code, plan docs, and task notes drift for long.
- Prefer `workflow_check` as the lightweight read-only health check when available. Use its digest first, then expand with `task_context_get`, `task_list`, or search only when needed.
- Parallelize read-only exploration when useful, but keep writes, status updates, and read-back verification serialized.
- Confirm every write by reading back the task, note, attachment, or equivalent state.

## Prompt Examples

- `Use $agenta-workflow to initialize the Agenta project and baseline version for this repository.`
- `Use $agenta-workflow to create module context tasks for this repository.`
- `Use $agenta-workflow to restore this Agenta task context and append follow-up notes.`
- `Use $agenta-workflow to close out this round of work and sync conclusions to Agenta.`
