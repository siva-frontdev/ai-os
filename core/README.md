# AI-OS Core

The foundational layer of the AI-native operating platform.

## Modules

| Module | Responsibility | Key Trait | Key Implementations |
|---|---|---|---|
| `config` | Configuration sources | `ConfigProvider` | `InMemoryConfigProvider`, `LayeredConfigProvider`, `ThreadsafeConfigProvider` |
| `container` | Dependency injection | `Container` | `InMemoryContainer` |
| `logging` | Structured logging | `Logger`, `LogSink` | `DefaultLogger`, `ConsoleSink`, `TracingBridge`, `NullLogger` |
| `events` | In-process pub/sub | `EventBus` | `InMemoryEventBus` plus `typed_publish`/`typed_subscribe` helpers |
| `lifecycle` | Service lifecycle | `Service`, `LifecycleManager` | `DefaultLifecycleManager` |
| `registry` | Service directory | `ServiceRegistry` | `DefaultServiceRegistry` |
| `health` | Health monitoring | `HealthCheck`, `HealthMonitor` | `DefaultHealthMonitor` |
| `bootstrap` | Wiring & entry point | — | `PlatformBuilder` |
| `application` | Top-level handle | — | `Application` |
| `error` | Unified error type | — | `CoreError` |
| `utils` | Shared helpers | — | `Id`, `now`, `ulid` |

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Application (facade with direct accessors)         │
├─────────────────────────────────────────────────────┤
│  Bootstrap (PlatformBuilder — wires all subsystems) │
├──────────┬──────────┬──────────┬────────────────────┤
│ Config   │ Logger   │ EventBus │ HealthMonitor      │
│ Registry │ Lifecycle│ Container│                    │
├──────────┴──────────┴──────────┴────────────────────┤
│  Error (CoreError)   │   Utils (Id, now, ...)       │
└─────────────────────────────────────────────────────┘
```

### Clean Architecture

- **Inner layer**: `error`, `utils` — zero dependencies on other modules
- **Infrastructure**: `config`, `logging`, `events`, `container` — define interfaces
- **Orchestration**: `lifecycle`, `registry`, `health` — compose infrastructure
- **Entry point**: `bootstrap`, `application` — wire everything

### SOLID

| Principle | How it's applied |
|---|---|
| **S**ingle Responsibility | Each module owns exactly one concern |
| **O**pen/Closed | All public traits — swap implementations without modifying consumers |
| **L**iskov Substitution | All implementations satisfy their trait contracts |
| **I**nterface Segregation | Small, focused traits (no kitchen-sink interfaces) |
| **D**ependency Inversion | High-level modules depend on abstractions, not concretions |

### Thread Safety

- Every trait requires `Send + Sync`
- Shared state protected by `RwLock` or `Atomic*`
- Events use `Arc<dyn Event>` for zero-copy fan-out
- Container stores `Arc<dyn Any + Send + Sync>`

## Usage

```rust
use ai_os_core::bootstrap::PlatformBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = PlatformBuilder::new()
        .with_tracing()                         // enable tracing bridge
        .build().await?;

    app.run_until_signal().await?;              // blocks until SIGINT
    Ok(())
}
```

### Register a custom service

```rust
use ai_os_core::lifecycle::{Service, ServiceState};
use async_trait::async_trait;

#[derive(Debug)]
struct MyService;

#[async_trait]
impl Service for MyService {
    fn name(&self) -> &str { "my-service" }
    async fn start(&self) -> Result<(), CoreError> { Ok(()) }
    async fn stop(&self) -> Result<(), CoreError> { Ok(()) }
}

app.lifecycle().register(Arc::new(MyService)).await?;
app.run().await?;
```

### Subscribe to events

```rust
use ai_os_core::events::*;
use async_trait::async_trait;

struct MyHandler;
#[async_trait]
impl EventHandler<StartedEvent> for MyHandler {
    async fn handle(&self, _event: &StartedEvent) -> Result<(), CoreError> {
        println!("Platform started!");
        Ok(())
    }
}

typed_subscribe(app.event_bus().as_ref(), Arc::new(MyHandler))?;
```

## Testing

Run all tests:

```bash
cargo test -p ai-os-core
```

42 tests cover:
- Unit tests in every module (35)
- Integration tests (7) covering full bootstrap → run → shutdown, event delivery, health checks, config querying, and start/stop ordering

## Dependencies

- `tokio` — async runtime
- `serde` / `serde_json` — serialization
- `tracing` / `tracing-subscriber` — structured logging backend
- `chrono` — timestamps
- `uuid` — unique identifiers
- `async-trait` — async trait support
- `thiserror` — error derivation
