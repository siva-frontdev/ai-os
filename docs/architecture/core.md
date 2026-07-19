# Core Platform Deep Dive

## Purpose

The Core Platform is the foundational layer of AI-native OS. It provides the essential services that every other module depends on: inter-component communication (EventBus), component lifecycle management (Service), structured observability (Logger), health monitoring (HealthMonitor), dependency injection (Container), service discovery (Registry), and configuration (ConfigProvider). No module above the Core layer should need to reimplement any of these concerns.

The Core Platform has zero dependencies on any other AI-native OS crate. It depends only on the Rust standard library and well-established third-party crates (Tokio, Serde, tracing, chrono, thiserror, async-trait, uuid).

---

## Responsibilities

- **EventBus**: Provide an in-process publish/subscribe mechanism that routes typed events from publishers to subscribers. Support type-safe and type-erased subscription patterns. Enable loose coupling between all modules.
- **Service Lifecycle**: Define the `Service` trait that every long-lived component implements. Manage service registration, ordered startup, reverse-ordered shutdown, and state tracking.
- **Logger**: Provide a structured, multi-sink logging abstraction with severity levels, hierarchical targets, and key-value field attachment. Bridge to the `tracing` ecosystem for OpenTelemetry compatibility.
- **HealthMonitor**: Maintain a registry of health checks. Run checks on demand or periodically. Cache reports for querying. Report three-state health (healthy, degraded, unhealthy).
- **Container**: Provide a string-keyed, thread-safe dependency injection container that stores singleton instances behind `Arc<dyn Any + Send + Sync>`.
- **Registry**: Maintain a name-based directory of every running service with version and dependency metadata.
- **Config**: Provide a layered configuration system that merges defaults, file sources, environment variables, and runtime updates.
- **Bootstrap**: Wire all core subsystems together through a builder pattern (`PlatformBuilder`) and return a ready-to-run `Application` handle.
- **Error Handling**: Define `CoreError` as the single error type for all core operations, with variants for each subsystem.

---

## Public Interfaces

### EventBus (`core::events`)

```rust
// Core trait (dyn-compatible)
pub trait EventBus: Debug + Send + Sync {
    async fn publish_event(&self, event: &dyn Event) -> Result<(), CoreError>;
    fn subscribe_erased(
        &self,
        type_id: TypeId,
        handler: Arc<dyn ErasedEventHandler>,
    ) -> Result<Subscription, CoreError>;
    fn unsubscribe(&self, sub: &Subscription) -> Result<(), CoreError>;
}

// Type-safe helpers
pub async fn typed_publish<E: Event>(
    bus: &dyn EventBus,
    event: &E,
) -> Result<(), CoreError>;

pub fn typed_subscribe<E: Event + 'static, H: EventHandler<E> + 'static>(
    bus: &dyn EventBus,
    handler: Arc<H>,
) -> Result<Subscription, CoreError>;

// Event trait
pub trait Event: Any + Debug + Send + Sync + 'static {
    fn event_type(&self) -> &'static str;
}

// Handler traits
pub trait EventHandler<E: Event>: Debug + Send + Sync + 'static {
    async fn handle(&self, event: &E) -> Result<(), CoreError>;
}
```

### Service Lifecycle (`core::lifecycle`)

```rust
pub enum ServiceState {
    Created = 0,
    Initializing = 1,
    Running = 2,
    Stopping = 3,
    Stopped = 4,
    Failed = 5,
}

pub trait Service: Debug + Send + Sync + 'static {
    fn name(&self) -> &str;
    async fn start(&self) -> Result<(), CoreError>;
    async fn stop(&self) -> Result<(), CoreError>;
}

pub trait LifecycleManager: Debug + Send + Sync {
    async fn register(&self, service: Arc<dyn Service>) -> Result<(), CoreError>;
    async fn start_all(&self) -> Result<(), CoreError>;
    async fn stop_all(&self) -> Result<(), CoreError>;
    fn state(&self, name: &str) -> Option<ServiceState>;
    fn status(&self) -> Vec<ServiceStatus>;
    fn service_count(&self) -> usize;
}
```

### Logger (`core::logging`)

```rust
pub enum LogLevel { Trace, Debug, Info, Warn, Error }

pub struct LogRecord {
    pub level: LogLevel,
    pub message: String,
    pub target: String,
    pub timestamp: DateTime<Utc>,
    pub fields: HashMap<String, Value>,
}

pub trait Logger: Debug + Send + Sync {
    fn log(&self, record: LogRecord);
    fn trace(&self, msg: &str);
    fn debug(&self, msg: &str);
    fn info(&self, msg: &str);
    fn warn(&self, msg: &str);
    fn error(&self, msg: &str);
    fn target(&self) -> &str;
    fn child(&self, target: &str) -> Arc<dyn Logger>;
    fn with_fields(&self, fields: HashMap<String, Value>) -> Arc<dyn Logger>;
}

pub trait LogSink: Debug + Send + Sync {
    fn write(&self, record: &LogRecord) -> Result<(), CoreError>;
}
```

### HealthMonitor (`core::health`)

```rust
pub enum HealthStatus {
    Healthy,
    Degraded { message: String },
    Unhealthy { message: String },
}

pub struct HealthReport {
    pub service_name: String,
    pub status: HealthStatus,
    pub checked_at: DateTime<Utc>,
    pub details: HashMap<String, String>,
}

pub trait HealthCheck: Debug + Send + Sync + 'static {
    fn name(&self) -> &str;
    async fn check(&self) -> HealthStatus;
}

pub trait HealthMonitor: Debug + Send + Sync {
    fn register(&self, check: Arc<dyn HealthCheck>) -> Result<(), CoreError>;
    async fn run_checks(&self) -> Vec<HealthReport>;
    fn start_periodic(self: Arc<Self>, interval: Duration) -> JoinHandle<()>;
    fn latest(&self, service: &str) -> Option<HealthReport>;
    fn all_latest(&self) -> Vec<HealthReport>;
}
```

### Container (`core::container`)

```rust
pub trait Container: Debug + Send + Sync {
    fn register(&self, key: &str, instance: Arc<dyn Any + Send + Sync>) -> Result<(), CoreError>;
    fn resolve(&self, key: &str) -> Option<Arc<dyn Any + Send + Sync>>;
    fn contains(&self, key: &str) -> bool;
}
```

### Configuration (`core::config`)

```rust
pub trait ConfigProvider: Debug + Send + Sync {
    fn get(&self, key: &str) -> Option<ConfigValue>;
    fn keys(&self) -> Vec<String>;
    fn merge(&self, other: &dyn ConfigProvider) -> Box<dyn ConfigProvider>;
}
```

### Application (`core::application`)

```rust
pub struct Application {
    // Accessors for every subsystem
    pub fn event_bus(&self) -> &Arc<dyn EventBus>;
    pub fn lifecycle(&self) -> &Arc<dyn LifecycleManager>;
    pub fn registry(&self) -> &Arc<dyn ServiceRegistry>;
    pub fn health(&self) -> &Arc<dyn HealthMonitor>;
    pub fn config(&self) -> &Arc<dyn ConfigProvider>;
    pub fn logger(&self) -> &Arc<dyn Logger>;
    pub fn is_running(&self) -> bool;

    pub async fn run(&self) -> Result<(), CoreError>;
    pub async fn shutdown(&self) -> Result<(), CoreError>;
    pub async fn run_until_signal(&self) -> Result<(), CoreError>;
}
```

---

## Dependencies

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime (used in HealthMonitor periodic tasks, Application signal handling) |
| `serde` / `serde_json` | Event serialization, JSON log output |
| `tracing` / `tracing-subscriber` | Log bridge to OpenTelemetry ecosystem |
| `chrono` | Timestamps for log records and health reports |
| `uuid` | ID generation for events and services |
| `thiserror` | Ergonomic error enum derivation |
| `async-trait` | Async trait methods for Service, EventHandler, etc. |

Core depends on **no** other AI-native OS crate. It is the dependency root.

---

## Events Published

| Event | Type | Trigger |
|---|---|---|
| `platform.started` | `StartedEvent` | Emitted after all services have been started successfully |
| `platform.shutdown` | `ShutdownEvent` | Emitted when the platform begins shutdown |

---

## Events Consumed

Core defines no event subscriptions of its own. It provides the infrastructure for other modules to publish and subscribe.

---

## Thread Model

| Component | Threading |
|---|---|
| `InMemoryEventBus` | All operations under `RwLock`. `publish_event` collects handlers under a read lock, then awaits each handler on the caller's task. |
| `DefaultLifecycleManager` | `register`, `start_all`, `stop_all` acquire write/read locks from `RwLock<Vec<ManagedService>>`. Service `start`/`stop` is awaited on the caller's task. |
| `DefaultLogger` | `log` writes synchronously to each `LogSink`. `ConsoleSink` uses `println!` (synchronous stdout). `TracingBridge` uses `tracing::event!` macro which is thread-safe. |
| `DefaultHealthMonitor` | `register` under write lock. `run_checks` runs each `HealthCheck` sequentially on the caller's task. `start_periodic` spawns a Tokio task. Cache reads/writes under `RwLock`. |
| `InMemoryContainer` | All operations under `RwLock<HashMap>`. |
| `DefaultServiceRegistry` | All operations under `RwLock<HashMap>`. |

All `RwLock` instances use `std::sync::RwLock` (not Tokio's) because critical sections are short (HashMap lookups/insertions). The `HealthMonitor` periodic task uses `tokio::spawn` and `tokio::time::interval`.

---

## Lifecycle

Core subsystems are created during `PlatformBuilder::build()` and initialized in a fixed order:

1. `LayeredConfigProvider` -- merge default, file, and environment configuration
2. `DefaultLogger` -- root logger with console sink (and optional tracing bridge)
3. `InMemoryEventBus` -- empty subscriber map
4. `DefaultLifecycleManager` -- empty service list
5. `DefaultServiceRegistry` -- empty registry
6. `DefaultHealthMonitor` -- empty check list and cache
7. `Application` -- wraps all subsystems

The `Application::run()` method starts all registered services via `LifecycleManager::start_all()`. Each service transitions through `Created -> Initializing -> Running`. If any service fails to start, the platform returns the error and subsequent services are not started.

The `Application::shutdown()` method stops all services in reverse order via `LifecycleManager::stop_all()`. Each service transitions `Running -> Stopping -> Stopped`.

---

## Error Handling

`CoreError` is the unified error type for all core operations. Notable variants:

| Variant | Source | Recovery |
|---|---|---|
| `LockPoisoned` | Any RwLock operation | Return error to caller; platform may need restart |
| `HandlerFailed { event_type, detail }` | Event handler execution | Other handlers still execute; error is logged |
| `StartFailed { name, detail }` | `Service::start()` | Platform startup aborts; service set to `Failed` |
| `StopFailed { name, detail }` | `Service::stop()` | Platform shutdown continues; service set to `Failed` |
| `State { name, detail }` | Duplicate registration | Registration rejected |
| `ServiceAlreadyRegistered` | Container duplicate | Registration rejected |
| `ServiceNotFound` | Registry resolve | Caller receives error |

The `Fail Fast and Gracefully` principle applies: invariant violations (e.g. duplicate registration) are detected immediately and reported, preventing silent corruption.

---

## Subsystem Interaction Diagrams

### EventBus Dispatch Flow

```
                   InMemoryEventBus
                         |
    typed_publish()      |
         |               |
         v               |
    publish_event()      |
         |               |
         v               |
    (event as &dyn Any)  |
        .type_id()       |
         |               |
         v               |
    subscribers.read()   |
         |               |
         v               |
    match type_id in     |
    HashMap<TypeId,      |
      Vec<SubscriberEntry>>
         |               |
         +-- no match -> Ok(())
         |               |
         +-- match -----> for each handler:
                              handler.handle(event).await
         |               |
         v               |
    Ok(())               |
```

### Service Startup Flow

```
LifecycleManager          ManagedService            Service (impl)
    |                         |                         |
    |  register(svc)          |                         |
    |------------------------>|                         |
    |  ManagedService::new()  |                         |
    |                         |                         |
    |  start_all()            |                         |
    |                         |                         |
    |  for each service:      |                         |
    |    set_state(Init)      |                         |
    |    start() ------------>|                         |
    |                         |  service.start()        |
    |                         |------------------------>|
    |                         |                         |
    |                         |  Ok(())                 |
    |                         |<------------------------|
    |    set_state(Running)   |                         |
    |    record started_at    |                         |
    |                         |                         |
```

### Health Check Flow

```
HealthMonitor                HealthCheck (impl)
    |                            |
    |  register(check)           |
    |----------------------------|
    |                            |
    |  run_checks()              |
    |                            |
    |  for each check:           |
    |    check()                 |
    |--------------------------->|
    |    HealthStatus            |
    |<---------------------------|
    |                            |
    |  build HealthReport        |
    |  cache.insert(name, report)|
    |                            |
    |  return Vec<HealthReport>  |
```

---

## Future Extensions

- **EventBus Middleware Pipeline** -- Add a middleware stack (logging, metrics, tracing, permission checking) that wraps every event dispatch.
- **Wildcard Subscription** -- Add prefix or pattern-based subscription matching so a single handler can receive multiple event types.
- **Persistent Event Log** -- Write all events to an append-only log for replay, debugging, and audit.
- **Config Hot-Reload** -- Subscribe to configuration change events and propagate updates to running services without restart.
- **Async Log Sinks** -- Make `LogSink::write` async and buffer log records to a Tokio mpsc channel for non-blocking I/O.
- **Health Check Timeouts** -- Add per-check timeouts using `tokio::time::timeout` to prevent hung checks from blocking the health report.
- **Container Lifecycle Hooks** -- Add pre-start and post-stop hooks to the `Service` trait.
- **Distributed Container Support** -- Extend the `Container` module (currently string-keyed DI) to support remote container management via the EventBus.

---

## References

- [Architecture Overview](overview.md)
- [Runtime Platform Deep Dive](runtime.md)
- [InMemoryEventBus Source](../../core/src/events/mod.rs)
- [Lifecycle Source](../../core/src/lifecycle/mod.rs)
- [Logger Source](../../core/src/logging/mod.rs)
- [HealthMonitor Source](../../core/src/health/mod.rs)
- [Container Source](../../core/src/container/mod.rs)
- [Registry Source](../../core/src/registry/mod.rs)
- [Bootstrap Source](../../core/src/bootstrap/mod.rs)
- [Application Source](../../core/src/application/mod.rs)
- [Design Principles](../principles.md)
