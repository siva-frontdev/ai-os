# Core Platform Interface — `core`

## Purpose

The Core layer defines the foundational abstractions that every other layer depends on. It provides the shared infrastructure for inter-layer communication (EventBus), service lifecycle management (Service), structured diagnostics (Logger, HealthMonitor), dynamic configuration (Config), and lightweight containerization (Container). All layers above Core interact exclusively through these abstractions — no layer bypasses the EventBus to communicate directly with another layer.

## Public APIs

### EventBus — typed publish/subscribe with async dispatch

```rust
#[async_trait]
pub trait EventBus: Debug + Send + Sync {
    /// Publish an event. Returns immediately; dispatch is async.
    async fn publish(&self, event: Box<dyn Event>) -> Result<EventId, BusError>;

    /// Subscribe to events of a specific TypeId.
    /// Returns a Subscription guard that unsubscribes on drop.
    async fn subscribe<T: Event>(
        &self,
        handler: Box<dyn Fn(T) -> BoxFuture<'static, ()> + Send + Sync>,
    ) -> Result<Subscription, BusError>;

    /// Unsubscribe a previously registered subscription.
    async fn unsubscribe(&self, sub: Subscription) -> Result<(), BusError>;

    /// Wait for an event matching a predicate (one-shot observer).
    async fn observe<F, T>(&self, predicate: F) -> Result<T, BusError>
    where
        F: Fn(&T) -> bool + Send + 'static,
        T: Event;
}
```

### Service — lifecycle state machine

```rust
#[async_trait]
pub trait Service: Debug + Send + Sync {
    async fn init(&self, ctx: CapabilityContext) -> Result<(), ServiceError>;
    async fn start(&self) -> Result<(), ServiceError>;
    async fn stop(&self) -> Result<(), ServiceError>;
    fn state(&self) -> ServiceState;
}

pub enum ServiceState {
    Uninitialized,
    Initialized,
    Running,
    Stopped,
    Faulted(String),
}
```

### Logger — structured, level-gated diagnostics

```rust
pub trait Logger: Debug + Send + Sync {
    fn log(&self, entry: LogEntry);
    fn set_level(&self, level: LogLevel);
    fn subscribe(&self, sink: Box<dyn LogSink>);
}
```

### HealthMonitor — component health checks

```rust
#[async_trait]
pub trait HealthMonitor: Debug + Send + Sync {
    async fn register(&self, probe: Box<dyn HealthProbe>) -> Result<(), HealthError>;
    async fn status(&self, component: &str) -> Result<HealthStatus, HealthError>;
    async fn snapshot(&self) -> Result<HealthSnapshot, HealthError>;
    fn events(&self) -> Box<dyn Stream<Item = HealthEvent> + Unpin + Send>;
}
```

### Container — lightweight resource sandbox

```rust
#[async_trait]
pub trait Container: Debug + Send + Sync {
    async fn create(&self, spec: ContainerSpec) -> Result<ContainerHandle, ContainerError>;
    async fn destroy(&self, handle: ContainerHandle) -> Result<(), ContainerError>;
    fn limits(&self) -> ResourceLimits;
}
```

### Config — typed, hierarchical, hot-reloadable

```rust
pub trait Config: Debug + Send + Sync {
    fn get<T: DeserializeOwned>(&self, key: &ConfigKey) -> Result<T, ConfigError>;
    fn set<T: Serialize>(&self, key: &ConfigKey, value: &T) -> Result<(), ConfigError>;
    fn watch<T: DeserializeOwned>(
        &self,
        key: &ConfigKey,
    ) -> Box<dyn Stream<Item = T> + Unpin + Send>;
    fn reload(&self) -> Result<(), ConfigError>;
}
```

## Dependencies

- **None.** Core has zero dependencies on other OS layers. It depends only on:
  - `tokio` (async runtime, channels)
  - `std::any::TypeId` (event routing)
  - `serde` / `serde_json` (config serialization)
  - `tracing` (structured diagnostics backend)

## Lifecycle

1. **Init** — `EventBus` is created first, followed by `Config`, `Logger`, and `HealthMonitor`.
2. **Start** — Each Core service starts accepting registrations. `EventBus` begins dispatching.
3. **Stop** — `EventBus` is drained, subscriptions are cancelled, remaining events are flushed.
4. **Ordering** — `Logger` must outlive all other services. `EventBus` is the last to stop.

## Events Published

| Event | Payload | Description |
|-------|---------|-------------|
| `core.service_started` | `{ service: String, timestamp: u64 }` | Emitted when a Service transitions to Running |
| `core.service_stopped` | `{ service: String, reason: String }` | Emitted when a Service transitions to Stopped |
| `core.service_faulted` | `{ service: String, error: String }` | Emitted when a Service transitions to Faulted |
| `core.health_change` | `{ component: String, from: HealthStatus, to: HealthStatus }` | Emitted on health status transitions |
| `core.config_changed` | `{ key: ConfigKey, old: Value, new: Value }` | Emitted when a watched config key changes |

## Events Consumed

None. Core does not subscribe to any events — it only publishes.

## Error Model

| Error | Variant | Recovery |
|-------|---------|----------|
| `BusError::Full` | Channel capacity exhausted | Backpressure — caller retries with backoff |
| `BusError::NoSubscribers` | Published event had zero subscribers | Silently dropped (informational) |
| `BusError::Closed` | Bus has been shut down | Return to caller, service should stop |
| `ServiceError::InitFailed` | Service init hook failed | Log fault, transition to Faulted |
| `ServiceError::StateViolation` | Illegal state transition attempted | Panic in debug, error in release |
| `ConfigError::NotFound` | Config key does not exist | Fall back to default if available |
| `ConfigError::TypeMismatch` | Deserialization failed | Return error, caller must handle |

No panics in release mode. All errors are surfaced as `Result` variants.

## Performance Expectations

| Operation | Target Latency (p99) | Throughput |
|-----------|---------------------|------------|
| Event publish (no subscribers) | < 1 µs | 10M+ events/s |
| Event publish (1 subscriber) | < 5 µs | 2M+ events/s |
| Event publish (10 subscribers) | < 15 µs | 500K+ events/s |
| Config get (hot path) | < 100 ns | 50M+ reads/s |
| Logger entry (info level) | < 2 µs | 500K+ entries/s |
| Health probe execution | < 10 ms | N/A (per-probe) |

## Thread Model

- `EventBus` uses a tokio `broadcast` channel per topic group. Publishing is lock-free.
- `Config` stores values behind `Arc<RwLock<HashMap>>`. Reads are shared, writes exclusive.
- `Logger` sinks run on a dedicated spawn_blocking thread to avoid blocking the async runtime.
- `HealthMonitor` probes are run on a tokio interval. Long probes are subject to timeout.
- All Core types are `Send + Sync`. Shared state is behind `Arc`. Lock scopes are minimal and local.

## Portability

- Core has zero OS-specific dependencies.
- `EventBus` routing uses `TypeId`, which is stable across all Rust targets.
- `Config` backends (file, env, etcd) are selected at build time via Cargo features.
- The only `cfg`-gated code in Core is for `Logger` backends (journald on Linux, oslog on macOS).
- All time handling uses `std::time::Instant` / `Duration` — no platform time APIs.
