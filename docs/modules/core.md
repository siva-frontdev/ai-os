# Core Platform Module

**Module:** `ai_os_core`
**Status:** Active (Phase 1)
**Crate:** `core/ai_os_core/`

## Purpose

The Core Platform module provides the foundational abstractions for the entire AI-native OS. It implements the event-driven architecture backbone, a uniform service lifecycle contract, structured observability, health monitoring, and a lightweight dependency injection container. Every other module depends on Core; it is the single kernel of the system.

## Responsibilities

- Event routing: type-erased publish/subscribe with dispatch-level filtering and async handler execution.
- Service lifecycle: define and govern the init -> start -> stop -> fail state machine for all long-lived components.
- Structured logging: emit JSON-formatted logs via `tracing` with module-level severity control.
- Health checking: register health probes, aggregate results into a composite 3-state health signal, cache aggressively.
- Service container: register, retrieve, and manage singleton-scoped service instances by `TypeId`.

## Public Interfaces

### `EventBus`

Located at `ai_os_core::events::EventBus`. A type-erased, asynchronous publish/subscribe channel keyed on `TypeId`.

```rust
use std::any::TypeId;
use ai_os_core::events::{EventBus, HandlerAdapter};
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct FileCreated {
    pub path: String,
    pub size: u64,
}

// A handler adapter bridges the concrete handler to the type-erased bus.
struct FileCreatedLogger;

#[async_trait]
impl HandlerAdapter for FileCreatedLogger {
    fn handles(&self) -> Vec<TypeId> {
        vec![TypeId::of::<FileCreated>()]
    }

    async fn handle(&self, event: Box<dyn std::any::Any + Send>) {
        if let Ok(ev) = event.downcast::<FileCreated>() {
            tracing::info!("file created: {} ({} bytes)", ev.path, ev.size);
        }
    }
}

let bus = EventBus::new();
bus.subscribe(Box::new(FileCreatedLogger));
bus.publish(FileCreated { path: "/tmp/test".into(), size: 0 }).await;
```

### `Service` Trait

Located at `ai_os_core::service::Service`. Defines the canonical lifecycle with state tracking.

```rust
use ai_os_core::service::{Service, State};

struct MyService {
    state: State,
}

#[async_trait]
impl Service for MyService {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.state = State::Initialized;
        Ok(())
    }

    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.state = State::Running;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.state = State::Stopped;
        Ok(())
    }

    fn state(&self) -> State {
        self.state
    }
}
```

The four states are:
- `Initialized` -- after `init()` succeeds.
- `Running` -- after `start()` succeeds.
- `Stopped` -- after `stop()` returns.
- `Failed` -- entered when any lifecycle method returns an error; queries are still allowed.

### `Logger`

Located at `ai_os_core::telemetry::Logger`. A configured `tracing_subscriber` layer that writes structured JSON.

```rust
use ai_os_core::telemetry::Logger;

let logger = Logger::builder()
    .json(true)
    .module("ai_os_core::events", tracing::Level::DEBUG)
    .module("ai_os_runtime", tracing::Level::INFO)
    .build();
logger.install(); // sets the global subscriber
```

The logger supports:
- Structured JSON output to stdout (or a configurable file sink).
- Fine-grained module-level filter overrides.
- Span and event field capture for distributed tracing correlation.

### `HealthMonitor`

Located at `ai_os_core::health::HealthMonitor`. Manages a registry of health checks and exposes a cached, composite status.

```rust
use ai_os_core::health::{HealthMonitor, HealthStatus, HealthCheck};

struct DiskCheck;

#[async_trait]
impl HealthCheck for DiskCheck {
    async fn check(&self) -> HealthStatus {
        // returns Healthy, Degraded, or Unhealthy
        HealthStatus::Healthy
    }
    fn name(&self) -> &'static str {
        "disk"
    }
}

let mut monitor = HealthMonitor::new();
monitor.register(Box::new(DiskCheck));

let overall = monitor.aggregate().await; // cached for 5 s
assert_eq!(overall, HealthStatus::Healthy);
```

The three aggregate statuses form a partial order: `Healthy > Degraded > Unhealthy`. If any probe returns `Unhealthy`, the aggregate is `Unhealthy`.

### `Container`

Located at `ai_os_core::container::Container`. A `HashMap<TypeId, Box<dyn Any + Send + Sync>>` service registry with singleton semantics.

```rust
use ai_os_core::container::Container;

let mut c = Container::new();
c.register::<dyn Service>(Box::new(MyService));

let svc: &dyn Service = c.resolve::<dyn Service>().unwrap();
```

Resolution panics if the type is not registered; callers should prefer the `try_resolve` variant.

## Dependencies

| Dependency | Scope | Purpose |
|---|---|---|
| `tokio` | runtime | Async task execution for `EventBus` dispatch |
| `tracing` | logging | Instrumentation framework |
| `tracing-subscriber` | logging | JSON formatting and filter layers |
| `async-trait` | compile | Allow `async fn` in trait definitions |
| `std::any` | stdlib | `TypeId`-based routing and downcasting |
| `std::collections::HashMap` | stdlib | Service registry and check registry |

No external crate dependencies beyond the Tokio ecosystem and tracing stack.

## Events Published

| Event | Trigger | Payload |
|---|---|---|
| `core::EventBusOverflow` | Internal channel capacity exceeded | `(TypeId, usize)` -- type and dropped count |
| `core::ServiceStateChanged` | Any service transitions state | `(TypeId, State, State)` -- service type, old, new |
| `core::HealthStatusChanged` | `HealthMonitor.aggregate()` returns a new status | `(HealthStatus, HealthStatus)` -- old, new |

## Events Consumed

| Event | Consumer | Action |
|---|---|---|
| `core::ServiceStateChanged` | `HealthMonitor` | Re-evaluate affected health probe |
| `runtime::TaskStarted` | `Logger` | Emit task lifecycle span |
| `runtime::TaskCompleted` | `Logger` | Close task lifecycle span |

## Thread Model

- `EventBus` uses a `tokio::sync::broadcast` channel internally; `Send + Sync` on the bus handle. `publish` is non-blocking; handler dispatch runs on the caller's task unless the handler spawns its own.
- `Container` is `Send + Sync` behind an `Arc<RwLock<...>>`. Reads (resolve) are lock-contention-free in practice because registration happens at startup.
- `Logger` delegates to `tracing`'s global filter, which is `Send + Sync`. The JSON formatter writes to a `tokio::io::BufWriter` behind a `Mutex`.
- `HealthMonitor` holds checks in an `Arc<Vec<...>>`; the aggregate cache uses a `tokio::sync::RwLock<Option<(HealthStatus, Instant)>>`.

No `unsafe` code is used anywhere in the public interface.

## Lifecycle

```
            init()
  ┌─────────────────────┐
  v                     │
New ──> Initialized ───┘
           │
        start()
           │
           v
        Running ──> Stopped
           │
           │ error
           v
         Failed
```

Services enter `Failed` when any lifecycle method errors. A failed service can be re-initialized via `reset()` followed by `init()`. The `Container` does not automatically restart failed services -- that is the responsibility of the Supervisor (see [Runtime](runtime.md)).

## Error Handling

All public fallible methods return `Result<T, Box<dyn std::error::Error>>` to allow maximum flexibility during early development. Planned error types:

- `EventBusError`: `ChannelClosed`, `DispatchFailed`, `TypeMismatch`.
- `ServiceError`: `AlreadyInitialized`, `NotInitialized`, `AlreadyRunning`.
- `ContainerError`: `TypeNotRegistered`, `TypeAlreadyRegistered`.

Recovery: `EventBus` dispatch failures are logged but do not propagate (fire-and-forget). Service lifecycle errors leave the service in `Failed` state. Container registration conflicts are returned immediately and should be caught at integration test time.

## Configuration

Configuration is loaded from `aios-core.toml` at startup:

```toml
[logging]
format = "json"                    # "json" | "text"
default_level = "info"             # fallback level
overrides = { "my_crate::noisy" = "warn" }

[health]
cache_ttl_secs = 5

[events]
internal_channel_capacity = 2048
```

The `Logger` builder reads from this struct; all fields have safe defaults.

## Testing Strategy

- **Unit tests**: EventBus round-trip publish/subscribe with mock handlers. Service lifecycle state transitions (must reject `start` before `init`). HealthMonitor aggregation logic (Degraded + Unhealthy = Unhealthy).
- **Property-based tests**: Container type registration and resolution round-trips (via `proptest` with a fixed set of types).
- **Integration tests**: Wire up a real `EventBus` + `Logger` + `HealthMonitor` and verify end-to-end logging output on state changes.
- **Benchmarks**: EventBus throughput at varying subscriber counts. HealthMonitor aggregate cache hit rate under load.

## Future Extensions

- Prioritized event channels (urgent vs. background) to prevent starvation.
- Hierarchical health checks with dependency-aware status propagation.
- Dynamic service registration at runtime (hot-plug).
- A `Noop` service implementation for testing downstream consumers.
- Structured error type stabilization (`thiserror` + `Display`) once the error surface hardens.
- Backpressure-aware `EventBus` with bounded blocking `publish`.
