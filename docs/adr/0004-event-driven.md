# ADR-0004: Event-Driven Architecture via EventBus

## Status

Accepted

## Date

2025-01-25

## Context

The AI-native OS platform requires a communication mechanism between modules that preserves the loose coupling mandated by the clean layered architecture ([ADR-0002](./0002-clean-architecture.md)). Direct function calls between modules create tight coupling, making it impossible to develop, test, and deploy modules independently.

### Problem

If Module A calls Module B's functions directly, the following problems emerge:

1. **Tight coupling**: Module A depends on Module B's concrete implementation. Changing B's implementation (e.g., switching a storage backend from SQLite to PostgreSQL) may require changes to A's code or at minimum a recompilation of A.

2. **Testing difficulty**: Testing Module A requires instantiating Module B, which may have complex dependencies on databases, GPU hardware, or external services. Tests become integration tests by necessity, even when only A's logic is under test.

3. **No audit trail**: Direct function calls leave no trace unless explicitly logged. Debugging cross-module interactions requires tracing through call stacks with a debugger. In production, this is often impossible.

4. **Blocking semantics**: Synchronous function calls block the caller. Async function calls require the caller to have access to the callee's async context. This creates implicit coupling of async runtimes and task models.

5. **Module discovery**: Modules must know each other's locations (crate paths, type names, function signatures). This prevents hot-swapping implementations and makes dynamic module loading (planned for Phase 9) impossible.

6. **No interception point**: There is no single point where cross-module communication can be observed, logged, throttled, or transformed. Adding observability requires modifying every call site.

### Requirements

The communication mechanism must satisfy:

| # | Requirement | Rationale |
|---|-------------|-----------|
| 1 | Loose coupling | Modules communicate through an intermediary, not directly. The publisher does not know the subscriber's identity. |
| 2 | Type safety | Events carry structured data of known types. Subscribers receive typed events, not raw bytes. Type mismatches are caught at compile time. |
| 3 | Async dispatch | Event handlers run asynchronously without blocking the publisher. The publisher should not await handler completion (fire-and-forget) or may optionally await a response (request-response). |
| 4 | Multiple subscribers | Multiple modules may handle the same event type. A single event publishing may trigger N handler invocations. |
| 5 | Fire-and-forget and request-response | Both one-way events and two-way request-response patterns must be supported. |
| 6 | Low overhead | Event dispatch should add minimal latency. Target: under 10 microseconds per dispatch hop in the in-process implementation. |
| 7 | Traceability | Each event should carry metadata (trace ID, span ID) for integration with the distributed tracing system. |
| 8 | Handler isolation | A failing handler should not affect other handlers for the same event. Errors are scoped to the handler. |

### Alternatives Considered

**Direct function calls**: Rejected. Violates all requirements. The tight coupling makes independent development and testing impossible.

**Message queue (RabbitMQ, NATS, Kafka)**: Rejected for in-process communication. Adds network serialization overhead (encode/decode for every dispatch), operational complexity (must run and maintain a message broker), and latency (network round-trip even for in-process communication). These may be used in Phase 8 for multi-process deployments where modules run in separate processes.

**Channel-based (Tokio mpsc/broadcast)**: Considered but rejected for general use. Channels require explicit wiring between publishers and subscribers (the publisher must hold a `Sender` handle, the subscriber must hold a `Receiver` handle), which re-introduces coupling. Channels are appropriate for specific point-to-point data streams within a single module (e.g., a metrics producer feeding into a metrics aggregator) but not for general module-to-module communication.

**Actor model (Actix)**: Rejected. Actix's actor framework is tightly coupled to the Actix runtime. If we later need to change runtimes or use actors across process boundaries, the entire actor infrastructure would need to be replaced. Actix also imposes an actor-per-type model that does not align with our trait-per-subsystem design.

**Tower middleware stack (Service, Layer)**: Considered for request-response patterns but deemed too HTTP-centric. Tower's `Service` trait is optimized for request-response with a single response type. Our communication patterns include events with zero or multiple subscribers, streaming responses, and fire-and-forget semantics.

**Observer pattern with trait objects**: Each publisher maintains a list of subscriber trait objects and iterates over them. Rejected because it requires the publisher to know about subscriber types at compile time or through dynamic registration, and it does not provide type-safe dispatch for different event types.

### EventBus Design

The EventBus is defined in the `core/` crate with the following core abstractions:

```rust
/// Core trait for all events that flow through the system.
pub trait Event: Send + 'static {
    /// A human-readable name for this event type (e.g., "agent.created").
    fn event_name(&self) -> &'static str;

    /// Unique identifier for this specific event instance.
    fn event_id(&self) -> EventId;

    /// Trace identifier for distributed tracing. Propagated through handler chains.
    fn trace_id(&self) -> TraceId;

    /// Optional parent span ID for tracing.
    fn parent_span_id(&self) -> Option<SpanId>;
}

/// EventBus trait -- the public interface for publishing and subscribing.
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish an event. All registered handlers for this event type
    /// are invoked asynchronously. Returns immediately after dispatch.
    async fn publish(&self, event: Box<dyn Event>);

    /// Subscribe a handler to a specific event type E.
    /// The handler is invoked for every event of type E published
    /// after subscription. Registration is idempotent.
    async fn subscribe<E, H>(&self, handler: Arc<H>)
    where
        E: Event,
        H: EventHandler<E> + Send + Sync + 'static;

    /// Publish a request event and await a response.
    /// This is a convenience wrapper around publish + subscribe.
    async fn request<E, R>(&self, event: E) -> Result<R, BusError>
    where
        E: Event + RequestEvent<R>,
        R: Send + 'static;

    /// Unsubscribe a previously registered handler.
    async fn unsubscribe<E>(&self, handler_id: HandlerId)
    where
        E: Event;

    /// Return the number of registered handlers for diagnostic purposes.
    fn handler_count(&self) -> usize;
}

/// Handler trait -- implemented by event consumers.
#[async_trait]
pub trait EventHandler<E: Event>: Send + Sync + 'static {
    /// Handle a single event. Return Ok(()) on success or HandlerError
    /// on failure. The EventBus logs the error and continues to other handlers.
    async fn handle(&self, event: E) -> Result<(), HandlerError>;
}
```

Key design points:

- **TypeId routing**: Events are dispatched based on `TypeId::of::<E>()`. This provides compile-time type safety without string-based topic matching. The routing table is a `DashMap<TypeId, Vec<HandlerEntry>>` for concurrent read access.

- **Boxed dispatch**: Events are boxed (`Box<dyn Event>`) at the publish boundary to enable heterogeneous storage in the bus's internal channel. The bus downcasts to the concrete type `E` when invoking handlers.

- **Trait object safety**: Both `EventBus` and `EventHandler` are fully `dyn`-compatible. This allows different EventBus implementations (in-memory, persisted, distributed) and different handler storage strategies.

- **Trace context**: Every event carries a `TraceId` and optional `SpanId`. The EventBus creates a tracing span around each handler invocation, linking child spans to the parent event's span.

- **Handler isolation**: Each handler invocation is wrapped in a `tokio::spawn` task with its own error handling. A panicking handler does not crash the publisher or other handlers.

### Communication Patterns

1. **Fire-and-forget (publish)**: The most common pattern. A module publishes an event and continues immediately. All subscribers handle the event concurrently. Example: `ResourceManager` publishes `ResourceExceeded`; `Scheduler` and `Supervisor` both handle it independently.

2. **Request-response (request)**: A module publishes a request event and awaits a response. The EventBus routes the request to a handler and returns the response. Example: `StateMachine` requests `PermissionChecker` to evaluate `ActionRequested` and awaits the boolean response.

3. **Scatter-gather**: A module publishes a request and collects responses from multiple handlers. Not directly supported in the initial EventBus; can be composed using request events with a subscription window.

4. **Event stream**: A module subscribes to a stream of events and processes them sequentially. The EventBus preserves order per subscriber (events of the same type are delivered in publish order to each subscriber).

### InMemoryEventBus Implementation

The default implementation (`InMemoryEventBus`) uses:

- A `DashMap<TypeId, Vec<HandlerEntry>>` for the subscriber registry. `HandlerEntry` holds the handler ID, a `Box<dyn Any>` for the type-erased handler, and a `Box<dyn Fn(...)>` for dispatch.
- For publish: the bus iterates the handler list for the event's TypeId, spawns a task for each handler, and returns.
- For subscribe: the bus inserts a `HandlerEntry` into the map for the given TypeId.
- Concurrency: `DashMap` provides lock-free concurrent access. Handler dispatch uses `tokio::spawn` for parallelism.

## Decision

We implement a centralized EventBus as the sole mechanism for inter-module communication within the platform. Specific commitments:

1. **EventBus in core**: The `EventBus` trait and `Event` trait are defined in the `core/` crate. The `InMemoryEventBus` implementation is provided in `core/src/eventbus.rs` as the default for single-process deployment and testing.

2. **All cross-crate communication goes through EventBus**: Direct function calls between modules in different crates are prohibited. This includes synchronous and async calls, whether through public functions, methods, or trait methods.

3. **Event types are defined in the originating crate**: Each module defines its own event types. The event type definitions live in the module's crate, not in core. This keeps `core/` stable and minimizes cascading recompilation when event schemas change.

4. **Request-response via two-phase events**: For request-response, the requesting module publishes a request event and subscribes to a response event. The response event carries a correlation ID matching the request. The `EventBus::request` convenience method wraps this pattern.

5. **Handler registration during module init**: Each module registers event handlers during initialization by calling `EventBus::subscribe`. Handler registration is idempotent. Module initialization order is managed by `Runtime::new()`.

6. **Tracing integration**: Every event dispatch creates a `tracing::Span` at the `debug` level. The trace ID from the event is used as the span's trace ID. Handler invocations create child spans.

7. **No topic-based routing**: All routing is by Rust type (`TypeId`). No topic hierarchies, wildcard subscriptions, or content-based routing. This simplifies the implementation to O(1) dispatch and prevents routing errors due to topic string mismatches.

8. **Events are lossless within a process**: The `InMemoryEventBus` retains events in a bounded channel until all handlers complete. If a handler panics, the event is not retried. Future distributed EventBus implementations may offer at-least-once or exactly-once delivery semantics.

## Consequences

### Positive

- **Loose coupling**: Modules know only the event types they handle, not the modules that produce them. The publisher does not know the subscriber count, identity, or implementation detail. Swapping implementations requires no changes to consumers.
- **Type safety**: Events are concrete Rust types implementing the `Event` trait. A subscriber for type `AgentCreated` receives `AgentCreated` structs, not raw JSON or protobuf. The compiler catches type mismatches.
- **Traceability**: Every event carries trace context. The tracing integration provides end-to-end observability of cross-module interaction flows without additional instrumentation.
- **Testability**: Tests can subscribe to events, assert that specific events were published, and inject fake events to drive module behavior. A module can be tested in isolation with a mock `EventBus`.
- **Extensibility**: New modules can subscribe to existing event types without modifying the producers. This enables third-party module loading in Phase 9 without requiring changes to first-party modules.
- **Handler isolation**: A bug in one handler (panic, infinite loop, deadlock) does not affect other handlers for the same event or the publisher. The EventBus wraps handlers in error-bound tasks.

### Negative

- **Indirection overhead**: Boxing events and dispatching through type-erased handler tables adds measurable overhead. Phase 3 profiling showed approximately 2 microseconds per dispatch for the `InMemoryEventBus`. This is acceptable for module boundaries but would be prohibitive for intra-module calls.
- **Event ordering**: The EventBus does not guarantee ordering between different event types. If module A publishes event type X and then event type Y, subscribers to type Y may receive their event before subscribers to type X receive theirs. Ordering is preserved per (subscriber, event type) pair.
- **No backpressure**: The current `InMemoryEventBus` does not implement backpressure. A fast publisher can enqueue events faster than a slow subscriber can process them, leading to unbounded memory growth in the internal channel. Backpressure will be addressed in a future ADR (planned for Phase 4).
- **Debugging complexity**: Following an event flow through multiple modules requires tracing tools. Without tracing enabled, the flow is invisible in the code — there are no direct call paths to follow. Developers must understand the EventBus subscription graph.
- **Boilerplate**: Each cross-crate event interaction requires: (a) defining an event struct with `Event` impl, (b) defining a handler struct with `EventHandler<E>` impl, (c) registering the handler during module init. This is approximately 30-50 lines per interaction.
- **No compile-time handler verification**: There is no guarantee at compile time that every published event type has at least one subscriber. Orphaned events (published but never handled) are detected only through runtime metrics or integration tests.

## Compliance

1. **Code review rule**: Any `pub use` or `pub fn` that exposes a function from one module crate to be called directly by another module crate must be flagged. The correct pattern is to define an event type and subscribe to it via the EventBus.

2. **CI lint for direct calls**: A custom CI script (`ci/check-eventbus.sh`) scans for direct cross-crate calls by examining `use` statements between workspace crates. It uses `cargo metadata` to determine crate dependency paths and `rg` to find `use` patterns that bypass the EventBus. The script excludes `core/` which all crates depend on legitimately.

3. **Benchmark gate**: A microbenchmark in `core/benches/eventbus.rs` measures round-trip dispatch latency. If the p99 latency exceeds 15 microseconds, the CI gate rejects the change. This prevents performance regressions in the EventBus implementation.

4. **Tracing assertion in tests**: Integration tests verify that every event publish creates a tracing span. Tests use `tracing-subscriber` with a `TestWriter` to capture spans and assert that each publish creates the expected span hierarchy.

5. **Ownership and code review**: The `core/src/eventbus.rs` module is owned by the platform architecture team. Any change to the EventBus trait or `InMemoryEventBus` implementation requires review and approval by at least two maintainers.

6. **Event documentation**: Every event type must be documented with: its purpose, which module publishes it, which modules subscribe to it, and the expected handling semantics. This documentation lives in the event type's doc comment and is checked during code review.

7. **Subscription coverage in tests**: Each module's test suite must include a test that verifies all expected event subscriptions are registered. This prevents silent changes to the subscription graph.

## Notes

- The `InMemoryEventBus` implementation uses a `DashMap<TypeId, Vec<Box<dyn Any>>>` for subscriber storage. `DashMap` was chosen over `RwLock<HashMap>` because the read-heavy workload (publish dispatches far more often than subscribe/unsubscribe) benefits from lock-free reads.
- During Phase 3 development, the event dispatch hot path was profiled with `perf` and `flamegraph`. The boxing overhead was measured at approximately 2 microseconds per dispatch, well within the 10-microsecond target. The TypeId lookup was not measurable (sub-nanosecond).
- A future ADR will address distributed EventBus for multi-process deployments (Phase 8). The distributed bus will implement the same `EventBus` trait, providing a seamless migration path.
- The backpressure problem will be addressed in ADR-NNNN (planned for Phase 4). Two approaches are under investigation: (a) bounded channels with blocking publish when the channel is full, and (b) a sliding window with demand signaling.
- Module panic isolation via `tokio::spawn` was added in Phase 3 after a bug caused subscriber A's panic to crash subscriber B. The fix isolates each handler in its own task.

## References

- [ADR-0001: Project Vision and Scope](./0001-project-vision.md) — Establishes the event-driven principle in architectural principles.
- [ADR-0002: Clean Architecture with Layered Modules](./0002-clean-architecture.md) — Crate isolation requires the EventBus for cross-crate communication.
- [ADR-0003: Rust as Implementation Language](./0003-rust.md) — Async traits, Send+Sync guarantees, and type system enabling typed EventBus dispatch.
- [ADR-0005: Runtime Platform Design](./0005-runtime-platform.md) — Runtime subsystems communicate through EventBus subscriptions.
- [EventBus Source: core/src/eventbus.rs](../../core/src/eventbus.rs)
- [EventBus Benchmark: core/benches/eventbus.rs](../../core/benches/eventbus.rs)
- [EventBus Integration Test: tests/eventbus_test.rs](../../tests/eventbus_test.rs)
- [tracing crate documentation](https://docs.rs/tracing)
