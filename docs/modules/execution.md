# Execution Platform Module

**Module:** `ai_os_execution`
**Status:** Planned (Phase 8)
**Crate:** Not yet created

## Purpose

The Execution Platform module is the action arm of the AI-native OS. It takes commands produced by the Brain and runs them in sandboxed environments (subprocess, WASM, container), captures structured output, enforces timeouts, tracks resource consumption, and routes results to downstream consumers. Execution is the final link in the cognitive chain: perception -> reasoning -> execution -> memory.

## Status Overview

| Subsystem | Design | Implementation | Tests |
|---|---|---|---|
| CommandEngine | Draft | Not started | Not started |
| OutputCapture | Draft | Not started | Not started |
| TimeoutManager | Draft | Not started | Not started |
| ResourceAccountant | Draft | Not started | Not started |
| OutputRouter | Draft | Not started | Not started |

## Public Interfaces

### `CommandEngine`

Planned trait for multi-backend command execution.

```rust
/// Planned for Phase 8
#[async_trait]
pub trait CommandBackend: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, command: &CommandSpec) -> Result<ExecutionHandle, ExecutionError>;
    async fn cancel(&self, handle: &ExecutionHandle) -> Result<(), ExecutionError>;
}

/// Planned for Phase 8
pub struct CommandSpec {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,
    pub stdin: Option<Vec<u8>>,
    pub timeout: Duration,
    pub max_output_bytes: u64,
}

/// Planned for Phase 8
pub struct ExecutionHandle {
    pub id: ExecutionId,
    pub backend: String,
    pub pid: Option<u32>,
    pub started_at: SystemTime,
}

/// Planned for Phase 8
pub struct CommandEngine {
    backends: HashMap<String, Arc<dyn CommandBackend>>,
    default_backend: String,
}

impl CommandEngine {
    pub fn new(default_backend: &str) -> Self;
    pub fn register_backend(&mut self, name: &str, backend: Arc<dyn CommandBackend>);

    /// Execute on the default backend.
    pub async fn execute(&self, spec: CommandSpec) -> Result<ExecutionHandle, ExecutionError>;

    /// Execute on a named backend.
    pub async fn execute_on(&self, backend: &str, spec: CommandSpec) -> Result<ExecutionHandle, ExecutionError>;

    /// Cancel a running execution.
    pub async fn cancel(&self, handle: &ExecutionHandle) -> Result<(), ExecutionError>;
}
```

Three planned backends:

1. **Subprocess** (`tokio::process::Command`): spawns native processes with pipe-connected stdio.
2. **WASM** (`wasmtime`): executes WebAssembly modules in a sandboxed, cross-platform runtime.
3. **Container** (`bollard`): creates and runs ephemeral Docker containers for maximum isolation.

Backend selection is automatic based on `CommandSpec` metadata (field `runtime_hint: Option<String>`) or explicit caller choice.

### `OutputCapture`

Planned struct for streaming and parsing command output.

```rust
/// Planned for Phase 8
pub struct OutputCapture {
    stdout_tx: tokio::sync::mpsc::Sender<OutputChunk>,
    stderr_tx: tokio::sync::mpsc::Sender<OutputChunk>,
    structured_parser: Option<Arc<dyn StructuredParser>>,
}

/// Planned for Phase 8
pub struct OutputChunk {
    pub stream: StreamKind,    // Stdout | Stderr
    pub data: Vec<u8>,
    pub offset: u64,
    pub timestamp: SystemTime,
}

/// Planned for Phase 8
pub enum StreamKind {
    Stdout,
    Stderr,
}

/// Planned for Phase 8
#[async_trait]
pub trait StructuredParser: Send + Sync {
    fn mime_types(&self) -> Vec<&str>;
    async fn parse(&self, data: &[u8]) -> Result<ParsedOutput, ExecutionError>;
}

/// Planned for Phase 8
pub enum ParsedOutput {
    Json(serde_json::Value),
    Yaml(serde_yaml::Value),
    Table(Vec<Vec<String>>),
    Raw(Vec<u8>),
}

impl OutputCapture {
    pub fn new(capacity: usize) -> (Self, tokio::sync::mpsc::Receiver<OutputChunk>);

    /// Attach a structured output parser (e.g., JSON, YAML).
    pub fn with_parser(mut self, parser: Arc<dyn StructuredParser>) -> Self;

    /// Feed bytes from a process stdout/stderr.
    pub async fn feed(&self, stream: StreamKind, data: &[u8]) -> Result<(), ExecutionError>;

    /// Signal that the stream is complete.
    pub async fn close(&self);

    /// Collect all output into a single buffer (for short-lived commands).
    pub async fn collect(self) -> Result<CommandOutput, ExecutionError>;
}

/// Planned for Phase 8
pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub parsed: Option<ParsedOutput>,
    pub exit_code: Option<i32>,
    pub duration: Duration,
}
```

### `TimeoutManager`

Planned struct for per-command deadline enforcement with grace period escalation.

```rust
/// Planned for Phase 8
pub struct TimeoutManager {
    timeouts: RwLock<HashMap<ExecutionId, TimeoutState>>,
    config: TimeoutConfig,
}

/// Planned for Phase 8
pub struct TimeoutConfig {
    pub default_timeout: Duration,
    pub grace_period: Duration,       // SIGTERM -> SIGKILL delay
    pub max_timeout: Duration,
}

/// Planned for Phase 8
enum TimeoutState {
    Watching { deadline: Instant, handle: ExecutionHandle },
    Grace { deadline: Instant, handle: ExecutionHandle },
    Expired,
}

impl TimeoutManager {
    pub fn new(config: TimeoutConfig) -> Self;

    /// Start watching an execution. Cancels if deadline passes.
    pub async fn watch(&self, handle: ExecutionHandle, timeout: Duration) -> Result<(), ExecutionError>;

    /// Extend the deadline for an execution in progress.
    pub async fn extend(&self, id: &ExecutionId, extra: Duration) -> Result<(), ExecutionError>;

    /// Cancel a watch (called when execution completes naturally).
    pub async fn unwatch(&self, id: &ExecutionId);
}
```

On timeout:
1. Send `SIGTERM` to the process (or invoke backend cancel).
2. Start the grace period timer.
3. If the process has not exited after `grace_period`, escalate to `SIGKILL`.

### `ResourceAccountant`

Planned struct for tracking command resource usage via `wait4` and `/proc`.

```rust
/// Planned for Phase 8
pub struct ResourceAccountant {
    accounts: RwLock<HashMap<ExecutionId, ResourceUsage>>,
    limits: ResourceLimits,
}

/// Planned for Phase 8
pub struct ResourceUsage {
    pub cpu_time_ms: u64,
    pub max_rss_kb: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub voluntary_context_switches: u64,
    pub involuntary_context_switches: u64,
}

impl ResourceAccountant {
    pub fn new(limits: ResourceLimits) -> Self;

    /// Register an execution for accounting.
    pub fn register(&self, id: ExecutionId) -> Result<(), ExecutionError>;

    /// Record usage after process exit (reads from `wait4` or `/proc/<pid>/stat`).
    pub async fn record(&self, id: &ExecutionId, pid: u32) -> Result<ResourceUsage, ExecutionError>;

    /// Get the recorded usage for a completed execution.
    pub fn get(&self, id: &ExecutionId) -> Option<ResourceUsage>;

    /// Check if usage exceeds limits.
    pub fn exceeds_limits(&self, usage: &ResourceUsage) -> bool;
}
```

On Linux, CPU time and RSS are collected from `libc::wait4` (or `libc::getrusage` for the child). I/O bytes are read from `/proc/<pid>/io`. Context switches come from `/proc/<pid>/status`.

### `OutputRouter`

Planned struct for piping command output to consumers.

```rust
/// Planned for Phase 8
pub struct OutputRouter {
    routes: Vec<Route>,
    bus: Arc<EventBus>,
}

/// Planned for Phase 8
pub enum RouteDestination {
    /// Pipe to the next command in a pipeline.
    NextCommand(Box<CommandSpec>),
    /// Store in memory (ShortTermMemory or LongTermStorage).
    Memory(StorageTarget),
    /// Publish to EventBus (callbacks).
    EventBus(String),    // event type name
    /// Write to file.
    File(PathBuf),
    /// Fan-out to multiple destinations.
    FanOut(Vec<RouteDestination>),
}

/// Planned for Phase 8
pub struct Route {
    pub source: ExecutionId,
    pub destination: RouteDestination,
    pub transform: Option<Arc<dyn OutputTransform>>,
}

/// Planned for Phase 8
#[async_trait]
pub trait OutputTransform: Send + Sync {
    async fn transform(&self, output: &CommandOutput) -> Result<Vec<u8>, ExecutionError>;
}

impl OutputRouter {
    pub fn new(bus: Arc<EventBus>) -> Self;

    /// Add a route from an execution to a destination.
    pub fn add_route(&mut self, route: Route);

    /// Route the output. May spawn new executions for pipeline steps.
    pub async fn route(&self, id: &ExecutionId, output: CommandOutput) -> Result<(), ExecutionError>;

    /// Remove all routes for an execution.
    pub fn remove_routes(&mut self, id: &ExecutionId);
}
```

Pipeline example: `CommandEngine::execute("cat /etc/hosts")` -> `OutputRouter` routes stdout to `NextCommand("grep localhost")` -> route output to `Memory(Episodic)`.

## Dependencies

| Dependency | Scope | Purpose |
|---|---|---|
| `ai_os_core` | internal | EventBus, Service, Container |
| `ai_os_memory` | internal | Storing command output to long-term memory |
| `ai_os_runtime` | internal | PermissionChecker, TaskManager integration |
| `tokio` | runtime | Async process management, timers |
| `tokio::process` | runtime | Subprocess backend |
| `wasmtime` | external | WASM execution backend |
| `bollard` | external | Docker/OCI container backend |
| `serde_json` | external | Structured output parsing |
| `serde_yaml` | external | Structured output parsing |
| `libc` | external | `wait4`, `getrusage` for resource accounting |

## Events Published

| Event | Trigger | Payload |
|---|---|---|
| `execution::CommandStarted` | `CommandEngine::execute` | `(ExecutionId, String)` -- id, command |
| `execution::CommandCompleted` | Command exits successfully | `(ExecutionId, CommandOutput)` |
| `execution::CommandFailed` | Command exits with non-zero | `(ExecutionId, i32, String)` -- id, code, stderr tail |
| `execution::CommandTimedOut` | TimeoutManager fires | `(ExecutionId, Duration)` |
| `execution::CommandCancelled` | `CommandEngine::cancel` | `ExecutionId` |
| `execution::OutputRouted` | `OutputRouter::route` | `(ExecutionId, RouteDestination)` |
| `execution::ResourceLimitExceeded` | ResourceAccountant detects overage | `(ExecutionId, ResourceUsage)` |

## Events Consumed

| Event | Consumer | Action |
|---|---|---|
| `brain::PlanProduced` | `CommandEngine` | Execute steps from the plan |
| `runtime::TaskCancelled` | `CommandEngine` | Propagate cancellation to running commands |
| `system::ResourceExhausted` | `ResourceAccountant` | Pre-emptively pause low-priority commands |
| `perception::InputEvent` | `CommandEngine` | Feed stdin to a waiting command |

## Thread Model

- `CommandEngine` is `Send + Sync`; its backend map is read-only after registration.
- `OutputCapture` uses `tokio::sync::mpsc` channels for streaming. Each command gets its own sender/receiver pair.
- `TimeoutManager` holds state behind a `RwLock`. The watch method spawns a `tokio::spawn` task that `select!`s between completion and a `tokio::time::sleep`.
- `ResourceAccountant` uses an `RwLock<HashMap>`. Writes happen synchronously after `wait4`.
- `OutputRouter` stores routes in a `RwLock<Vec<Route>>`; routing itself is an async operation that may spawn new commands.

## Lifecycle

```
Per-command lifecycle:
  Created -> Submitted -> Running -> Completed
                              | -> Failed (non-zero exit)
                              | -> TimedOut
                              | -> Cancelled

Resource accounting:
  register() on submit -> record() on completion

Timeout watch:
  watch() on start -> unwatch() on completion
                   -> timeout -> SIGTERM -> grace -> SIGKILL

Output routing:
  route() after completion or failure -> fan-out to destinations
```

Commands are submitted through the `CommandEngine`, which selects a backend, spawns the execution, registers it with `TimeoutManager` and `ResourceAccountant`, attaches `OutputCapture`, and returns an `ExecutionHandle`. The caller may wait on the handle or attach an `OutputRouter` for pipeline-style processing.

## Error Handling

Error type: `ai_os_execution::ExecutionError` with variants:

- `BackendNotFound(String)` -- requested backend is not registered.
- `SpawnError(String)` -- backend failed to spawn the command.
- `NonZeroExit(i32, String)` -- command exited with non-zero status; includes stderr tail.
- `TimeoutExpired(Duration)` -- command exceeded its timeout.
- `Cancelled` -- command was explicitly cancelled.
- `OutputParseError(String)` -- structured parser could not parse output.
- `ResourceExceeded(ResourceUsage)` -- post-execution check shows limits were breached.
- `RouteError(String)` -- output router destination unreachable.

Recovery: `TimeoutExpired` leaves the process in a `Grace` state; the caller may extend the deadline or confirm the kill. `NonZeroExit` is not an error per se -- some commands legitimately exit non-zero. The caller decides whether to retry. `ResourceExceeded` is logged but does not retroactively change the outcome.

## Configuration

```toml
[engine]
default_backend = "subprocess"

[timeout]
default_secs = 30
grace_secs = 5
max_secs = 3600

[resources]
max_cpu_ms = 60_000
max_memory_kb = 524_288        # 512 MB
max_io_bytes = 104_857_600     # 100 MB

[output]
max_bytes = 10_485_760         # 10 MB per command
auto_parse_json = true
auto_parse_yaml = true
```

Configuration file: `aios-execution.toml`.

## Testing Strategy

- **Unit tests**: OutputCapture chunk reassembly (verify chunks arrive in order and complete). TimeoutManager grace period escalation. ResourceAccountant `saturating_add` correctness.
- **Integration tests**: Subprocess backend: spawn `echo hello`, verify stdout captured. Non-zero exit: spawn `false`, verify `NonZeroExit`. Timeout: spawn `sleep 60` with 1 s timeout, verify timeout and SIGKILL. Pipeline: route `echo '{"a":1}'` through JSON parser, verify `ParsedOutput::Json`.
- **Contract tests**: For each backend, verify that `cancel` works (process tree is cleaned up). Verify `max_output_bytes` truncation.
- **Benchmarks**: Subprocess spawn latency (cold start). WASM instantiation time vs. subprocess. OutputCapture throughput with 100 MB stream.

## Future Extensions

- Execution verdict cache: if the same command with the same inputs was run recently, return the cached output (idempotency).
- Distributed execution: offload commands to remote workers via gRPC (for long-running or resource-intensive tasks).
- Incremental output delivery: push live streaming output to subscribers via WebSocket.
- Sandbox profiles: pre-configured seccomp, AppArmor, and capability sets per backend.
- Execution graph: DAG-based execution where outputs of one command fan-out to multiple downstream commands.
- Checkpoint and resume: for very long commands, periodically save state and allow resume after failure.
