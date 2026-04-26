# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the Vue 3 frontend: `main.ts` boots the app, `App.vue` is the current root component, and `assets/` holds bundled images. `public/` is for static files. `src-tauri/` contains the desktop shell and Rust backend: `src/`, `capabilities/`, `icons/`, and `tauri.conf.json`. Avoid hand-editing generated or build output such as `dist/`, `target/`, `src-tauri/target/`, and `src-tauri/gen/`.

## Build, Test, and Development Commands
Current Tauri config points at Bun:

- `bun run dev` or `npm run dev`: start the Vite frontend on port `1420`.
- `bun run tauri dev` or `npm run tauri dev`: run the full desktop app with the Tauri shell.
- `bun run build` or `npm run build`: run `vue-tsc --noEmit` and produce the frontend bundle.
- `bun run tauri build` or `npm run tauri build`: create desktop bundles.
- `cargo check --manifest-path src-tauri/Cargo.toml`: validate Rust changes quickly.
- `cargo fmt --manifest-path src-tauri/Cargo.toml`: format Rust code.

## Coding Style & Naming Conventions
- TypeScript/Vue: 2-space indentation, semicolons, double quotes.
- Python: PEP 8, 4-space indentation, snake_case modules/functions.
- Rust: default `rustfmt`; snake_case functions, CamelCase types.
- For project-specific language rules, use `dev_docs/coding-standard/vue-typescript-engineering-guidelines.md`, `dev_docs/coding-standard/rust-engineering-guidelines.md`, and `dev_docs/coding-standard/python-engineering-guidelines.md`.
- When a language-specific guide under `dev_docs/coding-standard/` is more specific than the baseline rules in this file, follow the language-specific guide for that language unless a root-level repository constraint says otherwise.
- Frontend user-facing copy must go through `vue-i18n`; when adding or changing UI text, update both `en` and `zh-CN` in `src/i18n/messages.ts` in the same change, and do not leave raw translation keys or hard-coded labels in Vue views except for backend payload/log content that is intentionally shown verbatim.
- Root-level `UI_DESIGN.md` is the source of truth for the desktop companion UI interaction model, spatial layering, and product-facing shell behavior.
- Root-level `UI_STYLE.md` is the source of truth for desktop UI visual tokens, material rules, motion rules, and anti-patterns.

## Execution Plan Authoring
- Active execution plans under `dev_docs/execution-plans/active/` must be authored and maintained in Chinese.
- When creating a new active plan for the first time, include at minimum: relevant background/context, the proposed solution, phased execution steps, and a continuously maintainable TODO tracker.
- The phased execution section should make stage boundaries explicit so contributors can understand delivery order, handoff points, and rollback scope.
- The TODO tracker must record completion status for every tracked item; remarks are optional but recommended when a task is blocked, descoped, or needs follow-up.
- Prefer practical, handoff-friendly headings in active plans such as `背景`, `方案`, `执行步骤`, and `TODO 追踪` so the document stays easy to scan and keep current.

## Workflow Ergonomics
- When adjacent execution-plan tasks share one code path or one implementation batch, it is acceptable to advance them as a phase bundle instead of pretending only one task moved.
- After each substantive phase, keep three surfaces synchronized: code and verification artifacts, active execution-plan status, and Agenta task notes/statuses.
- When changing an Agenta contract that spans multiple surfaces, update the affected service/domain/storage code, CLI, MCP, desktop bridge or mock bridge, frontend types, and tests in the same batch unless the user explicitly wants an intermediate partial state.
- Before adding a new SQL migration under `src-tauri/migrations/`, verify that the numeric prefix is not already taken.
- For tests that bootstrap isolated runtime configs, pin `project_context.paths` to a temp-local directory so repository-level `.agenta/project.yaml` files do not leak into scope resolution.

## Agenta Workflow
- Use the project-local skill at `.agents/skills/agenta-workflow` for Agenta project, version, task, note, and closeout workflows.
- Treat Agenta as the task-level ledger and closeout surface, not as the project's long-term memory system.
- Read repository-maintained context first: `AGENTS.md`, `README.md`, architecture notes, execution plans, and local skills.
- Select one Agenta operation surface before writes: prefer MCP when Agenta MCP tools are available and the user has not requested CLI; use `agenta` CLI when the user requests command-line operation, MCP is unavailable, or a repeatable batch/verification command is the better fit.
- Before substantial investigation or implementation, reuse or initialize the Agenta project and active version through the selected operation surface.
- For numbered or reusable work, set `task_code`, `task_kind`, and `note_kind` explicitly.
- After each substantive phase, keep code and verification artifacts, active execution plans, and Agenta task notes/statuses synchronized.
- After any Agenta write, read back the affected project, version, task, note, or attachment before continuing.
- If neither Agenta MCP tools nor the `agenta` CLI are available, report the workflow installation/configuration issue instead of silently skipping the ledger.

## Configuration Conventions
- Prefer YAML for persisted runtime configuration across apps, packages, tools, and local services.
- Commit safe templates as `*.example.yaml` or `*.example.yml`; keep machine-local or secret-bearing overrides in `*.local.yaml` or `*.local.yml`.
- Support environment-variable injection inside YAML for secrets and host-specific values instead of committing raw secrets.
- When introducing a new configuration surface, prefer a YAML-first loader with explicit schema validation; keep direct environment-variable reads only as compatibility fallbacks unless there is a strong reason not to.
- Keep configuration semantics explicit and documented near the owning app or package README.

## Serena Usage
- IMPORTANT: If Serena tools are available and `serena.activate_project` has not been called in the current context, call it once first. If Serena tools are unavailable, this requirement does not apply.

## SubAgent Usage
- When SubAgents are available, prefer using them for large, multi-step, or clearly separable tasks instead of keeping all work in a single thread.
- Before delegating substantial work, create a short task decomposition, identify the critical path, and split independent workstreams into bounded SubAgent tasks that can run in parallel.
- Prefer delegating targeted exploration, codebase discovery, or verification work to SubAgents when that helps keep the main thread focused and reduces irrelevant context accumulation.
- Ask exploration-oriented SubAgents to return concise findings, affected paths, assumptions, and recommended next steps so their output stays easy to integrate.
- When multiple SubAgents are used for implementation, keep ownership and write scope explicit to avoid overlapping edits and unnecessary coordination overhead.
- Keep urgent, tightly coupled, or immediately blocking work on the main thread when local execution is faster; use SubAgents to accelerate sidecar work, not to create avoidable orchestration cost.
- Unless the user explicitly requests another model, all SubAgents must use `gpt-5.5` (`GPT5.5`) as the default model.
- Reuse an existing SubAgent only when the follow-up task stays within the same bounded context; otherwise prefer spawning a new narrowly scoped SubAgent.

## Testing Guidelines
There is no dedicated JavaScript test runner configured yet. For frontend changes, treat `bun run build` or `npm run build` as the minimum verification step. For Rust changes, run `cargo check --manifest-path src-tauri/Cargo.toml` and `cargo test --manifest-path src-tauri/Cargo.toml` when tests exist. If you add tests later, keep Vue tests near components or under `src/__tests__/`, and place Rust tests inline or under `src-tauri/tests/`.

## Commit & Pull Request Guidelines
This checkout does not include `.git` history, so no repository-specific commit pattern can be inferred locally. Prefer short imperative or Conventional-style subjects such as `feat: add tray command` or `fix: handle empty greet input`. Pull requests should state whether frontend, Rust, or Tauri permissions changed, list verification commands, and include screenshots for visible UI changes.

## Security & Configuration Notes
When changing native capabilities or shell integration, review `src-tauri/capabilities/` and `src-tauri/tauri.conf.json` together. If the team standardizes on `npm` instead of Bun, update the `beforeDevCommand` and `beforeBuildCommand` entries in `src-tauri/tauri.conf.json` in the same change.
