# Agenta Feedback Loop

Use this file when an Agent should report friction about Agenta itself: this skill, MCP tools, CLI commands, desktop bridge behavior, documentation, or overall workflow ergonomics.

Feedback is not ordinary task closeout. Task conclusions still belong on the task being worked. Feedback belongs in a feedback inbox so maintainers can periodically triage product and workflow improvements.

## When To Submit Feedback

Submit feedback when one of these happened:

- The skill instructions were confusing, too vague, or forced extra guessing.
- MCP or CLI output was too noisy, missing a key field, or hard to act on.
- A tool name, schema, argument, error, or readback path caused avoidable friction.
- Documentation contradicted actual behavior or omitted a required step.
- The Agent had to invent a local convention that should be built into Agenta.

Do not submit feedback for normal implementation findings, project risks, or feature conclusions. Those should stay on the relevant task as `finding` or `conclusion` notes.

## Preferred Route

1. Check the current project context manifest for feedback routing:
   - `feedback_task_id`
   - `feedback_task_code`
   - `feedback_file`
2. If MCP mode is selected and `feedback_submit` is available, call it.
3. If CLI mode is selected, use `agenta feedback submit`.
4. If no Agenta write surface is available, append the same template to the configured `feedback_file`, usually `.agenta/feedback.md`.

Default feedback task code is `AgentFeedback-00`. If the feedback task is missing and the selected tool supports creation, let the tool create it.

## Feedback Template

Keep feedback short and evidence-backed:

```markdown
# Agent Feedback

- surface: skill | mcp | cli | desktop | docs | other
- severity: low | normal | high
- title: <short title>

## Friction
<what was unclear, noisy, missing, or hard to use>

## Expected
<what should have happened>

## Suggested Change
<specific improvement if known>

## Evidence
<tool call, command, file path, or short snippet>
```

Use `note_kind=finding` for raw feedback. Maintainers should add `conclusion` notes only after triage.

## MCP Example

```text
feedback_submit(
  project="demo",
  surface="mcp",
  severity="normal",
  title="task_context_get returns too much by default",
  friction="The Agent had to parse notes, attachments, and activities when it only needed the task digest.",
  expected="A lightweight first read should be easy to discover.",
  suggested_change="Mention include_notes=false and include_attachments=false in the tool description.",
  evidence="task_context_get default call during context restore"
)
```

## CLI Example

```powershell
agenta feedback submit `
  --project demo `
  --surface skill `
  --severity normal `
  --title "Feedback route was not discoverable" `
  --friction "The Agent did not know where to leave workflow feedback." `
  --expected "The skill should point to a stable feedback inbox." `
  --suggested-change "Document feedback_submit in the default loop."
```
