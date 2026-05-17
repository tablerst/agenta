# Agent Feedback Fallback

Prefer `feedback_submit` or `agenta feedback submit` so feedback is stored as an Agenta task note.

Use this file only when neither MCP nor CLI writes are available.

## Template

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
