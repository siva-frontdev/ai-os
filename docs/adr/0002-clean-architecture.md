# ADR-0002: Clean Architecture with Layered Modules

## Status

Accepted

## Date

2025-01-20

## Context

The AI-native OS platform requires a software architecture that supports independent development of modules, allows swapping implementations without ripple effects, and maintains clear dependency direction. The project has multiple developers, a multi-phase roadmap spanning years, and the need to evolve individual subsystems independently.

### Problem

Without an explicit architectural pattern, software systems naturally degrade into a "big ball of mud" where every module depends on every other module, changes have unpredictable side effects, and testing requires the full system. This is especially dangerous for a platform that:

- Has a nine-phase roadmap where earlier phases must remain stable while later phases are developed.
- Must support third-party modules in later phases without access to internal implementations.
- Requires strong security boundaries where privilege escalation through module dependencies must be prevented.
- Needs to be testable at every layer without requiring hardware (GPUs) or external services (LLM APIs).

### Forces

The following forces drive the architecture decision:

1. **Independent development**: Different teams or the same team at different times will work on the AI engine, runtime, tools, and API layers. These efforts must proceed in parallel without blocking each other. A developer working on the AI engine should not need to understand the runtime's scheduling internals.

2. **Testability**: Each module must be testable in isolation. Unit tests should not require spinning up the full system. Modules should be mockable through trait interfaces. Integration tests can assemble subsets of modules.

3. **Swap implementations**: We may need to replace implementations without changing consumers. Examples include switching from one LLM provider to another, changing the storage backend from SQLite to PostgreSQL, or replacing the scheduling algorithm. The architecture should make such swaps local to one module.

4. **Dependency direction**: Higher-level policy (workflows, orchestration, agent coordination) should depend on lower-level mechanisms (storage, inference, networking), not the reverse. The architecture must enforce this direction at compile time.

5. **Crate boundaries**: Rust's module system (Cargo workspaces) provides strong encapsulation boundaries through crate-level visibility. A crate declares its public API through `pub` items; everything else is private. We should leverage crates as the unit of architectural isolation, with the compiler enforcing the boundaries.

6. **Future-proofing**: The architecture must accommodate phases not yet designed (AI Engine, Tools, Session, API Layer) without requiring restructuring of earlier phases. A module in Phase 6 should be addable without modifying any Phase 2 code.

7. **Security**: Module isolation provides a security boundary. A bug in the Tools module should not corrupt the core runtime state. Crate boundaries combined with trait interfaces provide this isolation.

### Layer Definitions

The architecture defines four layers, each implemented as one or more Cargo workspace crates, plus a foundation layer of external dependencies:

```
+----------------------------+
|   API Layer (Phase 8)     |  gRPC, HTTP, WebSocket, CLI, SDKs
+----------------------------+
|   Runtime Layer (Ph 3-4)  |  Scheduler, Supervisor, Session, Task,
|                            |  Context, StateMachine, Resource, Permission
+----------------------------+
|   Core Layer (Phase 2)    |  EventBus, Core types, Error types,
|                            |  Storage traits, Domain events
+----------------------------+
|   AI Engine / Tools / etc |  Future layers (Phases 5-7)
+----------------------------+
|   Foundation              |  Tokio, Serde, tracing, Rust std
+----------------------------+
```

**Foundation** — External crates and the Rust standard library. The platform does not control these but depends on them. Foundation dependencies are managed through the workspace `Cargo.toml` and pinned to specific versions.

**Core Layer** — The innermost platform layer. Contains the `Event` and `EventBus` traits, core domain types (`AgentId`, `SessionId`, `EventId`, `TraceId`), platform error types, storage abstraction traits, configuration loading traits, and domain event type definitions. The Core layer has no knowledge of any layer above it. It depends only on Foundation.

**Runtime Layer** — Implements the eight runtime subsystems (Scheduler, Supervisor, SessionManager, TaskManager, ContextManager, StateMachine, ResourceManager, PermissionChecker). Depends on Core for types and EventBus. Does not depend on AI Engine, Tools, or API layers. Subsystems within Runtime communicate through the EventBus.

**API Layer** — Provides external interfaces: gRPC services, HTTP endpoints, WebSocket connections, CLI commands, and SDK stubs. Translates external protocol messages into internal EventBus events. Depends on Core and Runtime.

**Future Layers (AI Engine, Tools, Session, etc.)** — These are peer layers that depend on Core and potentially on Runtime but not on API. They are added in Phases 5-7.

### Layer Definitions in Detail

**Foundation Layer** comprises all external dependencies: Rust standard library, Tokio (async runtime, I/O, timers, synchronization), Serde (serialization), tracing (observability), clap (CLI), figment (configuration), sqlx (database access), and any other crates pulled from crates.io. Foundation dependencies are managed through the workspace `Cargo.toml` `[workspace.dependencies]` section with pinned versions. A `cargo-deny` configuration file bans duplicate versions and flags security advisories.

**Core Layer** is the innermost platform crate. It contains:
- The `Event` trait and `EventBus` trait (see ADR-0004).
- The `InMemoryEventBus` default implementation.
- Primitive domain types: `AgentId` (newtype over Ulid), `SessionId` (newtype over Ulid), `EventId` (newtype over Ulid), `TraceId` (newtype over uuid v7). All implement `Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize`.
- The `CoreError` enum with variants for common failure modes (NotFound, AlreadyExists, InvalidState, Internal, Bus, Configuration).
- The `Result<T, CoreError>` type alias.
- Storage abstraction traits: `Storage<T>: Get, Put, Delete, List` with async methods.
- Configuration loading traits: `ConfigSource` (File, EnvVar, Default).
- Domain event type identifiers and marker traits.
- The `EventBus` implements `Default` for tests.

Core has **zero platform dependencies**. It depends only on Foundation (Tokio, Serde, tracing, ulid, uuid, dashmap). No crate in Core imports from Runtime, AI Engine, or any other platform layer.

**Runtime Layer** is the largest crate in the initial phases. It implements the eight subsystems defined in ADR-0005. Each subsystem is:
- A trait in `runtime/src/lib.rs`.
- A default implementation in `runtime/src/<subsystem>/default.rs`.
- An event subscription module in `runtime/src/<subsystem>/events.rs`.

Runtime depends on Core for types and EventBus. It does not depend on any higher layer. Subsystems within Runtime communicate through the EventBus, but subsystems within the same crate may share types through `pub(crate)` visibility for performance-sensitive paths.

**Future Layers (AI Engine, Tools, Session, API)** are workspace crates added in Phases 5-8. They depend on Core and may depend on Runtime. They never depend on each other in a way that creates a dependency cycle.

### Crate Naming Convention

Workspace crates follow the naming convention:
- `core` — Core layer.
- `runtime` — Runtime layer (may be split into `runtime-scheduler`, `runtime-supervisor` if needed).
- `ai-engine` — AI Engine layer (Phase 5).
- `ai-tools` — Tools layer (Phase 6).
- `session` — Session layer (Phase 7).
- `api` — API layer (Phase 8).
- `cli` — CLI application binary (Phase 8).

The dashes follow Cargo convention for multi-word crate names. Directory names match crate names.

### Testing Strategy

The layered architecture enables a multi-level testing strategy:

| Test Type   | Scope                          | Dependencies                     | Speed     |
|-------------|--------------------------------|----------------------------------|-----------|
| Unit        | Single function or module      | None (or mocked)                 | ms        |
| Intra-crate | Multiple modules in one crate  | Mock EventBus                    | ms        |
| Inter-crate | Two crates (e.g., core+runtime)| Real EventBus, mock subsystems   | 10-100ms  |
| Integration | Full system (all crates)       | Real implementations             | 100ms-1s  |
| E2E         | System + external services     | LLM API, database, filesystem    | 1s-10s    |

Unit and intra-crate tests are the primary development feedback loop. Inter-crate and integration tests run in CI but are not required for local development.

### Communication Rules

1. All inter-module communication (across layers or within the same layer) uses the EventBus defined in Core.
2. Direct function calls between crates in different layers are prohibited unless explicitly justified in an ADR amendment.
3. The EventBus is defined in Core. All layers can publish and subscribe to events through the bus.
4. Modules define their own event types in their own crates. Event types are not centralized in Core.
5. Each crate's public API consists of: (a) event types it publishes, (b) event handler traits for events it subscribes to, and (c) configuration types.

### Alternatives Considered

**Monolithic crate**: A single crate with internal modules (`mod` declarations). Rejected because there is no compile-time enforcement of layer boundaries. Any module can import any other module. Circular dependencies are possible and often emerge organically.

**Microservices (separate processes)**: Each module as a separate process, communicating over gRPC. Rejected for Phases 1-4 because of: operational complexity (N processes to deploy, monitor, and debug), serialization overhead (protobuf encode/decode per event), latency (network round-trip for every cross-module interaction), and deployment friction (every developer needs N services running). May be reconsidered in Phase 8 for multi-node deployments.

**Plugin system (dlopen/dylib)**: Runtime-loaded dynamic libraries. Rejected because it adds: ABI stability requirements (Rust does not have a stable ABI), loading and unloading mechanics, versioning complexity, and security concerns (malicious plugins). May be revisited in Phase 9 for third-party module loading.

**Hexagonal architecture (ports and adapters)**: Considered in detail. The hexagonal pattern adds formal ports (inbound) and adapters (outbound) interfaces. We chose a simplified clean architecture because: the EventBus already provides the port/adapter decoupling naturally, and the formal hex architecture adds documentation overhead without additional enforcement in Rust (crate boundaries already provide the isolation).

**Shared-kernel architecture**: A single shared core with multiple satellites. This is essentially what we are doing, but with the additional constraint of strict one-way dependency through the EventBus.

## Decision

We adopt a clean layered architecture with the following concrete decisions:

1. **Cargo workspace structure**: The repository is a Cargo workspace with crates organized by layer. The current crates are:

   - `core/` — Core types, EventBus trait and in-memory implementation, error types, configuration traits, storage abstraction traits, domain event types. Zero dependencies on any other workspace crate.
   - `runtime/` — Agent lifecycle management, all eight subsystems. Depends on `core/` only, through Cargo dependency declaration.
   - Future phases will add `ai-engine/`, `tools/`, `session/`, `api/` as separate workspace crates, each depending on `core/` and potentially on `runtime/`.

2. **Dependency direction is enforced at build time**: The workspace `Cargo.toml` declares dependencies explicitly. CI tooling (`ci/check-deps.sh`) validates that `core/` has zero dependencies on any other workspace crate, and that `runtime/` does not depend on any crate outside `core/`.

3. **Each functional module MAY be a separate crate**: Within a layer, functional modules may be split into separate crates (e.g., `runtime-scheduler`, `runtime-supervisor`) at the discretion of the implementing team. The default is a single crate per layer; splitting is motivated by compile time, team boundaries, or independent release cadence.

4. **EventBus is the sole cross-crate communication channel**: All cross-crate communication happens through the EventBus. Modules define their own event types (structs implementing the `Event` trait) in their own crate and publish/subscribe via the bus.

5. **Traits define module boundaries**: Each module exposes its public API through Rust traits defined in the module's crate. Implementations are behind these traits, enabling mocking, testing, and swapping.

6. **No cyclic dependencies**: The crate dependency graph must remain acyclic. CI includes a cycle detection step using `cargo metadata` piped through `petgraph`.

7. **Visibility rules**: Types that cross crate boundaries are `pub`. Internal implementation details are `pub(crate)` or private. Each crate's `lib.rs` re-exports the public API.

## Consequences

### Positive

- **Strong boundary enforcement at compile time** through crate isolation. A developer cannot accidentally create a circular or upward dependency — the compiler and Cargo resolve prevent it.
- **Independent testability**: Each crate can be tested in isolation with mocked EventBus subscriptions and trait implementations. Unit tests in `core/` run without Tokio. Unit tests in `runtime/` use a mock `EventBus`.
- **Parallel development**: Multiple developers can work on different layers simultaneously without merge conflicts on shared code. The crate boundaries are natural work partitioning points.
- **Swap implementations**: The trait-based module boundaries make it straightforward to replace an implementation by implementing the same trait in a new crate and updating the `Runtime` composition.
- **Roadmap alignment**: The layer structure maps directly to the nine-phase roadmap. Each phase adds one or more crates at the appropriate layer.
- **Security isolation**: A vulnerability in a higher-layer crate (e.g., the API crate parsing malicious input) cannot directly corrupt the core data structures. The crate boundary and EventBus act as an isolation layer.
- **Compile-time dependency checking**: If a developer mistakenly adds a dependency from `core/` to `runtime/`, the build fails. No code review is needed to catch this error.

### Negative

- **Indirection overhead**: All communication goes through the EventBus, which adds boxing, dispatch, and handler invocation overhead. For latency-sensitive paths within the same layer, this overhead is measurable (approximately 2 microseconds per dispatch based on Phase 3 profiling).
- **Boilerplate**: Each cross-crate interaction requires: defining event types (struct + Event impl), defining handler types (struct + EventHandler impl), subscribing handlers during initialization, and publishing events. This is approximately 30-50 lines of boilerplate per event type.
- **Learning curve**: New contributors must understand the layered architecture, crate structure, EventBus pattern, and trait-based module boundaries before they can effectively contribute to any module.
- **Generics limitations**: Trait definitions at layer boundaries cannot always use generics due to trait object safety requirements for EventBus dispatch. Methods like `fn process<T: Into<Foo>>(&self, value: T)` are not possible in a dyn-compatible trait. Types must be concrete, boxed, or enum-dispatched.
- **Crate splitting overhead**: If modules within a layer are split into separate crates, every shared type must be in a common crate or duplicated. The default single-crate-per-layer approach avoids this.
- **Event type proliferation**: As the platform grows, the number of event types grows linearly with the number of module interactions. Event type organization and discoverability become documentation challenges.

## Compliance

1. **Build verification**: Running `cargo build --workspace` must succeed. If `core/` builds successfully but `runtime/` fails with a missing dependency on `core/`, the dependency direction is correct. Any workspace crate that depends on `core/` is in the correct direction.

2. **Dependency linting via script**: CI runs `ci/check-deps.sh` which parses `cargo metadata` and verifies:
   - `core/` has zero workspace crate dependencies.
   - `runtime/` only depends on `core/` (and Foundation).
   - No crate depends on a crate in a higher phase number.

3. **Code review rules**: Any `use` statement importing a crate from a higher layer into a lower-layer crate is automatically rejected. For example, `use runtime::scheduler::Scheduler` in `core/src/` is a violation. Reviewers check for EventBus usage instead of direct imports.

4. **EventBus audit**: A periodic automated audit (run weekly in CI) enumerates all cross-crate `use` statements and flags any that are not EventBus-related. The audit script uses `rust-analyzer` or equivalent to resolve imports.

5. **Cycle detection**: CI runs `ci/check-cycles.sh` which uses `cargo metadata --format-version 1 | jq` to extract the dependency graph and checks for cycles using a topological sort.

6. **Integration test policy**: Integration tests for cross-layer scenarios must use the EventBus. No integration test may call a module's function directly across a crate boundary.

7. **Module boundary markers**: Each crate's `lib.rs` must include a doc comment clearly stating which layer the crate belongs to and listing its allowed dependencies. This is checked during code review.

8. **CI workspace graph visualization**: CI generates a dependency graph of workspace crates using `cargo metadata | dot` and saves it as a CI artifact. A human reviews the graph periodically to ensure no unexpected dependencies have been introduced.

9. **Publish policy**: No workspace crate may be published to crates.io (they are internal to this project). The `[package] publish = false` field is set in every crate's `Cargo.toml`.

### Module Evolution

Modules within a layer can evolve through the following mechanisms:

1. **Additive changes**: New traits, new default implementations, new event types, and new handler registrations require standard code review but do not require an ADR.

2. **Breaking trait changes**: Changing an existing trait's method signatures requires a new ADR because it may affect all consumers of that trait across crate boundaries.

3. **Implementation replacement**: Replacing a default implementation (e.g., replacing `DefaultScheduler` with `EDFScheduler`) requires code review but does not require an ADR, as long as the trait interface is unchanged.

4. **Crate splitting**: Splitting a layer crate into multiple crates (e.g., splitting `runtime/` into `runtime-scheduler/` and `runtime-supervisor/`) requires a new ADR because it changes the crate dependency graph and may affect EventBus usage patterns.

5. **Layer addition**: Adding a new layer (e.g., `ai-engine/`) requires a new ADR because it changes the overall architecture and dependency rules.

This evolution policy balances stability (trait interfaces are stable) with flexibility (implementations can be swapped freely).

## Notes

- This ADR was refined during Phase 2 implementation when the exact boundary between Core and Runtime was clarified. Initially, the Core crate included event type definitions for all layers. This was moved to each module's own crate to reduce recompilation when event types change.
- The rule against direct cross-crate calls was relaxed for the Scheduler and Supervisor within the Runtime layer during Phase 3. These two subsystems are so tightly coupled (a scheduling decision triggers supervision, a failure triggers rescheduling) that the EventBus overhead was unacceptable. They live in the same crate and share internal types.
- Phase 8 (API Layer) may introduce a gRPC boundary that crosses process boundaries. This ADR applies within each process; cross-process communication will be addressed in a future ADR.
- The term "layer" in this ADR refers to logical dependency level, not network layering. Do not confuse with OSI model layers.

## References

- [ADR-0001: Project Vision and Scope](./0001-project-vision.md) — Establishes the phased roadmap and clean architecture principle.
- [ADR-0003: Rust as Implementation Language](./0003-rust.md) — Language choice enabling trait-based boundaries and crate isolation.
- [ADR-0004: Event-Driven Architecture via EventBus](./0004-event-driven.md) — The communication mechanism between layers.
- [ADR-0005: Runtime Platform Design](./0005-runtime-platform.md) — The Runtime layer crate structure and subsystem decomposition.
- [Clean Architecture by Robert C. Martin](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html)
- [Cargo Workspace Documentation](../../Cargo.toml)
- [CI Dependency Check Script](../../ci/check-deps.sh)
- [CI Cycle Detection Script](../../ci/check-cycles.sh)
