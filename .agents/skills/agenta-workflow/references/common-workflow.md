# Agenta Common Workflow

This file defines workflow rules shared by CLI mode and MCP mode. Regardless of the entry point, organize Agenta projects, versions, tasks, and notes according to these rules.

## 1. Classify The Scenario

First decide which scenario the request belongs to:

- Project initialization: the repository does not yet have a matching Agenta project or version.
- Context setup: module-level tasks and navigation notes need to be created.
- Task progress: an existing task needs reading conclusions, design conclusions, or implementation progress.
- Task closeout: the current task is done and needs status, summary, and closure verification.
- Index capture: multiple pieces of context need to be summarized into a persistent entry point.

## 2. Scenario Playbooks

Use these playbooks as default call order. Adapt field names to the selected operating surface, but keep the read-before-write and write-readback shape.

### Project Initialization

1. Select MCP or CLI mode.
2. List projects and reuse a matching project before creating one.
3. List versions for that project and reuse an active/default baseline when appropriate.
4. Create a baseline version only when no suitable version exists.
5. Set the project default version when this is the active lane.
6. Run `context_init` or the CLI `context init` equivalent for the workspace.
7. Create or reuse an index/context task for future recovery.
8. Write a conclusion note that records the project slug, active version, index task, and recovery path.
9. Read back the project, version, task, and note state.

### Context Restore

1. Prefer an explicit task id, task code prefix, project, or version from the user or local plan.
2. Use sorted task listing or search to find the recovery task.
3. Read full task context, including notes and attachments when available.
4. Summarize the reusable conclusions, relevant files, and open risks before continuing.
5. Do not create replacement tasks when an existing reusable context task already fits.

### Phase Closeout

1. Finish code, documentation, and verification first.
2. Update any local execution plan that exists.
3. Append one note per directly affected Agenta task.
4. Update task status only when the task state truly changed.
5. Read back the updated task or task context.
6. Report the verification commands and the Agenta task ids that were updated.

### Version Closeout

1. Check open tasks for the version before closing it.
2. Write or update a version-level index task when future recovery needs one.
3. Add a conclusion note with delivered scope, verification, known risks, and rollback notes.
4. Mark the version closed only after the closeout state is reusable.
5. If a new lane should become active, set the new version active and update the project default version.
6. Read back version and project state.

## 3. Read Current State Before Initialization

If the goal is initialization:

1. List existing projects first.
2. Check whether a project or similar slug already matches the current repository.
3. Reuse the existing project if one exists.
4. Create a project only when no suitable project exists.
5. Create a baseline version and set it as the default version when appropriate.

Recommended naming:

- Project: repository name or a readable product/project name.
- Slug: stable, short, and convenient for scripts or tools.
- Version: a baseline name such as `workspace-baseline-YYYY-MM-DD`.

If the request assumes a new version is now the active lane:

1. Read the current project default version and relevant version statuses first.
2. If the previous version was intentionally closed, verify that state instead of assuming it.
3. Mark the target version `active` and update the project default version before starting implementation, so later tasks inherit the correct lane.

## 4. Decompose Tasks Around Context Recovery

Do not flatten tasks only by directory. Prefer the recovery entry points future contributors will use most often:

- Startup and runtime baseline.
- API routes and domain boundaries.
- Graph execution and component initialization chain.
- Service layer and dependency injection.
- MCP, EDC, Skills, and VFS integration boundaries.
- AI chat, streaming, and upload boundaries.
- Tracing, Langfuse, and observability.
- Evaluations capability and compatibility boundaries.
- Test entry points and high-risk regression areas.
- Persistent summary or context index.

Use first-class fields explicitly when creating tasks:

- Numbered tasks: set `task_code`, for example `InitCtx-01`.
- Normal execution tasks: set `task_kind=standard`.
- Module context tasks: set `task_kind=context`.
- Summary, navigation, or persistent index tasks: set `task_kind=index`.

When restoring a numbered task set, prefer a sorted task list or task-code prefix search instead of guessing from fuzzy titles.

When adjacent tasks share one code path or one implementation batch:

- It is acceptable to progress them together as a phase bundle.
- Keep ownership explicit: note which tasks were directly advanced and which ones only received enabling work.
- Update every affected task after the batch; do not leave nearby tasks stale just because the code change started from one task.

## 5. Parallel And Serial Work

Safe to parallelize:

- Read-only exploration.
- Multi-module information gathering.
- Subagent reading and summarization.

Keep mostly serial:

- Creating projects or versions.
- Final naming and ordering checks when creating task batches.
- Writing notes to tasks.
- Updating task status.
- Reading back state to confirm writes.

When a phase-level batch finishes, sync these surfaces in order:

1. Code and verification artifacts.
2. Local execution plan status if one exists.
3. Agenta task statuses and notes.

Do not defer this synchronization for long-running threads unless there is a strong reason.

For Agenta repository work specifically, treat cross-surface contract changes as one batch:

- service/domain/storage
- CLI
- MCP
- desktop bridge / mock bridge
- frontend types or filters
- tests and regression fixtures

Avoid landing only one surface and planning to “catch up later” unless the user explicitly wants an intermediate partial state.

## 6. Task Note Style

When appending notes, write reusable context rather than a chat transcript.

Set `note_kind` explicitly:

- `scratch`: temporary draft or process note.
- `finding`: verified finding, usually the default.
- `conclusion`: reusable conclusion.

Recommended structure:

1. Topic and date.
2. Verified key conclusions.
3. Recommended reading order.
4. Key files.
5. Main risks, contracts, or cautions.
6. Recommended verification path when useful.

Writing rules:

- Lead with conclusions, not only file names.
- File paths should help future readers locate the relevant code directly.
- Explain why a risk is risky.
- Use `note_kind=conclusion` when the note is reusable as a conclusion.

## 7. Decision Rules

### When To Create A New Task

Create a new task if any condition is true:

- The module will be revisited repeatedly.
- The topic has an independent risk boundary.
- The conclusion is enough to become the entry point for the next work session.
- The content does not fit cleanly as an addendum to another task.

### When To Only Append A Note

Only append a note if:

- This work only adds context to an existing task.
- No new independent topic was produced.
- The work only incrementally improves an existing navigation task.

### When To Mark Done

Mark a task as `done` only when these conditions are mostly true:

- Notes contain enough context for future recovery.
- Project, version, and task ownership are correct.
- The current goal is closed.
- Task status and notes were read back and confirmed after writing.

If the task was just created and does not yet contain useful context, keep it `ready` or `in_progress`.

## 8. Final Checks

- The project exists and the slug is correct.
- The default version is set when needed.
- New tasks are attached to the correct version.
- Numbered tasks have `task_code`.
- Context and index tasks have the correct `task_kind`.
- Notes use `note_kind` to mark scratch, finding, or conclusion.
- Status matches the true completion state.
- Writes were confirmed by reading back the resulting state.

## 9. Avoid These Anti-Patterns

- Creating task titles without useful notes.
- Creating many tasks without clear ordering or numbering.
- Assuming writes succeeded without reading back.
- Putting everything into one giant task.
- Running multiple write operations in parallel and causing storage lock conflicts.
- Marking exploratory tasks as `done` too early.
