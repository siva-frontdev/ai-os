# Execution Platform Interface — `execution` *(planned, Phase 8)*

## Purpose

The Execution layer is the OS's motor cortex. It receives commands from the Brain layer, dispatches them to one of several backends (subprocess, wasm, container), captures output with resource accounting, enforces timeouts, and routes results back to the caller. It provides a uniform `Command` abstraction that hides backend-specific details — the Brain never knows whether a command ran in a subprocess or a wasm sandbox.

## Public APIs

### CommandEngine — command dispatch and lifecycle

```rust
#[async_trait]
pub trait CommandEngine: Debug + Send + Sync {
    async fn execute(&self, cmd: Command) -> Result<ExecutionHandle, EngineError>;
    async fn cancel(&self, handle: &ExecutionHandle) -> Result<(), EngineError>;
    async fn status(&self, handle: &ExecutionHandle) -> Result<ExecutionStatus, EngineError>;
    async fn list_active(&self) -> Result<Vec<ExecutionSummary>, EngineError>;
    async fn select_backend(&self, cmd: &Command) -> ExecutionBackend;
    fn events(&self) -> Box<dyn Stream<Item = ExecutionEvent> + Unpin + Send>;
}
```

### OutputCapture — stdout/stderr collection

```rust
#[async_trait]
pub trait OutputCapture: Debug + Send + Sync {
    async fn stdout(&self, handle: &ExecutionHandle) -> Result<Vec<u8>, CaptureError>;
    async fn stderr(&self, handle: &ExecutionHandle) -> Result<Vec<u8>, CaptureError>;
    async fn stream_stdout(&self, handle: &ExecutionHandle) -> Result<Box<dyn Stream<Item = Vec<u8>> + Unpin + Send>, CaptureError>;
    async fn stream_stderr(&self, handle: &ExecutionHandle) -> Result<Box<dyn Stream<Item = Vec<u8>> + Unpin + Send>, CaptureError>;
    async fn discard(&self, handle: &ExecutionHandle) -> Result<(), CaptureError>;
    async fn set_max_output(&self, handle: &ExecutionHandle, limit: ByteSize) -> Result<(), CaptureError>;
}
```

### TimeoutManager — per-command deadline enforcement

```rust
#[async_trait]
pub trait TimeoutManager: Debug + Send + Sync {
    async fn set_timeout(&self, handle: &ExecutionHandle, deadline: Instant) -> Result<(), TimeoutError>;
    async fn cancel_timeout(&self, handle: &ExecutionHandle) -> Result<(), TimeoutError>;
    async fn remaining(&self, handle: &ExecutionHandle) -> Result<Option<Duration>, TimeoutError>;
    async fn default_timeout(&self) -> Duration;
    async fn set_default_timeout(&self, duration: Duration) -> Result<(), TimeoutError>;
}
```

### ResourceAccountant — CPU, memory, I/O tracking

```rust
#[async_trait]
pub trait ResourceAccountant: Debug + Send + Sync {
    async fn usage(&self, handle: &ExecutionHandle) -> Result<ResourceUsage, AccountantError>;
    async fn peak_usage(&self, handle: &ExecutionHandle) -> Result<ResourceUsage, AccountantError>;
    async fn total_usage(&self, session: &SessionId) -> Result<ResourceUsage, AccountantError>;
    async fn set_limit(&self, handle: &ExecutionHandle, limit: ResourceLimit) -> Result<(), AccountantError>;
    async fn reset(&self, handle: &ExecutionHandle) -> Result<(), AccountantError>;
}
```

### OutputRouter — result distribution

```rust
#[async_trait]
pub trait OutputRouter: Debug + Send + Sync {
    async fn route(&self, result: ExecutionResult) -> Result<(), RouterError>;
    async fn add_sink(&self, pattern: RoutePattern, sink: Box<dyn OutputSink>) -> Result<(), RouterError>;
    async fn remove_sink(&self, id: &SinkId) -> Result<(), RouterError>;
    async fn list_sinks(&self) -> Result<Vec<SinkInfo>, RouterError>;
}
```

## Dependencies

- [Core](core.md) — `EventBus`, `Config`, `Logger`, `HealthMonitor`
- [Runtime](runtime.md) — `Scheduler`, `Supervisor`, `ResourceManager`
- [OSAL](osal.md) — `ProcessManager` (subprocess backend), `FileSystem` (output persistence)
- `wasmtime` (wasm backend, optional)
- `oci-spec` / `wasmtime-wasi` (container backend, optional)
- `tokio` (async I/O, pipes, timers)

## Lifecycle

1. **Init** — `CommandEngine` loads backend registry from `Config`. `TimeoutManager` reads default timeout. `ResourceAccountant` initializes counters. `OutputRouter` loads sink configuration.
2. **Start** — Backends are warmed (wasm engine precompilation, process pool allocation). `OutputRouter` opens configured sinks.
3. **Stop** — All active executions receive cancellation. Running subprocesses receive `SIGTERM` (with `SIGKILL` after grace). Wasm instances are dropped. Sinks are flushed and closed.

## Events Published

| Event | Payload | Description |
|-------|---------|-------------|
| `execution.started` | `{ handle, backend, command_summary }` | Command dispatched to a backend |
| `execution.completed` | `{ handle, exit_code, elapsed_ms, output_size }` | Command finished successfully |
| `execution.faulted` | `{ handle, error, stage }` | Command failed (timeout, OOM, backend crash) |
| `execution.cancelled` | `{ handle, reason }` | Command cancelled by caller or policy |
| `execution.resource_violation` | `{ handle, limit_type, actual, max }` | Resource limit exceeded |
| `execution.output_routed` | `{ handle, sink, destination }` | Output sent to a registered sink |

## Events Consumed

| Source | Event | Handling |
|--------|-------|----------|
| [Brain](brain.md) *(planned)* | `brain.decision_made` | `CommandEngine::execute` with the chosen action |
| [Runtime](runtime.md) | `runtime.resource_threshold` | `ResourceAccountant` adjusts limits, enforces backpressure |
| [Core](core.md) | `core.config_changed` | Reload default timeout, backend selection, sink config |
| [Core](core.md) | `core.service_stopped` | Cancel dependent in-flight executions |

## Error Model

| Error | Variant | Recovery |
|-------|---------|----------|
| `EngineError::NoSuitableBackend` | No backend supports the command | Return error, Brain may retry with different command |
| `EngineError::BackendUnavailable` | Selected backend unreachable | Fall back to next backend |
| `CaptureError::OutputTruncated` | Output exceeded `max_output` | Return partial output, set truncated flag |
| `TimeoutError::AlreadyExpired` | Deadline is in the past | Start with immediate cancellation |
| `AccountantError::LimitExceeded` | Resource budget exhausted | Cancel command, emit violation event |
| `RouterError::SinkFull` | Sink backpressure | Buffer with bounded queue, drop oldest |

## Performance Expectations

| Operation | Target Latency (p99) | Throughput |
|-----------|---------------------|------------|
| Command dispatch (noop) | < 50 µs | 20K+/s |
| Subprocess spawn + wait (empty) | < 1 ms | 1K+/s |
| Wasm instantiate + run (empty) | < 200 µs | 5K+/s |
| Output capture (1 KB) | < 5 µs | 200K+/s |
| Resource accounting update | < 2 µs | 500K+/s |
| Timeout check (per tick) | < 1 µs | 1M+/s |
| Output route (matched) | < 10 µs | 100K+/s |

## Thread Model

- `CommandEngine` holds a `HashMap<ExecutionBackend, Box<dyn BackendDriver>>` behind `Arc<RwLock>`. Backend selection is a read-locked match.
- Subprocess backend uses tokio `Command` — I/O is async, process wait is `spawn_blocking`.
- Wasm backend maintains a pre-initialized instance pool behind `Arc<Mutex<Vec<Instance>>>`.
- `OutputCapture` buffers are per-handle `Arc<Mutex<Vec<u8>>>` with a notification channel.
- `TimeoutManager` runs a tokio timer per active execution. Timer cancellation via `AbortHandle`.
- `ResourceAccountant` uses `AtomicU64` for counters where possible; `Arc<RwLock>` for aggregate session stats.
- `OutputRouter` sinks may be async — dispatch is fan-out over a tokio `JoinSet`.

## Portability

- `ExecutionBackend` is an enum — backends are selected by config, not by `#[cfg]`.
- Subprocess backend goes through [OSAL](osal.md) `ProcessManager`, never raw `libc`.
- Wasm backend uses `wasmtime` — it is not available on all platforms; graceful fallback is required.
- Container backend uses the OCI runtime spec — no Docker daemon dependency.
- No OS-specific types (`pid_t`, `uv_file`, `HANDLE`) appear in any trait signature.
- Output sinks (`OutputRouter`) are trait objects — file, network, and null sinks are all implementations.
