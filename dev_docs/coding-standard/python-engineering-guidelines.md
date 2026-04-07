# Python Engineering Guidelines

This document defines transferable Python engineering standards for modern production codebases. It targets Python 3.12+ and reflects 2026-era expectations around typing, async safety, and maintainable service architecture.

## Scope and Baseline

- Prefer Python 3.12+ for new work; adopt newer stable versions when the deployment baseline allows.
- Use `pyproject.toml` as the single project entry point for packaging and tooling.
- Prefer the `src/` layout for distributable packages.
- Keep code comments and docstrings in English.
- Treat static typing as part of design, not post-hoc decoration.

## Core Principles

1. **Keep the domain synchronous unless asynchrony is required.**
   - Most business rules do not need `async`.
   - Use `async` at I/O boundaries and orchestration layers.
2. **Make data shapes explicit.**
   - Typed models are required at transport, configuration, and persistence boundaries.
   - Core logic should not pass around loosely structured `dict[str, Any]` without a strong reason.
3. **Favor cohesion over “framework convenience”.**
   - Routes should delegate.
   - Services should orchestrate.
   - Stores/adapters should persist or fetch.
4. **Do not confuse abstraction with indirection.**
   - Introduce protocols, base classes, and factories only at real variability boundaries.
5. **Performance work must preserve readability.**
   - The fastest unreadable code is still expensive if every change becomes risky.

## Recommended Architecture

### Keep Entry Points Thin

Entry points such as CLI bootstrap, ASGI app setup, worker bootstrap, or scheduled job launchers should mainly:

- load validated configuration,
- construct dependencies,
- register routes/jobs/handlers,
- start the runtime.

They should not contain business workflows.

### Separate the System by Responsibility

A transferable split looks like:

- **domain**: entities, value objects, invariants, pure decision logic
- **application**: use cases, orchestration, workflow coordination
- **adapters/infrastructure**: HTTP, database, files, queues, provider SDKs
- **presentation/boundary**: FastAPI routes, CLI commands, background worker handlers

### Module Size Guidance

Soft review triggers:

- If a module grows beyond roughly 300–500 lines, review its responsibilities.
- If a single class coordinates transport, lifecycle, memory/state, plugins, and business decisions, split it.
- Prefer extracting a pure function or focused service before introducing a complex inheritance tree.

## Typing Standards

- Public functions and methods must have explicit type hints.
- Prefer concrete collection types at boundaries (`list[str]`, `dict[str, int]`) and protocols for behavioral contracts.
- Use `Literal`, enums, and discriminated unions for stateful flows.
- Avoid `Any` in core logic; if unavoidable, contain it at the boundary and normalize early.
- Prefer immutable or effectively immutable data models in read-heavy flows.

## Data Model Rules

### Boundary Models vs Domain Models

- Use Pydantic (or equivalent validation models) at transport/configuration boundaries.
- Prefer dataclasses, frozen dataclasses, or small typed objects for domain values when validation has already happened.
- Do not let framework request models leak through the entire system.

### Validation Strategy

- Validate once at the boundary.
- Normalize once before the domain layer.
- Keep repeated normalization logic in dedicated functions, not scattered across handlers.

## Async and Concurrency Standards

### Use Async Deliberately

Use `async` for:

- network I/O,
- websocket streaming,
- async database clients,
- concurrency coordination,
- cancellation-aware orchestration.

Do **not** use `async` for:

- pure computation,
- trivial in-memory getters,
- code that immediately calls blocking libraries anyway.

### Keep Async Boundaries Clean

- Blocking SDK calls must be isolated with `asyncio.to_thread(...)` or replaced with true async clients.
- Never block the event loop with CPU-heavy work, file-heavy loops, or synchronous HTTP calls.
- Use timeouts around external provider calls.
- Every spawned task must have an owner and shutdown path.

### Prefer Structured Concurrency

- Prefer `asyncio.TaskGroup` or an equivalent supervised pattern for sibling tasks.
- Fire-and-forget tasks are forbidden unless they are explicitly supervised, logged, and cancellable.
- Use bounded `asyncio.Queue` instances to express backpressure.
- Cancellation is part of the design, not an error case to ignore.

### Async Review Rules

Before marking an async workflow acceptable, verify:

- where cancellation enters,
- where blocking calls are isolated,
- how queue growth is bounded,
- what happens if one sibling task fails,
- how shutdown or disconnect cleans up resources.

## Error Handling Standards

- Raise typed domain/application exceptions internally when the failure category matters.
- Convert exceptions to HTTP/CLI/UI-facing errors only at the outer boundary.
- Never swallow exceptions silently in background tasks.
- When a fallback path is used, log it as a degraded state rather than pretending success.
- Error messages should identify the failing operation and the affected resource.

## Configuration Standards

- Parse configuration from one well-defined source of truth.
- Keep a distinction between raw config input and validated runtime settings.
- Resolve environment placeholders centrally.
- Fail fast on invalid startup configuration.
- Do not scatter `os.getenv(...)` reads across the codebase.

## Framework Usage Rules

### FastAPI / Web Boundaries

- Route handlers should primarily validate input, call application services, and map errors.
- Do not place workflow logic, provider decision logic, or persistence details directly in route functions.
- Websocket handlers should own subscription lifecycle carefully and must clean up on disconnect.

### Provider / Plugin Boundaries

- Define narrow provider interfaces around what the application actually needs.
- Do not mirror entire external SDKs into your codebase.
- Keep provider-specific data structures inside the adapter unless they are part of the real product contract.

## Performance Guidance

### High-Value Defaults

- Reuse clients and connections instead of recreating them per call.
- Precompile regexes that are used frequently.
- Normalize data once, not in every layer.
- Prefer streaming and incremental processing for large payloads.
- Avoid repeated Pydantic re-validation in hot paths after data is already trusted.
- Keep large mutable in-memory structures bounded and observable.

### Event Loop Safety

- No blocking sleep inside async code.
- No synchronous HTTP requests inside async request handlers.
- No unbounded queues for long-lived streaming systems unless there is a reviewed reason.
- No heavy JSON serialization/deserialization loops in hot paths if a typed in-memory form can be preserved longer.

### Optimize Only with Evidence

Only after profiling or production evidence:

- custom caches,
- object pooling,
- manual serialization shortcuts,
- concurrency fan-out tuning,
- vectorized/string micro-optimizations.

## Testing Standards

- Unit test normalization, ranking, state selection, and other pure logic.
- Integration test transport boundaries, streaming behavior, and provider fallbacks.
- Add regression tests for concurrency bugs, dedup logic, timeout handling, and cancellation behavior.
- Test both successful and degraded paths.
- For async systems, test disconnects, queue overflow policy, and cleanup behavior.

## Review Checklist

Before merging, verify:

- domain logic is not trapped in route handlers,
- `async` is used only where it adds real value,
- blocking calls are isolated from the event loop,
- task ownership and cancellation are explicit,
- Pydantic or transport models do not leak through the full system,
- errors are typed or at least categorized before reaching the boundary,
- configuration reads are centralized,
- tests cover both nominal and degraded flows.

## Common Smells

Refactor when you see these patterns:

- a “god runtime” class that owns every subsystem,
- route handlers that directly perform orchestration,
- `dict[str, Any]` traveling through half the application,
- `async def` wrappers around fully synchronous logic,
- fire-and-forget tasks with no supervision,
- repeated provider fallback logic scattered across modules,
- implicit global state hidden in module-level variables,
- `except Exception:` blocks that convert all failures into vague strings.
