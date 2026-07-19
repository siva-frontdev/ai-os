# Design Principles

## Introduction

These twelve principles are the decision-making framework for AI-native OS. Every architectural choice, implementation detail, and code review is evaluated against these principles. When two principles conflict (and they will), the earlier-listed principle takes precedence. The principles are ordered by priority.

Principles are not suggestions. They are binding design constraints. Code that violates a principle should be rejected in review unless an explicit exception has been granted by the architecture team with documented rationale.

---

## Principle 1: Clean Architecture

### Statement

The system is organized into layers with strict dependency direction. Inner layers define interfaces; outer layers implement them. Dependencies always point inward.

### Explanation

Clean Architecture, as described by Robert C. Martin, separates software into concentric layers. The innermost layers contain business logic and are independent of frameworks, databases, and external concerns. Outer layers implement infrastructure details. The dependency inversion principle ensures that outer layers depend on inner abstractions, not the reverse.

In AI-native OS, this translates to a strict layered model: Core depends on nothing except the Rust standard library. Runtime depends on Core. System depends on Runtime. Each successive phase layer depends only on the layer directly beneath it.

### Rationale

- Isolates domain logic from infrastructure changes.
- Enables testing of core logic without external dependencies.
- Allows replacement of infrastructure components without affecting business rules.
- Makes the system comprehensible at every layer of abstraction.

### Example

The Core Platform defines a `HealthCheck` trait. The Runtime Platform implements `HealthCheck` as a service that dispatches health events on the EventBus. The System Platform uses `HealthCheck` through its trait interface, unaware of the EventBus implementation. If the EventBus implementation changes, the System Platform code does not change.

---

## Principle 2: Event-Driven Design

### Statement

All inter-component communication occurs through typed events dispatched on the EventBus. No component holds a direct reference to another component.

### Explanation

Components communicate exclusively by publishing and subscribing to events. The EventBus mediates all message passing, providing routing, filtering, transformation, and middleware capabilities. Components never import, call, or reference other components directly.

### Rationale

- Eliminates tight coupling between components.
- Enables complete observability at the bus level.
- Supports asynchronous, non-blocking communication patterns.
- Simplifies adding, removing, or replacing components.
- Provides a natural audit trail of all system activity.

### Example

When a Task completes, the Task Manager dispatches a `runtime.task.completed` event. The Session Manager (which subscribed to `runtime.task.*`) receives the event and advances the session state. If a new component needs to react to task completions in the future, it simply subscribes to the same event. No existing code changes.

---

## Principle 3: Fail Fast and Gracefully

### Statement

Detect and report failures at the earliest possible moment. When failure occurs, contain the damage, report the event, and recover autonomously when possible.

### Explanation

Fail Fast means performing validation and assertion checks as early as possible. If an invariant is violated, the component should report the failure immediately rather than allowing corrupted state to propagate. Fail Gracefully means that when failure does occur, the component emits a structured failure event, releases resources, and terminates cleanly.

The Supervisor monitors for failure events and applies configurable restart policies. The goal is not to prevent all failures (which is impossible) but to detect them early and recover from them automatically.

### Rationale

- Prevents cascading failures caused by corrupted state.
- Reduces debugging time by surfacing failures close to their root cause.
- Enables automated recovery through the Supervisor.
- Produces clean, actionable failure events for observability.

### Example

A scheduler receives a task with a malformed priority field. Rather than assigning a default priority silently (which might violate a policy), the scheduler fails fast by rejecting the task and dispatching a `runtime.scheduler.task_rejected` event with the validation details.

---

## Principle 4: Security by Default

### Statement

The default configuration is secure. Permissions are denied by default and must be explicitly granted. Security decisions are made at the architecture level, not retrofitted.

### Explanation

Security is not a feature to be added later. It is a property of the system's design. Every event dispatch is authenticated. Every service has an identity. Every operation is authorized. The permission model is pluggable but always present. No component may bypass the security layer.

Security contexts are propagated with every event, ensuring that the identity and permissions of the original requestor are preserved across asynchronous boundaries.

### Rationale

- Security retrofitted after design is fragile, incomplete, and expensive.
- Default-deny prevents accidental exposure of capabilities.
- Propagation of security context prevents privilege escalation through indirection.
- A pluggable permission model allows adaptation to different security requirements without changing the architecture.

### Example

A new service registers itself with the system. By default, it has no permissions. It cannot dispatch events, access resources, or communicate with other services until the policy engine grants specific capabilities. The act of registration itself is authorized.

---

## Principle 5: Observability First

### Statement

Every component must emit structured, machine-parseable observability data. Observability infrastructure is a first-class concern, not an afterthought.

### Explanation

Observability encompasses logs, metrics, traces, and events. Every component in AI-native OS must:
- Produce structured log entries via the Logger with consistent fields.
- Emit health status updates to the HealthMonitor.
- Propagate trace and span context through the Context Manager.
- Report key metrics (counts, latencies, error rates) to the metrics subsystem.

Observability data is itself observable. The monitoring infrastructure emits its own health and metrics.

### Rationale

- An intelligent system cannot manage itself if it cannot observe itself.
- Structured data enables automated analysis, anomaly detection, and learning.
- Distributed tracing is essential for debugging async, event-driven systems.
- Observability data feeds the intelligence layers (Brain, Perception) in later phases.

### Example

When the Permission Checker evaluates an access request, it logs the decision with the principal, resource, action, and result. It emits a `runtime.permission.evaluated` event containing the same data. It records the decision latency as a metric. It extends the current trace span with the decision metadata.

---

## Principle 6: Single Responsibility

### Statement

Each module, service, and component has exactly one reason to change. Responsibilities are not shared across boundaries.

### Explanation

Single Responsibility Principle (SRP) states that a module should be responsible to a single actor or stakeholder. In AI-native OS, this means that each service in the system performs one clearly defined function and delegates everything else to other services.

A service that "manages sessions and also handles logging" has two responsibilities. Session management and logging are separate concerns that should be separate services.

### Rationale

- Reduces the impact of changes: modifying one responsibility does not risk breaking another.
- Improves testability: each service can be tested in isolation.
- Simplifies reasoning: a service with one responsibility is easier to understand.
- Supports the EventBus communication model: small, focused services communicate through events.

### Example

The Supervisor is responsible solely for monitoring service health and applying restart policies. It does not schedule tasks (that is the Scheduler's responsibility), manage sessions (Session Manager), or allocate resources (Resource Manager). It subscribes to health events and dispatches supervisor actions.

---

## Principle 7: Explicit over Implicit

### Statement

Behavior, dependencies, configuration, and data flow are declared explicitly. Magic, inference, and hidden side effects are avoided.

### Explanation

Explicit code makes its intent clear. Dependencies are declared through dependency injection, not discovered at runtime. Configuration is merged from declared sources, not inferred from environment. Event subscriptions are registered at initialization, not discovered by reflection. Error handling is explicit through Result types, not panics.

Rust's type system is a powerful tool for making invariants explicit. The project leverages types to encode guarantees that would be comments or runtime checks in other languages.

### Rationale

- Code readability: explicit code is self-documenting.
- Maintainability: the impact of changes is apparent from the explicit declarations.
- Debugging: implicit behavior causes surprises that are difficult to diagnose.
- Tooling: explicit declarations enable static analysis that catches errors at compile time.

### Example

A service declares its event subscriptions in a `Subscriptions` struct returned from a `subscriptions()` method. The EventBus validates these subscriptions at service registration time. No subscription is registered through implicit convention, naming patterns, or runtime discovery.

---

## Principle 8: Async by Default

### Statement

All I/O, inter-service communication, and event processing is asynchronous. Blocking operations are isolated and explicitly marked.

### Explanation

AI-native OS runs on Tokio's async runtime. All operations that wait on I/O, timers, or events are async. Blocking operations (CPU-intensive computation, synchronous syscalls) are offloaded to dedicated blocking thread pools managed by Tokio.

The async boundary is explicit: async functions, await points, and blocking annotations are part of the codebase's discipline. The `Send` and `Sync` bounds required by Tokio tasks are verified at compile time.

### Rationale

- Maximizes throughput by minimizing idle threads.
- Reduces resource consumption compared to thread-per-connection models.
- Enables the event-driven architecture to scale efficiently.
- Integrates naturally with Tokio's work-stealing scheduler.

### Example

The Resource Manager queries system resource usage. The query is async, yielding to Tokio's scheduler while waiting for the kernel response. When a blocking operation is unavoidable (e.g., reading a large file synchronously for a policy evaluation), it is wrapped in `tokio::task::spawn_blocking` and explicitly documented.

---

## Principle 9: Immutable State Where Possible

### Statement

State is immutable by default. Mutation is explicit, isolated, and controlled through well-defined interfaces.

### Explanation

Shared mutable state is the source of the most difficult bugs in concurrent systems. AI-native OS minimizes shared mutable state by:
- Using immutable data structures for event payloads and configuration.
- Isolating mutable state within service boundaries.
- Communicating state changes through events (which are immutable after dispatch).
- Using interior mutability patterns (Mutex, RwLock) only when necessary and with explicit reasoning.

### Rationale

- Eliminates data races at the architecture level.
- Simplifies reasoning about concurrent code.
- Enables safe caching and sharing of immutable data.
- Makes replay and debugging feasible: immutable events preserve a complete history.

### Example

When a session state changes, the Session Manager does not mutate a shared session object. Instead, it dispatches a `runtime.session.state_changed` event containing the new state. The new state is an immutable value. Any component that needs the current state maintains its own view, updated by subscribing to state change events.

---

## Principle 10: Convention over Configuration

### Statement

Sensible defaults are provided for all configuration. Explicit configuration is required only when deviating from conventions.

### Explanation

Every configurable aspect of the system has a default value that works for the common case. Configuration files document the conventions they override. New developers can run the system with zero configuration and get a working environment. Production deployments override only the specific values that differ from defaults.

Conventions cover:
- Directory layout and file naming.
- Service naming and event type patterns.
- Logging format and levels.
- Port assignments and endpoint conventions.
- Configuration file locations and formats.

### Rationale

- Reduces the cognitive load of configuration.
- Accelerates onboarding for new contributors.
- Ensures consistency across deployments.
- Simplifies documentation: document the convention, not the exception.

### Example

A new service named `task-scheduler` in the runtime layer is automatically assigned the module ID `runtime.task_scheduler`. Its configuration file is expected at `/etc/ai-os/runtime/task_scheduler.toml`. Its log output follows the standard structured format. The developer needs to write configuration only for the values that differ from these conventions.

---

## Principle 11: Test-First

### Statement

Tests are written before or concurrently with production code. Every module has unit tests, integration tests, and property-based tests as appropriate.

### Explanation

Testing is not a phase that follows development. It is integral to the development process. New functionality includes tests at the time of implementation. Every bug fix includes a regression test. The test suite is run in CI and must pass before merge.

The testing strategy covers:
- **Unit tests**: Individual functions and methods, including error paths.
- **Integration tests**: Service interactions through the EventBus.
- **Property-based tests**: Invariants that must hold for all inputs.
- **Health check tests**: Services respond correctly to health probes.
- **Security tests**: Permission enforcement and access control.

### Rationale

- Catches regressions before they reach production.
- Provides living documentation of expected behavior.
- Enables confident refactoring.
- Supports the Fail Fast principle by catching bugs at the earliest moment.

### Example

Before implementing a new scheduling policy, the developer writes property-based tests specifying the invariants the scheduler must maintain (e.g., "no task is scheduled on a resource that has insufficient capacity," "every ready task is scheduled within its maximum latency"). Only then is the policy implemented.

---

## Principle 12: Documentation as Source of Truth

### Statement

Documentation is maintained as a first-class deliverable, kept current with the code, and treated with the same rigor as production code.

### Explanation

Documentation is not an afterthought or a nicety. It is the authoritative reference for the system's design, behavior, and usage. Code comments explain "why" decisions that are not obvious from the code itself. External documentation explains architecture, principles, and workflows.

Documentation is versioned alongside code and updated as part of the same pull request. A feature is not complete until its documentation is complete. Outdated documentation is treated as a bug.

### Rationale

- Documentation is the primary onboarding tool for new contributors.
- Accurate documentation reduces question-answer cycles in issues and discussions.
- Code alone cannot explain design rationale, trade-offs, or conventions.
- Outdated documentation is worse than no documentation: it actively misleads.

### Example

A pull request that adds a new event type to the Runtime Platform must include:
- Rustdoc comments on the event type and its fields.
- An update to the event type reference (if maintained externally).
- Updates to any architecture documentation describing the event flow.
- A glossary entry if new terminology is introduced.

---

## Principle Resolution

When principles conflict, resolve as follows:

1. **Clean Architecture** overrides all other principles. If another principle would violate the layered dependency rule, the other principle must find a different expression.
2. **Security by Default** overrides principles 5-12. Security constraints cannot be relaxed for observability, performance, or convenience.
3. **Fail Fast and Gracefully** overrides principles 6-12. It is better to fail explicitly than to produce incorrect results from degraded state.
4. **Event-Driven Design** overrides principles 7-12. The event-driven communication model is not negotiable for performance or simplicity reasons.
5. Remaining principles are resolved by architectural review, with the decision and rationale documented.

---

## Conclusion

These twelve principles define the character of AI-native OS. They are the answer to every design question: "What does our architecture say?" They are the criteria for every code review: "Does this change violate a principle?" They are the framework for every trade-off discussion: "Which principle do we prioritize?"

New contributors should read these principles before reading any code. The principles explain why the code is the way it is. They are the map to the architecture. The [architecture document](architecture.md) is the territory.
