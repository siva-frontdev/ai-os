# Architecture Overview

## Purpose

AI-native OS is an async-first, event-driven operating platform built on Arch Linux and written in Rust. Its architecture follows a strict layered model where each layer depends only on the layer directly beneath it. Inter-module communication flows exclusively through a centralized EventBus, ensuring loose coupling, complete observability, and natural points for security enforcement.

This document provides the high-level architectural context for the entire platform. It describes the layered structure, communication patterns, startup and shutdown sequences, and the key architectural decisions that govern all implementation.

---

## Responsibilities

The architecture overview document covers the following concerns:

- Define the system boundary and context, identifying external actors and their interfaces to the platform.
- Document the layered architecture, including the dependency rule that governs all inter-layer relationships.
- Describe the three communication patterns (event notification, request-response, stream subscription) that modules use.
- Trace the complete lifecycle of an event from construction through dispatch to handler execution.
- Specify the startup and shutdown sequences that every platform component follows.
- Catalog the module dependency graph, showing which crates depend on which.
- Explain the threading model that underpins all async execution.
- Document cross-cutting concerns: lifecycle, error handling, observability, and security.
- Record key architectural decisions with links to Architecture Decision Records.

The overview does not describe implementation details of individual subsystems. Those are covered in the [Core](core.md), [Runtime](runtime.md), and [System](system.md) deep-dive documents.

---

## Public Interfaces

At the highest level, the platform exposes these public interfaces:

| Interface | Provider | Description |
|---|---|---|
| `EventBus` | Core | Publish and subscribe to typed events (pub/sub) |
| `Service` | Core | Lifecycle hooks: `name()`, `start()`, `stop()` |
| `LifecycleManager` | Core | Register services, start all, stop all, query state |
| `Logger` | Core | Structured logging with levels, targets, and fields |
| `HealthMonitor` | Core | Register health checks, run probes, query latest status |
| `Container` | Core | String-keyed dependency injection registry |
| `Scheduler` | Runtime | Priority-ordered task queue operations |
| `Supervisor` | Runtime | Task supervision with configurable restart policies |
| `SessionManager` | Runtime | Session CRUD lifecycle management |
| `TaskManager` | Runtime | Task creation, state transitions, session-scoped queries |
| `ContextManager` | Runtime | Trace/span context propagation |
| `RuntimeStateMachine` | Runtime | Runtime phase state machine |
| `ResourceManager` | Runtime | Per-task resource usage tracking and limits |
| `PermissionChecker` | Runtime | Role-based authorization checks |

All subsystem traits are designed to be `dyn`-compatible so they can be stored behind `Arc<dyn Trait>` and shared across the platform.

---

## Events Published

The platform-level events (defined by Core) are:

| Event | Type String | Publisher | Trigger |
|---|---|---|---|
| `StartedEvent` | `platform.started` | Core `Application` | All services started successfully |
| `ShutdownEvent` | `platform.shutdown` | Core `Application` | Platform begins graceful shutdown |

All other events are defined by the [Runtime](runtime.md) and [System](system.md) layers.

---

## Events Consumed

Core defines no event subscriptions. It provides the infrastructure for upper layers to publish and subscribe. Runtime and System layers subscribe to events as documented in their respective deep-dive documents.

---

## Future Extensions

- **Cross-Process EventBus**: Extend the in-process EventBus to support IPC transport, enabling multi-process deployments where modules run in separate OS processes.
- **Event Replay**: Add an event log that captures all events for debugging, analysis, and training the intelligence layers.
- **Dynamic Service Registration**: Allow services to register and unregister at runtime (currently registration is static at startup).
- **Multi-Tenant Scheduling**: Extend the Scheduler with tenant isolation and fairness guarantees.
- **Pluggable Permission Backends**: Support LDAP, OAuth, or custom permission providers behind the `PermissionChecker` interface.

---

## System Context

```
+-----------------------------------------------------------------------+
|                        AI-native OS Platform                           |
|                                                                        |
|  +------------------+  +------------------+  +----------------------+  |
|  | Service Consumers|  |   Admin/CLI     |  | External Integrations|  |
|  | (agents, apps)   |  |   (humans)      |  | (monitoring, CI)     |  |
|  +--------+---------+  +--------+---------+  +----------+-----------+  |
|           |                     |                        |              |
|           +---------------------+------------------------+              |
|                                 |                                      |
|                     +-----------+-----------+                          |
|                     |       EventBus       |                           |
|                     |  (Central Bus —      |                           |
|                     |   all communication) |                           |
|                     +---+-------+----+-----+                           |
|                         |       |    |                                 |
|              +----------+  +----+--+ +-+--------+                      |
|              | Core       | Runtime| | System   |                      |
|              | Platform   | Platform| | Platform |                      |
|              | (Phase 2)  |(Phase3)| |(Phase 4) |                      |
|              +------------+--------+-+----------+                      |
|              | EventBus   | Sched  | | Proc Mgr |                      |
|              | Lifecycle  | Superv | | FS Prov  |                      |
|              | Logger     | Sess   | | Net Mgr  |                      |
|              | Health     | Task   | | Svc Mgr  |                      |
|              | Container  | Ctx    | | EventCol |                      |
|              | Registry   | State  | | Resource |                      |
|              | Config     | Res    | +----------+                      |
|              |            | Perm   |                                    |
|              +------------+--------+                                    |
|                                                                         |
|  +------------------------------------------------------------------+  |
|  |                   Arch Linux Host OS                              |  |
|  |  (systemd, dbus, cgroups, namespaces, filesystem, networking)     |  |
|  +------------------------------------------------------------------+  |
+-----------------------------------------------------------------------+
```

All components reside in a single process space, with the EventBus as the sole communication channel. External consumers (agents, CLI tools, monitoring systems) interact with the platform through its public API surface, which is itself a set of EventBus-backed services.

---

## Layered Architecture

The platform is organized into nine development phases. The critical architectural boundary is the **Operating System Abstraction Layer (OSAL)** — it insulates all higher layers from Linux-specific details.

```
Platform Manifest (platform.toml)
    Boot process reads this file first. Determines which modules to load.
        |
        v
Layer 7: Intelligence Integration (planned)
        |
        v
Layer 6: Execution Platform (planned)
        |
        v
Layer 5: Brain Platform (planned)
        |
        v
Layer 4: Perception Platform (planned)
        |
        v
Layer 3: Memory Platform (planned)
        |
        v
Layer 2: Runtime Platform [Completed]
    Scheduler, Supervisor, Session, Task, Context, State, Resource, Permission
        |
        v
Layer 1: Core Platform [Completed]
    EventBus, Service, Logger, HealthMonitor, Container, Registry, Config
        |
        v
Layer 0: Operating System Abstraction Layer — OSAL [In Progress]
    SystemResource, Process, FileSystem, Network, EventCollector, ServiceManager
    ═══════════════════════════════════════════════════════════════
    Linux Kernel / Hardware (external)
```

The horizontal line below OSAL is the **platform boundary**. Everything above OSAL is OS-independent. All Linux-specific logic (syscalls, procfs, dbus, inotify, systemd, rtnetlink) lives behind OSAL trait interfaces. This allows future ports to non-Linux hosts without modifying any layer above OSAL.

### Dependency Rule

Source code dependencies must point downward. Code in the Runtime Platform may import Core Platform types. Code in either Core or Runtime may import OSAL types. The reverse is never permitted — OSAL never imports Core, Runtime, or any higher layer. This rule is enforced by Cargo workspace dependency configuration and verified in CI.

---

## Module Interaction Patterns

Modules communicate exclusively through the EventBus using three patterns:

### 1. Event Notification (Fire-and-Forget)

A module dispatches an event and has no expectation of a response. This is the most common pattern.

```
Publisher            EventBus            Subscriber
    |                   |                   |
    |-- dispatch() ---->|                   |
    |                   |-- handle() ------->|
    |                   |                   |
```

Example: `TaskManager` dispatches `runtime.task_completed`; `SessionManager` consumes it to advance session state.

### 2. Request-Response (Correlation ID)

A module dispatches a request and awaits a response correlated by ID. A oneshot channel is registered with a correlation map.

```
Requester            EventBus            Responder
    |                   |                   |
    |-- request(req) -->|                   |
    | (registers        |-- request(req) -->|
    |  oneshot)         |                   |
    |                   |<-- response(rsp)--|
    |<-- response(rsp)--|                   |
    | (completes        |                   |
    |  oneshot)         |                   |
```

### 3. Stream Subscription

A module subscribes to a stream of related events. The EventBus delivers events as they arrive. Used for log streaming, health monitoring, and telemetry.

---

## Event Flow Description

A complete event lifecycle proceeds through these stages:

1. **Construction**: A component creates an event struct implementing the `Event` trait. The event carries its trace context, source module ID, type string, and serialized payload.

2. **Dispatch**: The component calls `bus.publish_event(&event)` on the `EventBus` trait. The type-erased `InMemoryEventBus` resolves the event's `TypeId` at runtime.

3. **Handler Resolution**: The bus looks up all `SubscriberEntry` records registered for the event's `TypeId`. Handlers are collected under a read lock.

4. **Execution**: Each handler is invoked sequentially via `handler.handle(event)`. Handlers run asynchronously on the Tokio runtime. If any handler returns an error, it is propagated as `CoreError::HandlerFailed`.

5. **Downstream Events**: Handlers may dispatch new events in response, continuing the chain.

### Event Structure

Every event carries:

- `id`: UUID v7 (time-sortable)
- `trace_id` / `span_id`: distributed tracing identifiers
- `source`: module identifier
- `event_type`: namespaced string (e.g. `"runtime.task_completed"`)
- `payload`: serialized via Serde
- `timestamp`: time of dispatch
- `priority`: Critical, High, Normal, Low
- `security_context`: identity and permissions of the originator

---

## Startup Sequence

```
PlatformBuilder::build()
    |
    +-> LayeredConfigProvider (defaults + file + env)
    |
    +-> ConsoleSink / TracingBridge
    |
    +-> DefaultLogger (root target "ai-os")
    |
    +-> InMemoryEventBus
    |
    +-> DefaultLifecycleManager
    |
    +-> DefaultServiceRegistry
    |
    +-> DefaultHealthMonitor
    |
    +-> Application { event_bus, lifecycle, registry, health, config, logger }

Application::run()
    |
    +-> LifecycleManager::start_all()
    |       |
    |       +-> For each registered service (in order):
    |       |       set_state(Initializing)
    |       |       service.start().await
    |       |       set_state(Running)
    |       |
    |       +-> (failure sets state to Failed, returns error)
    |
    +-> set running = true
    |
    +-> publish_event(StartedEvent) ("platform.started")

Runtime (registered as a Service on the LifecycleManager):
    Runtime::start()
        |
        +-> state.transition(Created -> Initializing)
        +-> publish_event(RuntimePhaseChanged)
        +-> state.transition(Initializing -> Running)
        +-> publish_event(RuntimePhaseChanged)
```

### Service Start Order

Services are started in the order they were registered with the `LifecycleManager`. Dependencies must be registered before dependents. This is enforced by convention and documented in each service's registration code.

---

## Shutdown Sequence

```
Application::shutdown()
    |
    +-> set running = false
    |
    +-> LifecycleManager::stop_all()
    |       |
    |       +-> Reverse the service list
    |       |
    |       +-> For each service (in reverse order):
    |               set_state(Stopping)
    |               service.stop().await
    |               set_state(Stopped)
    |
    +-> (failure sets state to Failed, returns error)

Runtime::stop()
    |
    +-> state.transition(Running -> Draining)
    +-> publish_event(RuntimePhaseChanged)
    +-> state.transition(Draining -> Stopped)
    +-> publish_event(RuntimePhaseChanged)
```

Reverse-order shutdown ensures that dependents stop before their dependencies, preventing dangling references or use-after-free patterns.

---

## Module Dependency Graph

```
ai-os
  |
  +-- core/                      [Layer 1, Phase 2]
  |     +-- events/              EventBus trait + InMemoryEventBus
  |     +-- lifecycle/           Service trait + DefaultLifecycleManager
  |     +-- logging/             Logger trait + DefaultLogger + sinks
  |     +-- health/              HealthMonitor trait + DefaultHealthMonitor
  |     +-- container/           Container trait + InMemoryContainer
  |     +-- registry/            ServiceRegistry trait + DefaultServiceRegistry
  |     +-- config/              ConfigProvider + layered merge
  |     +-- bootstrap/           PlatformBuilder
  |     +-- application/         Application facade
  |     +-- error/               CoreError enum
  |     +-- utils/               ID generation, timestamps
  |
  +-- runtime/                   [Layer 2, Phase 3]
  |     +-- scheduler/           PriorityScheduler (BinaryHeap)
  |     +-- supervisor/          DefaultSupervisor (restart policies)
  |     +-- session/             DefaultSessionManager
  |     +-- task/                DefaultTaskManager (state machine)
  |     +-- context/             DefaultContextManager (task_local)
  |     +-- state/               DefaultRuntimeState (phase machine)
  |     +-- resource/            DefaultResourceManager
  |     +-- permission/          DefaultPermissionChecker (role-based)
  |     +-- runtime/             Runtime facade
  |
  +-- system/                    [Layer 3, Phase 4 -- In Progress]
        +-- .gitkeep             (subsystems planned:
                                   SystemResourceManager,
                                   ProcessManager,
                                   FileSystemProvider,
                                   NetworkManager,
                                   SystemEventCollector,
                                   ServiceManager)
```

---

## How Modules Communicate

All inter-module communication flows through the `EventBus` trait defined in `core::events`. The `InMemoryEventBus` is the sole production implementation.

### Subscription Registration

During service initialization (`Service::start()`), each service registers event handlers via `typed_subscribe` or `subscribe_erased`. Handlers are stored in a `HashMap<TypeId, Vec<SubscriberEntry>>` behind an `RwLock`.

### Dispatch Mechanics

When `publish_event(event)` is called:

1. The event's `TypeId` is extracted via `(&event as &dyn Any).type_id()`.
2. The subscriber map is read-locked and handlers for that `TypeId` are cloned into a local `Vec`.
3. Each handler is awaited in sequence. Errors are collected into `CoreError::HandlerFailed`.
4. The read lock is released before any handler executes, preventing deadlock.

### Type Safety

The `typed_subscribe` and `typed_publish` helper functions provide compile-time type safety for callers who want it. Internally, `HandlerAdapter` bridges typed handlers to the type-erased `ErasedEventHandler` trait that the bus requires.

### Filtering Model

The current `InMemoryEventBus` dispatches by exact `TypeId` match. There is no wildcard or prefix matching at the bus level. Modules that need pattern-based dispatch implement their own filtering on top of subscription.

---

## Thread Model

The platform runs on Tokio's multi-threaded runtime. Each Tokio task is an independent unit of execution.

| Subsystem | Threading Model |
|---|---|
| EventBus dispatch | Runs on the caller's Tokio task; handlers are awaited inline |
| LifecycleManager start/stop | Runs on the caller's Tokio task |
| Logger | Synchronous writes to sinks; `TracingBridge` delegates to tracing's async subscriber |
| HealthMonitor periodic checks | Spawned as a dedicated Tokio task per `start_periodic` call |
| Scheduler enqueue/dequeue | RwLock-protected; callers may be on any task |
| Supervisor | HashMap behind RwLock; callers on any task |
| SessionManager | HashMap behind RwLock; callers on any task |
| TaskManager | HashMap behind RwLock; callers on any task |
| ContextManager | `tokio::task_local!` for context propagation; no locks |
| StateMachine | RwLock-protected single phase value |
| ResourceManager | HashMap behind RwLock; callers on any task |
| PermissionChecker | Stateless; no locks |

Shared state is protected by `RwLock` or `Mutex` from `std::sync`. Tokio's `RwLock` is used in `Runtime` for the composite struct; all other locks use `std::sync::RwLock` for short critical sections.

---

## Lifecycle

Every component follows the `Service` trait lifecycle:

```
       +-----------+
       |  Created  |     (after construction)
       +-----+-----+
             |
             v
       +-----------+
       |Initializing|    (service.start() called)
       +-----+-----+
             |
       +-----v-----+
  +---->|  Running  |<----+
  |     +-----+-----+     |
  |           |            |
  |           v            |
  |     +-----------+     |
  +-----|  Stopping |     |   (resume from Draining)
  |     +-----+-----+     |
  |           |            |
  |           v            |
  |     +-----------+     |
  |     |  Stopped  |-----+
  |     +-----------+
  |
  +---> Failed (on error)
```

The `ServiceState` enum encodes these as discriminants: `Created = 0`, `Initializing = 1`, `Running = 2`, `Stopping = 3`, `Stopped = 4`, `Failed = 5`. The atomic `u8` field in `ManagedService` enables lock-free reads.

---

## Error Handling

All platform errors use a unified error enum (`CoreError` for core, `RuntimeError` for runtime). Each subsystem defines its own error variants within its layer's error enum.

| Layer | Error Type | Variant Count | Key Variants |
|---|---|---|---|
| Core | `CoreError` | 12 | `LockPoisoned`, `HandlerFailed`, `StartFailed`, `StopFailed` |
| Runtime | `RuntimeError` | 10 | `Scheduler`, `Supervisor`, `Session`, `Task`, `State`, `Resource`, `PermissionDenied` |
| System | `SystemError` (planned) | ~15 | `ResourceCollectionFailed`, `ProcessSpawnFailed`, `DbusConnectionFailed` |

Key principles:

- **Fail Fast**: Invariant violations are detected at the earliest point. A duplicate service registration is rejected immediately, not silently overwritten.
- **Graceful Reporting**: Failures are reported via events on the EventBus. The Supervisor subscribes to failure events and applies restart policies.
- **Lock Poisoning**: If an `RwLock` or `Mutex` is poisoned, `CoreError::LockPoisoned` or the corresponding `RuntimeError` variant is returned. No panic is propagated to the caller.
- **Handler Errors**: If an event handler fails, the error is captured in `CoreError::HandlerFailed` with the event type and detail message, but other handlers for the same event continue execution.

---

## Key Architectural Decisions

1. **Clean Layering with Strict Dependency Direction** -- Each phase layer depends only on the layer below. This guarantees that the system remains comprehensible as it grows across nine phases. See [ADR-0001](../adr/0001-layered-architecture.md) (planned).

2. **EventBus as the Sole Communication Channel** -- No module holds a direct reference to another. All inter-module communication is mediated by typed events. This enables loose coupling, complete observability, and centralized security enforcement. See [ADR-0002](../adr/0002-eventbus-communication.md) (planned).

3. **Tokio as the Async Runtime** -- Tokio's work-stealing scheduler, ecosystem compatibility, and production maturity make it the async runtime for the platform. See [ADR-0003](../adr/0003-tokio-runtime.md) (planned).

4. **In-Memory EventBus with Type-Erased Dispatch** -- The `InMemoryEventBus` uses `TypeId`-based dispatch for performance, with typed helpers for compile-time safety. This avoids the overhead of runtime serialization for in-process events. See [ADR-0004](../adr/0004-inmemory-eventbus.md) (planned).

5. **String-Keyed DI Container** -- Unlike a `TypeId`-based approach, string keys make the `Container` trait dyn-compatible and allow cross-language or runtime-defined service names. See [ADR-0005](../adr/0005-string-keyed-container.md) (planned).

6. **Reverse-Order Shutdown** -- Services stop in reverse registration order, ensuring dependents stop before their dependencies. This prevents dangling references during teardown. See [ADR-0006](../adr/0006-reverse-shutdown.md) (planned).

7. **BinaryHeap Priority Scheduler** -- The scheduler uses `std::collections::BinaryHeap` with a custom `Ord` implementation that orders by priority (ascending) then creation time (FIFO tiebreaker). This provides O(log n) enqueue and O(1) peek. See [ADR-0007](../adr/0007-priority-scheduler.md) (planned).

8. **tokio::task_local! for Context Propagation** -- Runtime context (trace/span IDs) is stored in `tokio::task_local!` rather than passed explicitly through every function signature. This preserves ergonomics while maintaining trace continuity across async boundaries. See [ADR-0008](../adr/0008-task-local-context.md) (planned).

---

## References

- [Core Platform Deep Dive](core.md)
- [Runtime Platform Deep Dive](runtime.md)
- [OSAL Deep Dive](system.md)
- [Design Principles](../principles.md)
- [Glossary](../glossary.md)
- [Roadmap](../roadmap.md)
