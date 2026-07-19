# System Architecture

## Overview

AI-native OS is organized as a clean layered architecture with strictly enforced dependency direction. Each layer depends only on the layer directly beneath it, and no layer depends on layers above. This constraint ensures that the system remains comprehensible, testable, and maintainable as it grows across nine phases of development.

The architecture is event-driven, async-first, and modular. All inter-component communication flows through a centralized EventBus, which decouples producers from consumers and provides a natural point for observability, tracing, and access control.

---

## Layered Architecture

### Layer Diagram

```
+-------------------------------------------------------------------+
|                     Intelligence Integration                       |
|  (Phase 9)                                                        |
+-------------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------------+
|                        Execution Platform                          |
|  (Phase 8)                                                        |
+-------------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------------+
|                        Perception Platform                         |
|  (Phase 7)                                                        |
+-------------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------------+
|                          Brain Platform                            |
|  (Phase 6)                                                        |
+-------------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------------+
|                         Memory Platform                            |
|  (Phase 5)                                                        |
+-------------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------------+
|                        System Platform                             |
|  (Phase 4 - In Progress)                                          |
|  Daemon Manager | Policy Engine | Capability Discovery | Config   |
+-------------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------------+
|                       Runtime Platform                             |
|  (Phase 3 - Completed)                                            |
|  Scheduler | Supervisor | Session Mgr | Task Mgr | Context Mgr    |
|  State Machine | Resource Mgr | Permission Checker                |
+-------------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------------+
|                         Core Platform                              |
|  (Phase 2 - Completed)                                            |
|  EventBus | Service Lifecycle | Logger | Health Monitor | Container|
+-------------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------------+
|                       Dev Environment                              |
|  (Phase 1 - Completed)                                            |
|  Toolchain | Rust Build System | CI/CD | Dev Container             |
+-------------------------------------------------------------------+
```

### Dependency Rule

Source code dependencies (imports, trait bounds, type references) must point **downward**. Code in the Runtime Platform may depend on Core Platform types and traits. Code in the Brain Platform may depend on Memory Platform, which depends on System Platform, and so on. The reverse is never permitted.

This rule is enforced by Cargo workspace dependency configuration and checked in CI.

---

## Module Dependency Graph (Text)

```
ai-os
  |
  +-- core/
  |     +-- eventbus/          -- EventBus, Event, Handler, Subscription
  |     +-- lifecycle/         -- Service lifecycle (Init, Running, Stopping, Stopped)
  |     +-- logger/            -- Structured logging with levels and spans
  |     +-- health/            -- HealthMonitor, HealthCheck, HealthStatus
  |     +-- container/         -- Container management (Docker/Podman integration)
  |
  +-- runtime/
  |     +-- scheduler/         -- Task scheduling and dispatch
  |     +-- supervisor/        -- Service supervision and restart policies
  |     +-- session/           -- Session management and lifecycle
  |     +-- task/              -- Task management, queues, workers
  |     +-- context/           -- Distributed context (trace/span propagation)
  |     +-- statemachine/      -- State machine engine for workflows
  |     +-- resource/          -- Resource management (CPU, memory, I/O)
  |     +-- permission/        -- Permission checking and access control
  |
  +-- system/
  |     +-- daemon/            -- Daemon management (start, stop, monitor)
  |     +-- policy/            -- Policy engine (rule evaluation, enforcement)
  |     +-- capability/        -- Capability discovery and registration
  |     +-- config/            -- Configuration management (sources, merging)
  |     +-- plugin/            -- Plugin system (loading, isolation, lifecycle)
  |
  +-- memory/
  |     (planned)              -- Persistent memory stores, pattern storage
  |
  +-- brain/
  |     (planned)              -- Reasoning, decision-making, state modeling
  |
  +-- perception/
  |     (planned)              -- Sensor integration, signal processing
  |
  +-- execution/
  |     (planned)              -- Action planning, multi-agent coordination
  |
  +-- integration/
        (planned)              -- Unified intelligence layer
```

---

## Event-Driven Design

### The EventBus

The EventBus is the central nervous system of AI-native OS. All inter-module communication happens through typed events dispatched on the bus. No module holds a direct reference to another module. This achieves:

- **Loose coupling**: Modules can be added, removed, or replaced without affecting peers.
- **Observability**: Every event can be logged, traced, and audited at the bus level.
- **Replay**: The event stream can be captured and replayed for debugging, analysis, or training.
- **Security**: Access control policies can be enforced at the bus level on every event.
- **Async by nature**: Event dispatch integrates naturally with Tokio's async runtime.

### Event Structure

Every event in the system carries:

```
Event {
  id: EventId (UUID v7, time-sortable)
  trace_id: TraceId
  span_id: SpanId
  source: ModuleId
  type: EventType (namespaced, e.g., "runtime.task.completed")
  payload: Vec<u8> (serialized via Serde)
  timestamp: SystemTime
  priority: EventPriority (Critical, High, Normal, Low)
  security_context: SecurityContext
}
```

### Event Flow

1. A component constructs an event with its current trace context.
2. The component dispatches the event to the EventBus via `bus.dispatch(event)`.
3. The EventBus applies middleware (logging, tracing, permission checking, metrics).
4. The EventBus routes the event to all subscribers matching the event type.
5. Subscribers process the event asynchronously via Tokio tasks.
6. Subscribers may dispatch new events in response, continuing the chain.

### Subscription Model

Modules subscribe to event types using pattern matching. Subscription supports:

- Exact type matching: `"runtime.task.completed"`
- Wildcard prefix matching: `"runtime.task.*"`
- Multiple type matching: `["runtime.task.completed", "runtime.task.failed"]`

Subscriptions are registered at service initialization and are part of the module's declared interface.

---

## Async Runtime: Tokio

AI-native OS uses Tokio as its asynchronous runtime. Tokio was chosen over alternative async runtimes for the following reasons:

1. **Production maturity**: Tokio is the most widely deployed async runtime in the Rust ecosystem, with extensive testing and optimization.
2. **Work-stealing scheduler**: Tokio's multi-threaded scheduler provides the performance characteristics needed for a system platform.
3. **Ecosystem compatibility**: Key dependencies (hyper, tonic, tower, axum) are built on Tokio.
4. **Instrumentation**: Tokio provides tracing integration for observability.
5. **Resource management**: Tokio's task budgeting and cooperative scheduling prevent runaway tasks.

The async runtime is configured at platform startup and managed by the Core Platform. Individual services run as Tokio tasks spawned within the runtime.

### Async Patterns

- **Fire-and-forget**: Events that require no response are dispatched and processed independently.
- **Request-response**: For operations requiring a result, the requestor awaits a response event on a correlation ID.
- **Stream processing**: Long-running event sequences use Tokio `Stream` and `Sink` traits.
- **Backpressure**: The EventBus applies backpressure through bounded channels, preventing producers from overwhelming consumers.

---

## Service Lifecycle

Every component in AI-native OS follows a standardized lifecycle managed by the Core Platform's lifecycle subsystem.

### Lifecycle States

```
                 +-----------+
                 |  Init     |
                 +-----------+
                      |
                      v
                 +-----------+
                 |  Starting |
                 +-----------+
                      |
                      v
                 +-----------+
            +---->|  Running  |<----+
            |     +-----------+     |
            |           |           |
            |           v           |
            |     +-----------+     |
            +-----|  Stopping |     |
                  +-----------+     |
                       |           |
                       v           |
                  +-----------+     |
                  |  Stopped  |-----+
                  +-----------+
```

1. **Init**: The service is constructed and dependencies are injected.
2. **Starting**: The service initializes its internal state, registers subscriptions with the EventBus, and spawns background tasks.
3. **Running**: The service processes events and performs its primary function.
4. **Stopping**: The service receives a shutdown signal, drains in-flight work, and unregisters subscriptions.
5. **Stopped**: The service has released all resources and terminated.

The Supervisor (Runtime Platform) monitors service health and may restart services that fail unexpectedly, transitioning them back through Starting to Running.

---

## How Modules Communicate

Modules communicate exclusively through the EventBus. There are three communication patterns:

### 1. Event Notification

A module dispatches an event and has no expectation of a response. This is the most common pattern. Example: `runtime.task.completed` is dispatched by the Task Manager and consumed by the Session Manager to advance session state.

### 2. Request-Response

A module dispatches a request event and awaits a response event correlated by a request ID. The requesting module uses a oneshot channel registered with a correlation map. The responding module processes the request and dispatches a response event containing the same correlation ID.

```
Service A                    EventBus                    Service B
    |                           |                           |
    |--- dispatch(request) ---->|                           |
    |                           |--- dispatch(request) ---->|
    |                           |                           |
    |                           |<--- dispatch(response) ---|
    |<--- dispatch(response) ---|                           |
```

### 3. Stream Subscription

A module subscribes to a stream of related events. The EventBus delivers events to the subscriber as they arrive. This is the pattern used for log streaming, health monitoring, and telemetry.

---

## Cross-Cutting Concerns

### Observability

Observability is built into the architecture at every level:
- Every event carries trace and span context.
- The Logger produces structured, machine-parseable log entries.
- The HealthMonitor provides liveness and readiness probes.
- Metrics are emitted for every EventBus operation.
- A dedicated tracing middleware layer captures and exports span data.

### Security

Security is enforced at multiple layers:
- **Service level**: Every service runs in a security context with an identity.
- **Event level**: Every event carries its security context and is authorized before dispatch.
- **Permission level**: The Permission Checker evaluates access control rules before action execution.
- **Policy level**: The Policy Engine evaluates higher-level rules (e.g., "services from this source must not access that capability").

### Configuration

Configuration follows a layered merge model:
1. Default values (compiled into binaries)
2. Configuration files (TOML, YAML, JSON)
3. Environment variables
4. Runtime configuration events (dispatched on the EventBus)
5. CLI flags

Later sources override earlier ones. Configuration changes at runtime take effect via configuration events without requiring service restart.

### Error Handling

All components follow the Fail Fast and Graceful principle:
- Detectable invariant violations cause immediate failure (fail fast).
- Failures are reported via events on the EventBus (fail gracefully).
- The Supervisor captures failure events and applies restart policies.
- Health checks detect degraded states before they become failures.

---

## Phase Architecture Summary

| Phase | Layer | Dependencies | Key Modules |
|---|---|---|---|
| 1 (Dev Environment) | Foundation | None | Toolchain, CI/CD, Dev container |
| 2 (Core Platform) | L0 | Phase 1 | EventBus, Lifecycle, Logger, Health, Container |
| 3 (Runtime Platform) | L1 | Phase 2 | Scheduler, Supervisor, Session, Task, Context, StateMachine, Resource, Permission |
| 4 (System Platform) | L2 | Phase 3 | Daemon, Policy, Capability, Config, Plugin |
| 5 (Memory Platform) | L3 | Phase 4 | Persistent stores, Pattern storage, Query engine |
| 6 (Brain Platform) | L4 | Phase 5 | Reasoning, Decision, State modeling, Learning |
| 7 (Perception Platform) | L5 | Phase 6 | Sensor integration, Signal processing, Pattern recognition |
| 8 (Execution Platform) | L6 | Phase 7 | Action planning, Multi-agent coordination |
| 9 (Intelligence Integration) | L7 | Phase 8 | Unified intelligence, Self-optimization |

---

## Architectural Constraints

1. **No circular dependencies**: The crate dependency graph must be a DAG, verified by Cargo build.
2. **No upward dependencies**: A module in layer N may not depend on modules in layer N+1 or higher.
3. **No direct module references**: Modules communicate only through the EventBus, never through direct function calls across module boundaries.
4. **All state is observable**: No component may hold essential state that is not accessible through the EventBus or observability infrastructure.
5. **All services are restartable**: No service may assume it runs forever; all state must be recoverable.
6. **All events are serializable**: Events must implement Serde's Serialize and Deserialize for persistence and replay.

---

## Technology Stack

| Component | Choice | Rationale |
|---|---|---|
| Language | Rust (stable, latest) | Memory safety, performance, async support |
| Async runtime | Tokio | Production maturity, ecosystem, work-stealing |
| Serialization | Serde | Standard Rust serialization framework |
| Event Bus | Custom (in-house) | Designed for typed, traceable, async event routing |
| Container | Docker / Podman (via API) | Industry standard container runtimes |
| Storage (Phase 5+) | TBD | Evaluated at Phase 5 design |
| Tracing | tracing crate + OpenTelemetry | Industry standard for distributed tracing |
| Metrics | metrics crate | Lightweight, composable metrics collection |
| CLI | clap | Standard Rust CLI argument parser |
| Configuration | config crate | Layered configuration with merge support |

---

## Summary

The architecture of AI-native OS is designed for a system that must grow from a minimal core to a full intelligence platform over a decade of development. Clean layering, event-driven communication, and async-first execution provide the foundation for this growth without sacrificing performance, security, or comprehensibility.

Every architectural decision is guided by the [design principles](principles.md). Every term is defined in the [glossary](glossary.md). Every phase is tracked on the [roadmap](roadmap.md).
