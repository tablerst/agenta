# Rust Engineering Guidelines

This document defines transferable Rust engineering standards for production-oriented applications and libraries. It is intentionally repository-agnostic: it should remain useful when code moves between products, teams, or runtime hosts.

## Scope and Baseline

- Prefer the current stable Rust toolchain.
- For greenfield work, prefer the latest stable edition available to the organization (Rust 2024+ in 2026-era projects).
- Enforce `rustfmt` and `clippy` in CI.
- Keep comments and API docs in English.
- Optimize for correctness, explicitness, maintainability, and measured performance.

## Core Principles

1. **Model the domain first, not the transport.**
   - Represent business states with enums and typed structs.
   - Keep JSON/YAML/protobuf shapes at the boundary.
2. **Use explicit ownership to reduce incidental complexity.**
   - Prefer single ownership and message passing over shared mutable state.
   - Introduce shared state only when there is a clear concurrency or lifecycle need.
3. **Design for replacement at real variability boundaries.**
   - Use traits when multiple implementations are expected.
   - Do not introduce traits only to make code look “abstract”.
4. **Keep modules cohesive and APIs small.**
   - A module should have one dominant reason to change.
   - Public APIs should be smaller than internal implementation surfaces.
5. **Prefer state machines over flag soups.**
   - If the code tracks lifecycle, readiness, degradation, or retries, use enums and typed transition functions instead of multiple booleans and string labels.

## Recommended Code Organization

### Entry Points Should Stay Thin

- `main.rs`, app bootstrap code, and framework adapters should mostly:
  - load configuration,
  - wire dependencies,
  - register commands/routes,
  - start supervised tasks.
- They should not contain core business logic.

### Separate Layers by Responsibility

Use logical separation such as:

- **domain**: core rules, entities, invariants
- **application**: orchestration, workflows, state transitions
- **infrastructure/adapters**: file system, network, database, framework glue
- **presentation/host boundary**: CLI, desktop commands, HTTP handlers, plugin bridge

### Module Size Guidance

Soft review triggers, not hard limits:

- If a file exceeds roughly 300–500 lines, ask whether it hides multiple responsibilities.
- If a type owns configuration parsing, lifecycle supervision, transport emission, persistence, and business decisions, it should likely be split.
- Prefer extracting pure helper functions before introducing a new type.

## Types and Data Modeling

### Prefer Semantic Types

- Use newtypes or dedicated structs for IDs, endpoints, paths, counts, and config values when confusion is likely.
- Avoid passing loosely structured maps through core logic.
- Use `Option<T>` only when absence is a legitimate state.
- Use `Result<T, E>` for recoverable failures; avoid sentinel values.

### Serialization Boundaries

- Deserialize into raw boundary structs first when input is untrusted or versioned.
- Convert raw structs into validated runtime structs before use.
- Keep serde attributes close to boundary models, not deeply mixed into core domain logic.

## Abstraction Rules

### When to Use Generics

Use generics when:

- behavior is selected at compile time,
- the abstraction is performance-sensitive,
- call sites are few and well understood.

### When to Use Trait Objects

Use trait objects when:

- implementations are chosen at runtime,
- plugins/providers are loaded dynamically,
- API stability matters more than monomorphized performance.

### Avoid Premature Indirection

Do not introduce:

- traits with only one foreseeable implementation,
- builder patterns for simple value objects,
- deep wrapper stacks that hide control flow,
- generic utility modules that erase domain meaning.

## Error Handling Standards

### Use Typed Errors Internally

- Prefer typed error enums for library and domain layers.
- Prefer `thiserror` for application/library error definitions.
- Reserve stringly typed errors for final UI/CLI/HTTP boundaries only.

### Add Context at Boundaries

- Use contextual error messages when crossing file system, process, network, or framework boundaries.
- If an application boundary aggregates heterogeneous failures, `anyhow`-style aggregation is acceptable there.
- Preserve the original cause whenever possible.

### Error Message Quality

A good error message should answer:

- what failed,
- on which resource,
- under what relevant condition,
- what the operator can do next, if actionable.

## Concurrency and Async Rules

### Prefer Structured Concurrency

- Tasks must have an owner.
- Spawns should be supervised, cancellable, and tied to application lifecycle.
- Prefer bounded channels and explicit shutdown signals.

### Shared State

- Prefer task ownership plus message passing over `Arc<Mutex<T>>` when practical.
- If shared state is necessary:
  - keep lock scope minimal,
  - never hold a blocking lock across slow I/O,
  - never hold an async lock across unrelated awaits,
  - document invariants near the lock owner.

### Blocking Work

- Do not run blocking file system, subprocess, or CPU-heavy work on async executors without isolation.
- Offload blocking operations to dedicated threads or blocking task pools.

### Retry and Supervision

- Retries must be bounded and observable.
- Health monitoring should be stateful, not a blind loop.
- Backoff, thresholds, and degradation states should be explicit constants or config.

## Configuration Standards

- Keep a distinction between:
  - raw config input,
  - validated runtime config,
  - live mutable runtime state.
- Validate configuration eagerly during startup.
- Fail fast on invalid config; degrade gracefully only when the product explicitly requires it.
- Keep environment interpolation centralized and testable.

## Logging, Metrics, and Observability

- Prefer structured logging over concatenated strings.
- Include trace or request identifiers on cross-boundary workflows.
- Emit lifecycle transitions explicitly.
- Do not log secrets, tokens, or raw user-sensitive payloads by default.
- Make degraded paths visible; silent fallback is a bug in observability.

## Performance Guidance

### Write Simple Code First, Then Measure

- Start with clear, idiomatic code.
- Profile before introducing unsafe code, custom allocators, or micro-optimizations.
- Record why a low-level optimization exists.

### Preferred High-Value Optimizations

- Avoid unnecessary cloning on hot paths.
- Prefer borrowing (`&str`, slices, references) where ownership transfer is not needed.
- Pre-allocate when size is known or bounded.
- Use enums instead of parsing status strings repeatedly.
- Batch I/O and event emission where correctness allows.
- Keep serialization at the edges to avoid repeated encode/decode churn.

### Treat These as Advanced, Not Default

Only after measurement:

- `SmallVec`, arenas, interning, manual buffer reuse
- lock-free data structures
- unsafe optimizations
- custom task scheduling

## Testing Standards

- Unit test pure logic aggressively.
- Integration test boundary behavior: config loading, process supervision, transport contracts, and failure recovery.
- Add regression tests for lifecycle bugs, retry behavior, and path normalization issues.
- For state machines, test allowed and forbidden transitions.
- For config loaders, test defaults, overrides, invalid inputs, and path resolution.

## Review Checklist

Before merging, verify:

- domain types are explicit and not stringly typed,
- entry points remain thin,
- error types are actionable,
- async tasks have owners and shutdown behavior,
- locks are minimal and justified,
- degraded/fallback paths are observable,
- tests cover core transitions and boundary validation,
- abstractions match real variability rather than aesthetic preference.

## Common Smells

Refactor when you see these patterns:

- one runtime struct owning too many unrelated responsibilities,
- multiple booleans representing a hidden lifecycle state machine,
- `Result<T, String>` escaping deep internal layers,
- repeated `clone()` without a clear ownership reason,
- background loops with no stop condition,
- helper modules named `utils` or `common` that hide domain logic,
- transport payloads reused directly as domain models.
