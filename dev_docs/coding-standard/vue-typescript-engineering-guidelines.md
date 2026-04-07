# Vue + TypeScript Engineering Guidelines

This document defines transferable frontend engineering standards for Vue 3 + TypeScript applications. It is optimized for long-lived products that need maintainable state management, disciplined abstraction, and predictable performance.

## Scope and Baseline

- Target Vue 3.5+ and TypeScript 5.6+ for modern projects.
- Prefer `script setup` and strict TypeScript settings.
- Keep comments in English.
- Treat the UI as a product system, not a collection of pages and handlers.
- Prioritize explicit state, composable behavior, and measured rendering performance.

## Core Principles

1. **Keep the composition root thin.**
   - `App.vue` or page-level containers should compose features, not implement every workflow detail.
2. **Separate orchestration from presentation.**
   - Presentational components render props and emit events.
   - Composables and services own behavior, effects, and integration logic.
3. **Prefer explicit types over implicit conventions.**
   - Use well-named interfaces, discriminated unions, and generated transport contracts where possible.
4. **Avoid duplicated state.**
   - Store canonical state once.
   - Derive everything else with `computed` or pure selectors.
5. **Performance is a design concern, not a rescue mission.**
   - Large reactive graphs, deep watchers, and all-in-one components create avoidable cost.

## Recommended Architecture

### Component Roles

Use clear role separation:

- **composition root / page container**: wires feature modules, routes, and high-level providers
- **feature container**: owns a single user-facing workflow or panel
- **presentational component**: purely renders and emits user intent
- **composable**: owns reusable reactive behavior and lifecycle-aware effects
- **service / client module**: host bridge, network calls, local persistence, protocol mapping
- **type / contract module**: shared UI-facing types and adapters

### Keep Top-Level Components Small

Soft review triggers:

- If a component is hundreds of lines long and owns unrelated tabs, audio, transport, telemetry, and workflow logic, split it.
- If a component contains more business logic than template logic, move behavior into composables or services.
- If a watcher updates many unrelated refs, the state model likely needs redesign.

### Feature Boundaries

A feature should generally own:

- one dominant workflow,
- one state model,
- a small number of components,
- a minimal public API.

Avoid “misc” or “shared” modules that slowly become dumping grounds.

## TypeScript Standards

- Enable strict typing.
- Prefer exact, intention-revealing types over wide object shapes.
- Use discriminated unions for async/resource state such as `idle | loading | ready | error`.
- Normalize transport payloads at the boundary before they enter UI state.
- Avoid `as` casts unless you are narrowing after a real runtime check.
- Keep UI state types separate from backend contract types when transformation is involved.

## Reactivity Rules

### Choose the Right Primitive

- Use `ref` for scalars and independently replaced values.
- Use `reactive` for cohesive mutable objects with many related fields.
- Use `shallowRef` for heavy external objects, renderer instances, large SDK objects, or DOM-bound adapters.
- Use `computed` for derived values; do not persist derivable labels back into state.

### Watchers Are for Side Effects

Use `watch` and `watchEffect` only for:

- network refreshes,
- host bridge synchronization,
- persistence side effects,
- imperative APIs,
- debounced or cancellable reactions.

Do **not** use watchers to simulate missing state design.

### Cleanup Is Mandatory

- Every listener, timer, stream, and imperative resource must be cleaned up on scope disposal.
- If a composable opens a connection, it must also define its teardown.
- Side effects should be owned by the smallest scope that can safely clean them up.

## Async and Data Flow Standards

### Keep Async Control Flow Explicit

- Every async action should define its loading, success, and error behavior.
- Use `AbortController` or an equivalent cancellation strategy for request races.
- Do not let stale async results overwrite newer state.
- Prefer one clear async coordinator over several independent watchers racing each other.

### Boundary Normalization

- Parse and validate host/backend payloads near the boundary.
- Convert transport naming and nullable rules into UI-facing types once.
- Do not spread backend shape assumptions across components.

### Host / Native Bridge Rules

- Keep desktop/native bridge calls inside dedicated service modules.
- UI components should not know bridge command names, low-level file URL conversion, or runtime transport quirks.
- Fallback behavior must be explicit and observable.

## Abstraction Rules

- Extract a composable when behavior is reused or when a component becomes too orchestration-heavy.
- Extract a service when logic depends on host APIs, fetch, storage, websocket, or protocol translation.
- Do not create a composable for a single computed property.
- Do not over-generalize with “universal hooks” that hide domain meaning.
- Prefer small, feature-specific abstractions over giant shared utility layers.

## State Management Guidance

- Keep canonical state as small as possible.
- Prefer selectors/computed views over duplicating summaries, labels, and counts.
- Group related state transitions into named functions instead of mutating many refs ad hoc.
- When a workflow becomes complex, model it as a reducer-like state machine or a focused store/composable.
- Derived display labels should be generated, not manually synchronized.

## Rendering and Performance Guidance

### High-Value Defaults

- Split large screens into smaller components and lazy-mount secondary panels where appropriate.
- Use stable keys for list rendering.
- Virtualize long lists or logs.
- Keep expensive transformations out of templates.
- Use `shallowRef` or `markRaw` for non-reactive SDK objects and renderer adapters.
- Avoid deep watchers on large nested objects.
- Debounce noisy input or telemetry streams before they reach expensive UI updates.

### Avoid These Performance Traps

- giant page components with broad reactive scope,
- repeated mapping/filtering in templates,
- storing both raw and formatted copies of the same state everywhere,
- many fine-grained watchers that trigger each other indirectly,
- recreating connections or listeners on unrelated state changes,
- turning every incoming transport envelope into full-screen rerenders.

### Measure Before Micro-Optimizing

Only after profiling:

- memoization helpers beyond standard `computed`,
- manual render function tuning,
- custom caching layers,
- aggressive object reuse tricks.

## Error Handling and UX Degradation

- Surface degraded mode explicitly.
- Keep operator-facing detail actionable: what failed, what fallback is active, what can be retried.
- Separate recoverable UI errors from programmer errors.
- Log bridge/protocol errors with enough context for debugging, without leaking sensitive data.
- If fallback rendering or offline mode is active, the UI should state that clearly.

## Testing Standards

- Unit test pure mapping, parsing, selectors, and state transition helpers.
- Component test presentational behavior and emitted events.
- Integration test composables and service modules that coordinate async flows.
- Add regression tests for race conditions, stale updates, fallback activation, and cleanup leaks.
- Validate heavy panels, stream logs, and audio/renderer coordination with manual notes when browser-like tests are insufficient.

## Review Checklist

Before merging, verify:

- top-level components remain composition roots rather than logic sinks,
- async actions are cancellable or race-safe,
- state is canonical and not duplicated unnecessarily,
- watchers are used for side effects, not for patching broken design,
- backend/native bridge details stay inside service modules,
- large external objects are not made deeply reactive without reason,
- fallback and degraded states are visible to the user/operator,
- tests cover both nominal and degraded interactions.

## Common Smells

Refactor when you see these patterns:

- a single component owning most application behavior,
- repeated transport parsing logic across multiple components,
- many refs representing one hidden workflow state machine,
- watchers mutating watchers mutating more watchers,
- backend fetch code embedded directly inside UI components,
- derived labels stored as mutable state,
- UI components calling native bridge commands directly in many places,
- “shared utilities” that know too much about multiple features.
