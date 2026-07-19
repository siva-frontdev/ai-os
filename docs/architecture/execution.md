# Execution Platform Blueprint

## Purpose

The Execution Platform (Phase 8, Planned) is the action layer of AI-native OS. It receives tasks from the Brain Platform and the Runtime Platform and executes them in controlled, sandboxed environments -- subprocesses, WASM sandboxes, or containers. It captures output as structured data, enforces timeouts and cancellation, accounts for resource consumption, and routes results to the next stage in a pipeline or back to storage. The Execution Platform is the point where plans become side effects on the system.

The Execution Platform builds on the [Runtime Platform](runtime.md) (scheduling, supervision, resource management) and the [System Platform](system.md) (process management, filesystem, networking). It feeds results back to the [Brain Platform](brain.md) for evaluation and to the [Memory Platform](memory.md) for persistence.

---

## Responsibilities

- **Command Execution Engine**: Execute commands in multiple runtime environments: native subprocesses (via `tokio::process`), WASM sandboxes (via `wasmtime`), and OCI containers (via direct OCI runtime integration or Podman REST API). Abstract each execution environment behind a common `Executor` trait.
- **Output Capture**: Stream stdout and stderr in real time. Parse structured output formats (JSON, YAML, CSV) into typed data. Collect exit codes, signals, and termination reasons. Support binary output capture for non-text payloads.
- **Timeout and Cancellation**: Enforce per-execution timeout with configurable grace period for cleanup. Support cancellation via Runtime task cancellation propagation. Kill process trees on timeout or cancellation. Publish timeout and cancellation events.
- **Resource Accounting**: Track CPU time, peak memory, I/O bytes, and wall-clock duration per execution. Report resource usage on completion. Feed data to Runtime ResourceManager for aggregation and limit enforcement.
- **Output Routing**: Route execution output to configurable destinations: return to caller (inline), write to Memory Platform, pipe to next execution stage, write to filesystem (via System FileSystemProvider), or broadcast to multiple consumers via EventBus.

---

## Public Interfaces

### Executor (`execution::executor`)

```rust
pub enum ExecutionEnvironment {
    Subprocess { binary: String, args: Vec<String>, env: HashMap<String, String> },
    Wasm { module_path: String, function: String, allow_net: bool, allow_fs: bool },
    Container { image: String, command: Vec<String>, mounts: Vec<Mount>, network: NetworkConfig },
}

pub struct ExecutionConfig {
    pub environment: ExecutionEnvironment,
    pub working_dir: Option<String>,
    pub timeout: Duration,
    pub grace_period: Duration,
    pub max_output_bytes: u64,
    pub capture_stdout: bool,
    pub capture_stderr: bool,
    pub parse_output: Option<OutputFormat>,
    pub resource_limits: ResourceLimits,
}

pub enum OutputFormat { Json, Yaml, Csv, Raw, Binary }

pub struct ExecutionHandle {
    pub execution_id: Uuid,
    pub task_id: Uuid,
    pub environment: ExecutionEnvironment,
    pub started_at: DateTime<Utc>,
    pub status_rx: watch::Receiver<ExecutionStatus>,
}

pub enum ExecutionStatus {
    Pending,
    Running { pid: Option<u32> },
    Completed { result: ExecutionResult },
    Failed { error: ExecutionError },
    TimedOut,
    Cancelled,
}

pub trait Executor: Debug + Send + Sync {
    async fn execute(&self, config: ExecutionConfig) -> Result<ExecutionHandle, ExecutionError>;
    async fn cancel(&self, execution_id: Uuid) -> Result<(), ExecutionError>;
    async fn status(&self, execution_id: Uuid) -> Result<ExecutionStatus, ExecutionError>;
}
```

### OutputCapture (`execution::output`)

```rust
pub struct OutputStream {
    pub stream_type: StreamType,
    pub data: Vec<u8>,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
}

pub enum StreamType { Stdout, Stderr, Combined }

pub struct ParsedOutput {
    pub format: OutputFormat,
    pub raw: Vec<u8>,
    pub parsed: serde_json::Value,
    pub schema_valid: bool,
}

pub trait OutputCapture: Debug + Send + Sync {
    async fn capture(&self, execution_id: Uuid, stream: OutputStream) -> Result<(), ExecutionError>;
    async fn flush(&self, execution_id: Uuid) -> Result<CapturedOutput, ExecutionError>;
    async fn parse(&self, execution_id: Uuid, format: OutputFormat) -> Result<ParsedOutput, ExecutionError>;
    fn subscribe(&self, execution_id: Uuid) -> Result<mpsc::Receiver<OutputStream>, ExecutionError>;
}
```

### ResourceAccounting (`execution::accounting`)

```rust
pub struct ExecutionResources {
    pub execution_id: Uuid,
    pub cpu_time_ms: u64,
    pub peak_memory_bytes: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub wall_clock_ms: u64,
    pub network_bytes_in: u64,
    pub network_bytes_out: u64,
}

pub trait ResourceAccounting: Debug + Send + Sync {
    async fn record_usage(&self, execution_id: Uuid, resources: ExecutionResources) -> Result<(), ExecutionError>;
    async fn get_usage(&self, execution_id: Uuid) -> Result<ExecutionResources, ExecutionError>;
    async fn total_usage(&self, session_id: Uuid) -> Result<ExecutionResources, ExecutionError>;
    async fn report(&self, execution_id: Uuid) -> Result<ResourceReport, ExecutionError>;
}
```

### OutputRouter (`execution::router`)

```rust
pub enum RouteDestination {
    Caller,
    MemoryStore { ttl: Option<Duration> },
    Pipe { next_execution_id: Uuid },
    FileSystem { path: String },
    EventBus { event_type: String },
    Broadcast { destinations: Vec<RouteDestination> },
}

pub struct RoutingRule {
    pub name: String,
    pub condition: RoutingCondition,
    pub destination: RouteDestination,
}

pub enum RoutingCondition {
    Always,
    OnSuccess,
    OnFailure,
    OnExitCode { code: u32 },
    OnOutputMatch { pattern: String },
}

pub trait OutputRouter: Debug + Send + Sync {
    async fn register_rule(&self, rule: RoutingRule) -> Result<(), ExecutionError>;
    async fn route(&self, execution_id: Uuid, output: &CapturedOutput) -> Result<Vec<RouteResult>, ExecutionError>;
    fn list_rules(&self) -> Vec<RoutingRule>;
}
```

### ExecutionPlatform (`execution::ExecutionPlatform`)

```rust
pub struct ExecutionPlatform {
    pub executor: Arc<dyn Executor>,
    pub output_capture: Arc<dyn OutputCapture>,
    pub resource_accounting: Arc<dyn ResourceAccounting>,
    pub output_router: Arc<dyn OutputRouter>,
}
```

`ExecutionPlatform` implements Core's `Service` trait. Its `start()` method initializes the WASM runtime engine (if configured) and validates container runtime connectivity.

---

## Dependencies

| Crate | Purpose | Subsystem |
|---|---|---|
| `ai-os-core` | EventBus, Service, Logger, LifecycleManager | All subsystems |
| `ai-os-runtime` | Scheduler, TaskManager, ResourceManager, ContextManager | Executor (task mapping), ResourceAccounting |
| `ai-os-system` | ProcessManager, FileSystemProvider | Subprocess executor, filesystem output |
| `ai-os-memory` | MemoryStore for output persistence | OutputRouter (MemoryStore destination) |
| `tokio` | Async runtime, process management, `mpsc` for streams | Executor, OutputCapture |
| `wasmtime` | WASM sandbox execution engine | Wasm executor |
| `podman-api` / `oci-runtime` | Container execution via Podman REST API or direct OCI | Container executor |
| `serde` / `serde_json` | Structured output parsing, event serialization | OutputCapture, OutputRouter |
| `serde_yaml` | YAML output parsing | OutputCapture |
| `csv` | CSV output parsing | OutputCapture |
| `nix` | Process group management, resource usage queries | ResourceAccounting |
| `chrono` | Timestamps for execution lifecycle | All subsystems |
| `uuid` | Execution and task identifiers | All subsystems |
| `thiserror` | Error type derivation | All subsystems |

---

## Events Published

| Event | Type String | Subsystem | Trigger |
|---|---|---|---|
| `ExecutionStarted` | `execution.started` | Executor | New execution launched |
| `ExecutionCompleted` | `execution.completed` | Executor | Execution finishes with exit code 0 |
| `ExecutionFailed` | `execution.failed` | Executor | Execution finishes with non-zero exit or error |
| `ExecutionTimedOut` | `execution.timed_out` | Executor | Wall-clock timeout exceeded |
| `ExecutionCancelled` | `execution.cancelled` | Executor | Execution cancelled via Runtime task cancellation |
| `OutputCaptured` | `execution.output.captured` | OutputCapture | Output chunk received |
| `OutputParsed` | `execution.output.parsed` | OutputCapture | Structured output parsed successfully |
| `OutputParseFailed` | `execution.output.parse_failed` | OutputCapture | Structured output parsing error |
| `OutputRouted` | `execution.output.routed` | OutputRouter | Output delivered to destination |
| `OutputRouteFailed` | `execution.output.route_failed` | OutputRouter | Output delivery to destination failed |
| `ResourcesRecorded` | `execution.resources.recorded` | ResourceAccounting | Resource usage snapshot captured |

---

## Events Consumed

| Event | Source | Consumer | Purpose |
|---|---|---|---|
| `runtime.task_cancelled` | TaskManager | Executor | Cancel associated execution |
| `runtime.task_failed` | TaskManager | Executor | Mark execution as failed |
| `runtime.session_destroyed` | SessionManager | Executor | Cancel all session-scoped executions |
| `brain.plan.step_started` | Brain Reasoner | Executor | Trigger execution for plan step |
| `brain.plan.step_completed` | Brain Reasoner | OutputRouter | Route output based on evaluation result |

The Execution Platform is driven by Brain Platform plan steps and Runtime Platform task lifecycle events. When a plan step starts, the Execution Platform receives the execution request (via direct method call through the Runtime task, not through the EventBus -- execution is a synchronous action within a task). The EventBus events above are for observability and cross-cutting concerns like cancellation.

---

## Thread Model

| Subsystem | Threading |
|---|---|
| `SubprocessExecutor` | Each subprocess runs as a Tokio child process task. `tokio::process::Command` spawn manages the child. Awaits exit status on the child's task. |
| `WasmExecutor` | WASM compilation on `spawn_blocking`. Instance execution on the caller's async task. WASM instances are not `Send`; each execution gets a fresh instance. |
| `ContainerExecutor` | HTTP calls to Podman API via `reqwest` on the caller's task. Container health polled on a Tokio interval task. |
| `OutputCapture` | Per-execution `mpsc::Receiver<OutputStream>` fed by the child process's stdout/stderr pipes. A dedicated Tokio task reads pipe fds and pushes to the channel. |
| `ResourceAccounting` | `RwLock<HashMap<Uuid, ExecutionResources>>`. CPU/memory read during execution via `wait4` / `rusage` on the child wait task. |
| `OutputRouter` | `RwLock<Vec<RoutingRule>>` for rule registry. Routing evaluation on the caller's task. I/O-bound destinations (MemoryStore, FileSystem) use async calls. |

The critical resource constraint is the subprocess `wait` task: each running child process occupies a Tokio task until completion. The `max_concurrent` configuration limits the number of simultaneous subprocesses to prevent task starvation. WASM and container executions share this limit pool.

---

## Lifecycle

`ExecutionPlatform` implements Core's `Service` trait:

1. **Construction**: `ExecutionPlatform::new()` creates executor, output capture, resource accounting, and routing subsystems. No external connections are opened.
2. **Start**: `start()` validates the WASM runtime engine is functional (if enabled), tests connectivity to the container runtime (if configured), pre-compiles any registered WASM modules, and registers event subscriptions for task cancellation.
3. **Running**: The platform accepts execution requests. Each execution creates a Runtime task, spawns the appropriate executor, streams output, and publishes completion events. Output routing runs synchronously after completion.
4. **Stop**: `stop()` sends SIGTERM to all running subprocesses, drains the output pipelines, waits for in-flight WASM executions with a configurable timeout (default: 30 seconds), kills remaining processes with SIGKILL after grace period, and unregisters event subscriptions.

Start order within the Execution Platform:

```
1. ResourceAccounting  (stateless, initialized first)
2. OutputCapture       (stream infrastructure ready)
3. OutputRouter        (routing rules loaded from config)
4. Executor            (subprocess, WASM, container engines initialized last)
```

---

## Error Handling

`ExecutionError` is the unified error type:

| Variant | Subsystem | Condition |
|---|---|---|
| `SubprocessSpawnFailed(String)` | SubprocessExecutor | `fork`/`exec` failure (binary not found, permission denied) |
| `SubprocessKillFailed(String)` | SubprocessExecutor | Unable to terminate process tree |
| `WasmCompilationFailed(String)` | WasmExecutor | WASM module validation or compilation error |
| `WasmExecutionFailed(String)` | WasmExecutor | WASM trap or host function error |
| `ContainerPullFailed(String)` | ContainerExecutor | Image pull failure |
| `ContainerStartFailed(String)` | ContainerExecutor | Container create or start failure |
| `TimeoutExceeded { execution_id, timeout }` | Executor | Wall-clock duration exceeded configured timeout |
| `OutputBufferExceeded { execution_id, max_bytes }` | OutputCapture | Captured output exceeds `max_output_bytes` |
| `OutputParseError(String)` | OutputCapture | Structured output malformed (invalid JSON, YAML, CSV) |
| `ResourceCollectionFailed(String)` | ResourceAccounting | `wait4` / `rusage` syscall failure |
| `RoutingError(String)` | OutputRouter | Destination unreachable or write failure |
| `MaxConcurrencyReached` | Executor | At capacity, execution queued or rejected |
| `Core(CoreError)` | All subsystems | Error from Core EventBus or Logger |
| `Runtime(RuntimeError)` | Executor | Error from Runtime TaskManager |
| `System(SystemError)` | SubprocessExecutor | Error from System ProcessManager |

**Recovery strategies**:

- `SubprocessSpawnFailed` is retried once after a 1-second delay (transient resource exhaustion). If the binary is missing, the error is permanent and published for operator action.
- `TimeoutExceeded` sends SIGTERM to the process group, waits the grace period, then sends SIGKILL. The partial output captured before timeout is preserved and routed with a warning tag.
- `OutputParseError` falls back to raw binary output. The `parsed` field is set to `null` and `schema_valid` is `false`. The raw output is preserved for alternative parsing.
- `MaxConcurrencyReached` publishes a backpressure event. The caller should retry after the backoff interval indicated in the event payload. The Brain Platform's DecisionMaker may re-rank or delay the execution.
- `ContainerPullFailed` retries with a different registry mirror if configured. After 3 failures, the execution is marked failed and the error details are stored in Memory for diagnostic analysis.

---

## Data Flow: Execution Lifecycle

```
Brain Reasoner          Executor              OutputCapture         ResourceAccounting
    |                       |                       |                       |
    | 1. step action        |                       |                       |
    |--- (Runtime task) --->|                       |                       |
    |                       | 2. create handle      |                       |
    |                       | 3. spawn process      |                       |
    |                       | 4. publish Started    |                       |
    |                       |                       |                       |
    |                       | 5. stdout/stderr      |                       |
    |                       |--- (mpsc stream) ---->|                       |
    |                       |                       | 6. buffer chunks      |
    |                       |                       | 7. publish Captured   |
    |                       |                       |                       |
    |                       | 8. process exits      |                       |
    |                       | 9. collect rusage     |                       |
    |                       |--------------------------------------------->|
    |                       |                       |                       |
    |                       | 10. flush output      |                       |
    |                       |---------------------->|                       |
    |                       |   CapturedOutput      |                       |
    |                       |<----------------------|                       |
    |                       |                       |                       |
    |                       | 11. parse output      |                       |
    |                       |---------------------->|                       |
    |                       |   ParsedOutput        |                       |
    |                       |<----------------------|                       |
    |                       |                       |                       |
    |                       | 12. publish Completed |                       |
    |                       | 13. route output      |                       |
    |<--- ExecutionResult --|                       |                       |
```

---

## Future Extensions

- **Interactive Executions**: Support executions that maintain a bidirectional I/O stream (stdin/stdout) for interactive sessions, with flow control and keepalive.
- **Execution Templates**: Define reusable execution templates (parameterized commands, preset resource limits, standard routing rules) registered at startup and referenced by name.
- **Result Caching**: Cache execution results keyed by command + input hash. Return cached results for identical executions within a configurable TTL, avoiding redundant computation.
- **Distributed Execution**: Forward executions to remote Execution Platform instances via a gRPC bridge, enabling a cluster of execution workers behind a local scheduler.
- **Execution Checkpointing**: Support pause/snapshot/resume for long-running executions using CRIU (Checkpoint/Restore In Userspace) for subprocesses and WASM state serialization.
- **Foreign Function Interface**: Allow WASM executions to import host functions from a curated registry (file I/O, network, crypto) with explicit capability declarations in the WASM module manifest.
- **Pipeline Composition**: Define a DSL for composing multiple executions into a DAG pipeline, where each stage's output is automatically routed to the next stage's input.
- **Execution Provenance**: Record the complete provenance chain (which plan, goal, intent, and input led to this execution) as structured metadata on every output, enabling full audit trails.

---

## References

- [Architecture Overview](overview.md)
- [Core Platform Deep Dive](core.md)
- [Runtime Platform Deep Dive](runtime.md)
- [System Platform Blueprint](system.md)
- [Memory Platform Blueprint](memory.md)
- [Brain Platform Blueprint](brain.md)
- [Perception Platform Blueprint](perception.md)
- [Design Principles](../principles.md)
- [Roadmap](../roadmap.md)
- WASMtime: https://wasmtime.dev/
- Podman REST API: https://docs.podman.io/en/latest/_static/api.html
