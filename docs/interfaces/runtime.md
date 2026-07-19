# Runtime Platform Interface — `runtime`

## Purpose

The Runtime layer provides the execution substrate for all user-space and OS-internal tasks. It owns the scheduler that multiplexes work onto tokio tasks, supervises child processes and wasm isolates, manages session scoping and resource budgets, and enforces permission checks. Every component in Layer 3 and above runs under Runtime's scheduling and supervision contracts.

## Public APIs

### Scheduler — task dispatch and priority management

```rust
#[async_trait]
pub trait Scheduler: Debug + Send + Sync {
    async fn spawn(&self, task: Task) -> Result<TaskHandle, SchedulerError>;
    async fn cancel(&self, handle: TaskHandle) -> Result<(), SchedulerError>;
    async fn priority(&self, handle: &TaskHandle) -> Priority;
    async fn set_priority(&self, handle: &TaskHandle, prio: Priority) -> Result<(), SchedulerError>;
    async fn stats(&self) -> SchedulerStats;
    fn events(&self) -> Box<dyn Stream<Item = SchedulerEvent> + Unpin + Send>;
}
```

### Supervisor — child process and isolate lifecycle

```rust
#[async_trait]
pub trait Supervisor: Debug + Send + Sync {
    async fn start(&self, spec: SupervisorSpec) -> Result<ChildHandle, SupervisorError>;
    async fn signal(&self, handle: &ChildHandle, sig: Signal) -> Result<(), SupervisorError>;
    async fn wait(&self, handle: ChildHandle) -> Result<ExitStatus, SupervisorError>;
    async fn restart(&self, handle: &ChildHandle) -> Result<(), SupervisorError>;
    async fn shutdown(&self, handle: &ChildHandle) -> Result<(), SupervisorError>;
}
```

### SessionManager — user session scoping and lifecycle

```rust
#[async_trait]
pub trait SessionManager: Debug + Send + Sync {
    async fn create(&self, spec: SessionSpec) -> Result<SessionId, SessionError>;
    async fn destroy(&self, id: SessionId) -> Result<(), SessionError>;
    async fn lookup(&self, token: &SessionToken) -> Result<SessionId, SessionError>;
    async fn active_sessions(&self) -> Result<Vec<SessionInfo>, SessionError>;
    fn events(&self) -> Box<dyn Stream<Item = SessionEvent> + Unpin + Send>;
}
```

### TaskManager — task metadata and querying

```rust
#[async_trait]
pub trait TaskManager: Debug + Send + Sync {
    async fn submit(&self, task: Task) -> Result<TaskHandle, TaskError>;
    async fn status(&self, handle: &TaskHandle) -> Result<TaskStatus, TaskError>;
    async fn list(&self, filter: TaskFilter) -> Result<Vec<TaskSummary>, TaskError>;
    async fn metrics(&self, handle: &TaskHandle) -> Result<TaskMetrics, TaskError>;
    fn stream(&self) -> Box<dyn Stream<Item = TaskEvent> + Unpin + Send>;
}
```

### ContextManager — capability and tracing context propagation

```rust
pub trait ContextManager: Debug + Send + Sync {
    fn current(&self) -> CapabilityContext;
    fn push_scope(&self, ctx: CapabilityContext) -> ScopeGuard;
    fn with_context<T>(&self, ctx: CapabilityContext, f: impl FnOnce() -> T) -> T;
    fn trace_id(&self) -> TraceId;
}
```

### StateMachine — generic hierarchical state machine

```rust
#[async_trait]
pub trait StateMachine<S: State>: Debug + Send + Sync {
    async fn transition(&self, target: S) -> Result<(), StateError>;
    fn current(&self) -> S;
    fn permitted(&self) -> Vec<S>;
    fn on_transition(&self, cb: Box<dyn Fn(S, S) + Send + Sync>);
}
```

### ResourceManager — quota tracking and enforcement

```rust
#[async_trait]
pub trait ResourceManager: Debug + Send + Sync {
    async fn acquire(&self, res: ResourceRequest) -> Result<ResourceLease, ResourceError>;
    async fn release(&self, lease: ResourceLease) -> Result<(), ResourceError>;
    async fn usage(&self, scope: &ScopeId) -> Result<ResourceUsage, ResourceError>;
    async fn set_limit(&self, scope: &ScopeId, limit: ResourceLimit) -> Result<(), ResourceError>;
    fn events(&self) -> Box<dyn Stream<Item = ResourceEvent> + Unpin + Send>;
}
```

### PermissionChecker — policy-based authorization

```rust
pub trait PermissionChecker: Debug + Send + Sync {
    fn check(&self, ctx: &CapabilityContext, action: &Action, target: &Target) -> Result<(), PermissionError>;
    fn check_many(&self, ctx: &CapabilityContext, requests: &[AccessRequest]) -> Result<Vec<AccessDecision>, PermissionError>;
    fn query(&self, ctx: &CapabilityContext) -> Result<Vec<Capability>, PermissionError>;
}
```

## Dependencies

- [Core](core.md) — `EventBus`, `Service`, `Logger`, `Config`, `HealthMonitor`
- `tokio` (task spawning, timeouts, channels)
- `wasmtime` (optional, wasm isolate support)

## Lifecycle

1. **Init** — `Scheduler` is created, backed by a tokio multi-threaded runtime. `ResourceManager` reads limits from `Config`. `PermissionChecker` loads policy.
2. **Start** — `Supervisor` begins polling child processes. `SessionManager` opens the session registry. `Scheduler` begins accepting `spawn` calls.
3. **Stop** — All tasks are drained (graceful with timeout, then hard cancel). Children receive `SIGTERM`, then `SIGKILL` after grace period. Sessions are flushed.

## Events Published

| Event | Payload | Description |
|-------|---------|-------------|
| `runtime.task_spawned` | `{ handle, priority, session }` | New task submitted to scheduler |
| `runtime.task_completed` | `{ handle, result, elapsed_ms }` | Task finished (success or error) |
| `runtime.task_faulted` | `{ handle, error, backtrace }` | Task panicked or exceeded budget |
| `runtime.session_created` | `{ session_id, user }` | New session opened |
| `runtime.session_destroyed` | `{ session_id, reason }` | Session closed |
| `runtime.resource_threshold` | `{ scope, resource, usage_pct }` | Resource usage exceeds 80% limit |
| `runtime.policy_denied` | `{ session, action, target }` | Permission check rejected |

## Events Consumed

| Source | Event | Handling |
|--------|-------|----------|
| [Core](core.md) | `core.config_changed` | `Scheduler` adjusts worker count, `ResourceManager` reloads limits |
| [Core](core.md) | `core.service_faulted` | `Supervisor` may restart dependent services |

## Error Model

| Error | Variant | Recovery |
|-------|---------|----------|
| `SchedulerError::QueueFull` | Backlog exceeded | Backpressure to caller |
| `SchedulerError::TaskPanicked` | Task unwound | Restart if supervised |
| `SupervisorError::CrashLoop` | Process restarts too fast | Backoff, emit alert |
| `ResourceError::BudgetExceeded` | No quota remaining | Block until release or timeout |
| `PermissionError::Denied` | Policy rejection | Log audit event, return to caller |
| `SessionError::Expired` | Token TTL elapsed | Force re-authentication |

## Performance Expectations

| Operation | Target Latency (p99) | Throughput |
|-----------|---------------------|------------|
| Task spawn (idle) | < 10 µs | 100K+/s |
| Task spawn (loaded) | < 50 µs | 50K+/s |
| Permission check (cached) | < 5 µs | 1M+/s |
| Permission check (policy eval) | < 1 ms | 10K+/s |
| Resource acquire (available) | < 2 µs | 500K+/s |
| State transition | < 1 µs | 1M+/s |

## Thread Model

- `Scheduler` uses a work-stealing thread pool. Task affinity is optional.
- Tokio's `spawn_blocking` is used for CPU-bound task phases.
- `PermissionChecker` maintains a read-optimized policy cache behind `Arc<RwLock>>`.
- `ResourceManager` uses lock-free atomics for counters; `Arc<RwLock>` for limit structs.
- `Supervisor` spawns one tokio task per supervised child for wait status polling.
- All public types are `Send + Sync`. Shared state is behind `Arc`.

## Portability

- `Supervisor` abstracts process creation behind an internal trait (`ProcessBackend`). The Linux backend uses `libc::fork`/`exec`. WASM backend uses `wasmtime`.
- `Scheduler` uses tokio exclusively — no platform-specific threading APIs.
- `PermissionChecker` policy is expressed in a platform-agnostic policy language (Kellog).
- Signals are mapped to an internal enum — raw `Signum` values never cross the trait boundary.
- File paths in `SupervisorSpec` are `PathBuf`, which is OS-portable.
