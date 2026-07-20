# RFC-0005: Execution Platform

| Field | Value |
|---|---|
| **Status** | Draft |
| **Author** | AI-OS Architecture Team |
| **Phase** | 8 |
| **Created** | 2026-07-20 |
| **Updated** | 2026-07-20 |
| **Requires** | RFC-0001 (OSAL), RFC-0002 (Memory), RFC-0003 (Brain), RFC-0004 (Perception) |
| **Supersedes** | None |

## Abstract

The Execution Platform provides AI-native OS with a structured execution substrate that translates Brain `ToolRequirement` objects into sandboxed, monitored, recoverable tool invocations. It implements a configurable pipeline: planning (candidate resolution, schema validation), dispatch (priority queuing, concurrency limiting, backend selection), execution (subprocess / WASM / container backends), monitoring (timeout, resource limits), result collection (output parsing, artifact attachment, enrichment), output routing (distribution to destinations), and recovery (retry with backoff, rollback via compensation actions, cancellation). The Execution Platform is the bridge between cognitive decisions and system actions.

## Motivation

Without an Execution Platform, the Brain Platform would either: (a) spawn system commands directly, violating clean architecture and creating unbounded security exposure; (b) delegate to external orchestration systems (Kubernetes, Nomad, systemd), introducing heavyweight dependencies and losing event-driven integration; or (c) execute tools inline in the Brain's reasoning loop, blocking cognitive processing during potentially long-running operations.

The Execution Platform provides:

1. **Unified execution model** — Every tool, from simple shell commands to WASM modules to OCI containers, is represented as a `ToolCandidate` with standardised input/output schemas, sandbox profiles, and lifecycle.
2. **Sandboxed isolation** — Every execution runs in an isolation context (process-level with seccomp, WASM with capability limiting, or container with OCI runtime) determined by the tool's `SandboxProfile`. Default deny on all permissions.
3. **Deterministic lifecycle** — Every execution follows a 14-state state machine (Planned → Queued → Dispatching → Running → Completed/Failed/TimedOut/Cancelled) with well-defined transitions, events at every transition, and recovery paths.
4. **Resource governance** — Every execution has a `ToolBudget` (deadline, max CPU, max memory, max output bytes) enforced by the Monitor. Violations trigger termination with preserved partial output.
5. **Recovery automation** — Failed executions can be retried with configurable backoff, rolled back via compensation action plans, or escalated for human intervention — all driven by registered `RecoveryPolicy`.
6. **Output routing** — Execution results are routed to configurable destinations (caller, Memory store, Brain reflection, chained execution, filesystem, EventBus) based on matching `RoutingRule` predicates.

### Specification Cross-References

- `specification.md` section 7 (AI Lifecycle): The Execution Platform implements the Act stage — tool invocation, output collection, and result routing.
- `specification.md` section 12 (Module Contracts): The Execution Platform follows all module contract requirements (Service trait, EventBus, HealthMonitor).
- `specification.md` section 4 (Security Goals): Sandbox isolation for every execution; all tool invocations are authorized; all execution events are audited.

## Design

### Layer Position

```
intelligence/                    (Layer 8 — Future)
  |
  v
execution/                       (Layer 7 — This RFC)
  |
  v
brain/                           (Layer 6 — Consumer of execution results)
  |
  v
perception/                      (Layer 5 — Observation enrichment for results)
  |
  v
memory/                          (Layer 4 — Result persistence, history replay)
  |
  v
runtime/                         (Layer 3 — Task scheduling, permissions, context)
  |
  v
core/                            (Layer 2 — EventBus, Service, Logger, Config)
  |
  v
system/                          (Layer 1 — OSAL: process, filesystem, network)
```

### Crate Structure

```
execution/                       (Layer 7 — This RFC)
  +-- execution-core/            Foundation types: ExecutionPlan, ToolCandidate, ToolBudget,
  |                                    ExecutionState, ExecutionId, ExecutionResult,
  |                                    ExecutionMetrics, Error types, Event types
  +-- execution-registry/        ToolRegistry: CRUD for registered tools,
  |                                    ToolResolver: capability-based candidate lookup,
  |                                    ToolBinding enum (Subprocess, Wasm, Container)
  +-- execution-planner/         ExecutionPlanner trait: requirement → plan,
  |                                    input schema validation, sandbox resolution,
  |                                    budget assembly, priority calculation
  +-- execution-dispatcher/      Dispatcher: priority queue, concurrency limiter,
  |                                    backend selector, sandbox creator
  +-- execution-runner/          Runner trait, sub-runners:
  |                                    SubprocessRunner, WasmRunner, ContainerRunner
  +-- execution-sandbox/         SandboxEnforcer: profile resolution, sandbox creation,
  |                                    isolation level enforcement
  +-- execution-monitor/         ExecutionMonitor: timeout enforcement, resource sampling,
  |                                    state watcher, metric accumulation
  +-- execution-results/         ResultCollector: output buffering, parse attempts,
  |                                    OutputParser, artifact collection, OutputRouter
  +-- execution-recovery/        RecoveryManager: retry policy evaluation, backoff computation,
  |                                    rollback plan execution, cancellation handler
  +-- execution-coordinator/     Top-level orchestration: pipeline wiring, lifecycle management,
  |                                    ExecutionHandle creation, EventBus integration,
  |                                    Service trait implementation
```

### Dependency Graph

```
execution-core (no deps within execution)
  ├── execution-registry (core, OSAL)
  ├── execution-planner (core, registry, sandbox, runtime)
  ├── execution-sandbox (core, OSAL)
  ├── execution-dispatcher (core, planner, sandbox, runtime)
  ├── execution-runner (core, OSAL, sandbox)
  │   ├── SubprocessRunner (core, OSAL ─ ProcessManager)
  │   ├── WasmRunner (core, wasmtime)
  │   └── ContainerRunner (core, OSAL ─ OciRuntime)
  ├── execution-monitor (core, runner, runtime)
  ├── execution-results (core, runner, monitor, memory, perception)
  ├── execution-recovery (core, results, dispatcher, memory)
   └── execution-coordinator (all execution crates, brain, runtime, memory, perception)
```

### Dependency Table

| Crate | Depends On | Dependency Type |
|---|---|---|
| `execution-core` | `ai-os-core`, `ai-os-runtime` | Workspace |
| `execution-registry` | `execution-core`, `ai-os-core`, `ai-os-system` | Workspace |
| `execution-planner` | `execution-core`, `execution-registry`, `execution-sandbox`, `ai-os-core` | Workspace |
| `execution-sandbox` | `execution-core`, `ai-os-core`, `ai-os-system` | Workspace |
| `execution-dispatcher` | `execution-core`, `execution-planner`, `execution-sandbox`, `ai-os-core`, `ai-os-runtime` | Workspace |
| `execution-runner` | `execution-core`, `ai-os-core`, `ai-os-system`, `wasmtime` (SubprocessRunner: `tokio::process`, ContainerRunner: `oci-spec` / `http`) | Workspace + External |
| `execution-monitor` | `execution-core`, `execution-runner`, `ai-os-core`, `ai-os-runtime` | Workspace |
| `execution-results` | `execution-core`, `execution-runner`, `execution-monitor`, `ai-os-core`, `ai-os-memory`, `ai-os-perception-core` | Workspace |
| `execution-recovery` | `execution-core`, `execution-results`, `execution-dispatcher`, `ai-os-core`, `ai-os-memory` | Workspace |
| `execution-coordinator` | All execution crates, `ai-os-brain-core`, `ai-os-core`, `ai-os-runtime` | Workspace |

### Execution State Machine

```
  ┌─────────────────────────────────────────────────────────────┐
  │                     Terminal States                          │
  │   ┌───────────┐  ┌───────────┐  ┌──────────┐  ┌──────────┐ │
  │   │Completed  │  │  Failed   │  │ TimedOut │  │Cancelled │ │
  │   └─────┬─────┘  └─────┬─────┘  └────┬─────┘  └────┬─────┘ │
  │         │              │              │              │       │
  └─────────┼──────────────┼──────────────┼──────────────┼───────┘
            │              │              │              │
            └──────────────┼──────────────┼──────────────┘
                           │              │
                           v              v
                     ┌──────────┐  ┌──────────┐
                     │  Failed  │  │ TimedOut │
                     └────┬─────┘  └────┬─────┘
                          │              │
                          v              v
                    ┌──────────────────────┐
                    │  Recovery Manager    │
                    │  retry?  rollback?   │
                    └──────┬───────┬───────┘
                           │       │
              retry ───────┘       └─────── no recovery needed
                           │
                           v
                     ┌──────────┐
                     │ Retrying │────→ Dispatcher (re-execution)
                     └──────────┘       │
                                        v
                                  ┌──────────┐
                                  │ Running  │
                                  └──────────┘

Main flow:

  Planned ──→ Queued ──→ Dispatching ──→ Running ──→ Collecting ──→ Routing ──→ Completed
                         │                │             │
                         v                v             v
                     Cancelled        TimedOut       Failed
                                      Cancelled

Recovery flow:

  Failed ──→ Retrying ──→ Queued (re-execution)
  TimedOut ──→ Retrying ──→ Queued (re-execution)
  Failed/TimedOut/Cancelled with rollback ──→ RollingBack ──→ RolledBack/Terminal
  Failed/TimedOut/Cancelled with escalate ───→ Escalated
```

| From | To | Trigger |
|---|---|---|
| `Planned` | `Queued` | Plan accepted by Dispatcher |
| `Queued` | `Dispatching` | Concurrency slot available |
| `Queued` | `Cancelled` | Cancel requested before dispatch |
| `Dispatching` | `Running` | Backend execution started |
| `Dispatching` | `Cancelled` | Cancel requested during dispatch |
| `Running` | `Collecting` | Process/instance exited normally |
| `Running` | `TimedOut` | Deadline exceeded |
| `Running` | `Cancelled` | Cancel requested during execution |
| `Running` | `Failed` | Non-zero exit / runtime error |
| `Collecting` | `Routing` | Output parsed and enriched |
| `Collecting` | `Failed` | Parse error / storage failure |
| `Routing` | `Completed` | All routes delivered |
| `Routing` | `Failed` | Non-retryable route failure |
| `Failed` | `Retrying` | Retry policy grants retry |
| `TimedOut` | `Retrying` | Retry policy grants retry |
| `Failed` | `RollingBack` | Rollback policy triggered |
| `TimedOut` | `RollingBack` | Rollback policy triggered |
| `Cancelled` | `RollingBack` | Rollback policy triggered |
| `Retrying` | `Queued` | Backoff elapsed, re-dispatched |
| `RollingBack` | `RolledBack` | All compensation steps succeeded |
| `RollingBack` | `Escalated` | Compensation step failed, escalate policy |
| `RollingBack` | `Terminal` | Compensation step failed, ignore policy |

### Execution Plan Model

```rust
pub struct ExecutionPlan {
    pub id: ExecutionId,
    pub requirement: ToolRequirement,
    pub candidate: ToolCandidate,
    pub binding: ToolBinding,
    pub sandbox_profile: SandboxProfile,
    pub budget: ToolBudget,
    pub routing_rules: Vec<RoutingRule>,
    pub rollback_plan: Option<RollbackPlan>,
    pub permissions: ExecutionPermissions,
    pub priority: Priority,
    pub created_at: Timestamp,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub retry_multiplier: f64,
}

pub struct ToolBudget {
    pub timeout_ms: u64,
    pub max_cpu_ms: u64,
    pub max_memory_bytes: u64,
    pub max_output_bytes: u64,
    pub cognitive_budget: Option<CognitiveBudget>,
}

pub enum ToolBinding {
    Subprocess {
        binary: PathBuf,
        args: Vec<String>,
        env: HashMap<String, String>,
        working_dir: Option<PathBuf>,
        allowed_paths: Vec<PathBuf>,
        denied_binaries: Vec<String>,
    },
    Wasm {
        module_path: PathBuf,
        function_name: String,
        input_format: WasmInputFormat,
        fuel_limit: Option<u64>,
        precompile: bool,
    },
    Container {
        image: String,
        command: Vec<String>,
        mounts: Vec<ContainerMount>,
        network: ContainerNetwork,
        pull_policy: PullPolicy,
        resource_limits: ContainerResources,
    },
}

pub enum ExecutionState {
    Planned,
    Queued,
    Dispatching,
    Running,
    Collecting,
    Routing,
    Completed,
    Failed,
    TimedOut,
    Cancelled,
    Retrying,
    RollingBack,
    RolledBack,
    Escalated,
}

pub struct ExecutionHandle {
    pub id: ExecutionId,
    pub state_rx: watch::Receiver<ExecutionState>,
    pub cancel_tx: tokio::sync::oneshot::Sender<CancelReason>,
    pub result_rx: tokio::sync::oneshot::Receiver<ExecutionResult>,
}
```

### Events

#### Published Events

| Event Type | Payload | Description |
|---|---|---|
| `execution.plan_created` | `ExecutionPlanSummary` | A valid plan was created from a ToolRequirement |
| `execution.plan_rejected` | `PlanRejectedPayload` | Planning failed (no candidate, schema invalid) |
| `execution.queued` | `ExecutionQueuedPayload` | Plan entered dispatch queue |
| `execution.dispatched` | `ExecutionDispatchedPayload` | Plan dispatched to runner |
| `execution.started` | `ExecutionStartedPayload` | Tool execution started in backend |
| `execution.completed` | `ExecutionCompletedPayload` | Tool exited with zero status |
| `execution.failed` | `ExecutionFailedPayload` | Tool exited with non-zero status or runtime error |
| `execution.timed_out` | `ExecutionTimedOutPayload` | Deadline exceeded, execution terminated |
| `execution.cancelled` | `ExecutionCancelledPayload` | Execution cancelled by request |
| `execution.resource_exceeded` | `ResourceExceededPayload` | CPU/memory/output limit exceeded |
| `execution.result_routed` | `ResultRoutedPayload` | Result delivered to a destination |
| `execution.result_route_failed` | `RouteFailedPayload` | Result delivery to a destination failed |
| `execution.retrying` | `ExecutionRetryingPayload` | Failed execution scheduled for retry |
| `execution.rolling_back` | `ExecutionRollingBackPayload` | Rollback plan execution started |
| `execution.rolled_back` | `ExecutionRolledBackPayload` | Rollback completed successfully |
| `execution.escalated` | `ExecutionEscalatedPayload` | Execution escalated for human intervention |
| `execution.tool_registered` | `ToolRegistrationPayload` | New tool registered in registry |
| `execution.tool_unregistered` | `ToolUnregistrationPayload` | Tool removed from registry |
| `execution.backpressure_warning` | `BackpressurePayload` | Dispatch queue approaching capacity |
| `execution.bypass_engaged` | `BypassPayload` | A stage entered bypass mode |
| `execution.bypass_disengaged` | `BypassPayload` | A stage exited bypass mode |
| `execution.coordinator_ready` | `CoordinatorStatusPayload` | Coordinator initialized and ready |

#### Consumed Events

| Event Type | Source | Handler |
|---|---|---|
| `brain.decision.made` | Brain Platform | `ExecutionCoordinator::handle_brain_decision` — creates ExecutionPlan from ToolRequirement |
| `brain.decision.escalated` | Brain Platform | `ExecutionCoordinator::handle_escalated_decision` — high-risk execution with extra audit |
| `runtime.session_closed` | Runtime Platform | `ExecutionCoordinator::handle_session_close` — cancel all active executions for session |
| `runtime.permission_changed` | Runtime Platform | `RecoveryManager::recheck_permissions` — re-evaluate retry/rollback policies |
| `memory.snapshot_created` | Memory Platform | `RecoveryManager::register_snapshot` — register compensation snapshot reference |
| `core.lifecycle.stopping` | Core Platform | `ExecutionCoordinator::prepare_shutdown` — graceful drain of active executions |
| `core.config.reloaded` | Core Platform | `ExecutionCoordinator::reload_config` — hot-reload configuration |

### Configuration

The Execution Platform is configured via TOML under the `[execution]` section of `platform.toml`. Full specification is in `docs/configuration/execution.md`. Key sections:

```toml
[execution]
max_concurrency = 10
default_timeout_ms = 30_000
max_output_bytes = 10485760
channel_capacity = 10000
backpressure_threshold = 0.8

[execution.pipeline]
planner_enabled = true
dispatcher_enabled = true
monitor_enabled = true
results_enabled = true
recovery_enabled = true
stage_bypass_threshold = 5
stage_bypass_window_secs = 60

[execution.dispatch]
default_priority = 50
per_backend_concurrency.subprocess = 10
per_backend_concurrency.wasm = 5
per_backend_concurrency.container = 3

[execution.sandbox]
default_isolation = "process"
profiles.none = { isolation = "none", allowed_syscalls = "*" }
profiles.read_only = { isolation = "process", allowed_paths = ["/usr/bin/*"] }

[execution.backends.subprocess]
enabled = true
allowed_binaries = ["/usr/bin/*"]
denied_binaries = ["/usr/bin/su", "/usr/bin/sudo"]
default_working_dir = "/tmp/ai-os/exec"

[execution.backends.wasm]
enabled = false
max_memory_pages = 256
fuel_limit = 100000

[execution.backends.container]
enabled = false
default_runtime = "runc"
pull_policy = "if_not_present"

[execution.registry.tools]
example_grep = { binding = { type = "subprocess", binary = "/usr/bin/grep" }, capabilities = ["text.search"], input_schema = {}, output_schema = {}, sandbox = "read_only" }

[execution.routing.defaults]
rules = [
  { condition = "state == Completed", destinations = ["caller", "memory", "brain"] },
  { condition = "state == Failed", destinations = ["log", "memory"] },
]

[execution.recovery]
default_max_retries = 3
default_retry_delay_ms = 1000
default_retry_multiplier = 2.0
default_rollback_on_cancel = false
default_escalate_on_failure = true
```

### Thread Model

| Component | Concurrency Model |
|---|---|
| `ExecutionPlanner` | Synchronous on Coordinator's `brain.decision.made` handler |
| `Dispatcher` | Priority queue behind `RwLock`. Queue waiters via `tokio::sync::Notify`. Dispatch on coordinator task. Per-backend semaphore (`tokio::sync::Semaphore`). |
| `SubprocessRunner` | Per-execution `tokio::task` for child process wait. Separate tasks for async stdout/stderr reads. Child I/O via `tokio::io::AsyncRead`. |
| `WasmRunner` | WASM compilation on `tokio::task::spawn_blocking`. Instance execution on async task with `wasmtime` async support. Fuel metering on each wasm instruction. |
| `ContainerRunner` | Async HTTP requests to OCI runtime socket. Container health polling on `tokio::time::interval`. Streaming stdout/stderr via OCI attach API. |
| `ExecutionMonitor` | Per-execution Tokio timer (`tokio::time::sleep`). Resource sampling on `tokio::time::interval`. State stored in `RwLock<HashMap<ExecutionId, ExecutionWatch>>`. |
| `ResultCollector` | Output buffering in `BytesMut` per execution. Parse dispatch on `spawn_blocking` for expensive parsers. |
| `OutputRouter` | Route evaluation synchronously. Route delivery via async per-destination handlers. |
| `RecoveryManager` | Retry backoff via `tokio::time::sleep`. Rollback action dispatch through the pipeline (re-entrant). State in `RwLock<HashMap<ExecutionId, RecoveryState>>`. |
| `ExecutionCoordinator` | Single coordinator task (or sharded pool). Pipeline stages connected via bounded `mpsc` channels. Stage client traits behind `Arc<dyn Trait>`. |

### Lifecycle

#### Init Phase
1. Load configuration from platform config.
2. Initialize ToolRegistry (load built-in and configured tools).
3. Initialize SandboxEnforcer (load sandbox profiles).
4. Create bounded `mpsc` channels between pipeline stages.
5. Initialize each stage's state (empty queues, empty watch maps).
6. Register EventBus subscriptions.

#### Start Phase
1. Start the Coordinator task (EventBus listener loop).
2. Each pipeline stage waits for input on its channel.
3. Publish `execution.coordinator_ready` event.
4. Health check passes when coordinator task is running and channels are open.

#### Stop Phase
1. Pause EventBus subscription (stop accepting new ToolRequirements).
2. Cancel all queued plans (mark as Cancelled, notify Brain).
3. Graceful drain of running executions (send SIGTERM, wait grace period, then SIGKILL).
4. Complete result collection and routing for drained executions.
5. Flush and close `mpsc` channels.
6. Publish `execution.coordinator_stopped`.

### Error Handling

| Error Category | Variants | Severity | Recovery |
|---|---|---|---|
| **Plan errors** | `NoSuitableCandidate`, `SchemaValidationFailed`, `SandboxResolutionFailed`, `BudgetExceeded` | Error | Non-retryable. Return to Brain for replanning. |
| **Dispatch errors** | `QueueFull`, `ConcurrencyLimitReached`, `BackendUnavailable`, `BackendFallbackFailed` | Warning | Transient. Retry with backoff (dispatch level). |
| **Sandbox errors** | `SandboxCreationFailed`, `IsolationUnsupported`, `ProfileNotFound` | Error | Non-retryable (config error). Escalate. |
| **Execution errors** | `ProcessSpawnFailed`, `InvalidExitCode`, `SignalTerminated`, `ExecutionPanic`, `WasmTrap`, `ContainerCreateFailed`, `ContainerStartFailed` | Error | Retryable per policy. Retry budget decremented. |
| **Monitor errors** | `TimeoutExceeded`, `CpuLimitExceeded`, `MemoryLimitExceeded`, `OutputLimitExceeded` | Warning | Terminal. Partial output preserved. Retryable per policy (different budget may succeed). |
| **Result errors** | `OutputParseFailed`, `OutputTooLarge`, `ArtifactCollectionFailed`, `EnrichmentFailed` | Warning | Non-retryable. Raw output preserved. Route may succeed with unparsed data. |
| **Route errors** | `DestinationUnavailable`, `RoutePermissionDenied`, `RouteStoreFailed`, `RoutePipeFailed` | Warning | Transient: retry once. Permanent: skip route, log failure. |
| **Recovery errors** | `RetryBudgetExhausted`, `RollbackStepFailed`, `RollbackRetryExhausted`, `CancellationFailed`, `CompensationToolFailed` | Error | Escalate on exhaustion. Log on ignore policy. |
| **Coordinator errors** | `ChannelClosed`, `StagePanic`, `BypassTriggered`, `LifecycleConflict` | Critical | Restart stage. If coordinator panics, supervisor restarts full service. |

#### Retry Policy

```
retry_allowed(state, policy, attempt) -> bool {
    if attempt >= policy.max_retries { return false }
    match state {
        Failed => policy.on_failure.retry(),
        TimedOut => policy.on_timeout.retry(),
        Cancelled => policy.on_cancellation.retry(),
        _ => false,
    }
}

backoff(attempt, base_ms, multiplier) -> Duration {
    let jitter = rand::thread_rng().gen_range(0.0..=0.5);
    Duration::from_millis((base_ms * multiplier.powf(attempt as f64) * (1.0 + jitter)) as u64)
}
```

### Security Considerations

1. **Default deny** — No tool may execute unless explicitly registered with a `ToolBinding` and `SandboxProfile`. Unregistered capabilities produce `NoSuitableCandidate`.
2. **Sandbox isolation** — Every execution runs in a sandbox determined by the tool's profile. Three isolation levels with increasing restriction: WASM (capability-limited, fuel-metered), Process (seccomp, dedicated user), Container (OCI, read-only rootfs, no networking by default).
3. **Permission check** — Before dispatch, the `ExecutionContext` (session, identity, permissions) is validated against the `ExecutionPermissions` required by the tool. Permission denials produce `PermissionDenied` and are logged as security events.
4. **Input validation** — All `ToolRequirement.input_params` are validated against the candidate's `input_schema` (JSON Schema). Schema violations are rejected before any system action occurs.
5. **Denied binaries** — Subprocess backend has configurable `denied_binaries` and `allowed_binaries` globs. Binaries matching deny patterns or not matching allow patterns are rejected.
6. **Audit trail** — Every state transition, permission check, and route delivery is published as an EventBus event and persisted in Memory (episodic store). The full execution history is replayable.
7. **Command injection prevention** — Subprocess backend uses structured args (`Vec<String>`), never shell string interpolation. WASM backend has no shell access. Container backend uses the OCI runtime's exec API, never shell invocation.

### Performance Considerations

1. **Throughput targets**:
   - Subprocess execution: <50ms overhead (plan → dispatch → spawn).
   - WASM execution: <5ms overhead (compilation cached, instance creation amortized).
   - Container execution: <500ms overhead (image pull excluded, cached images only).
   - Pipeline overhead: <2ms per non-execution stage.
   - Result routing: <1ms per route (synchronous destinations).

2. **Concurrency**:
   - Default max 10 concurrent executions across all backends.
   - Per-backend limits: subprocess 10, WASM 5, container 3.
   - Queue capacity: 10,000 plans.
   - Backpressure warning published at 80% queue capacity.

3. **Resource limits**:
   - Default timeout: 30 seconds.
   - Default max output: 10 MB.
   - Default max memory: 512 MB (subprocess), 256 WASM pages (WASM), 512 MB (container).
   - WASM fuel limit: 100,000 instructions.

4. **Scalability**:
   - The coordinator runs as a single task (or sharded pool for multi-tenant deployments).
   - Each runner operates independently. No shared state between runner instances.
   - Output parsing uses `spawn_blocking` for serde-heavy parsers.
   - The dispatcher's priority queue operations are O(log n).

### Testing Strategy

1. **Unit tests** — Every state machine transition, every error variant, every policy evaluation. Property-based tests (proptest) for priority queue ordering, backoff computation, state machine reachability.

2. **Integration tests**:
   - SubprocessRunner: spawn `/bin/echo`, `/bin/true`, `/bin/false`, verify output collection, exit code, resource metrics.
   - WasmRunner: execute a WASM module that returns arithmetic results, verify output parsing.
   - Dispatcher: submit 20 equal-priority plans, verify max 10 concurrent, remaining queued.
   - Monitor: submit a plan with 10ms timeout, verify `TimedOut` state and partial output preservation.
   - Recovery: submit a plan that always fails, verify retry up to `max_retries`, then escalate.
   - Recovery: submit a plan with a rollback plan, cancel it, verify rollback steps execute in order.
   - OutputRouter: verify result routed to caller, memory, and brain simultaneously.

3. **Chaos tests**:
   - Random stage failure: close an `mpsc` channel mid-pipeline, verify bypass engages.
   - Coordinator restart during active execution, verify execution state recovery.
   - Backend crash: kill a subprocess externally, verify correct state transition and retry.

4. **Benchmarks**:
   - Pipeline end-to-end throughput: plans/second at varying concurrency levels.
   - WASM vs subprocess vs container latency comparison.
   - Priority queue enqueue/dequeue performance at 10K items.

## Drawbacks

1. **Backend diversity** — Three backends (subprocess, WASM, container) triple the maintenance surface. Each backend has distinct failure modes, resource accounting quirks, and security profiles. Mitigation: all backends implement the same `Runner` trait; backend-specific logic is confined to the runner crate.
2. **Pipeline latency** — The staged pipeline adds ~5ms overhead per execution for non-execution stages. For sub-millisecond tools, this is significant. Mitigation: bypass mode allows per-stage skipping. For latency-critical tools, a direct execution path via `ExecutionCoordinator::execute_direct()` bypasses the pipeline.
3. **Rollback complexity** — Compensation-based rollback (execute inverse actions, restore snapshots, delete created files) can itself fail, leading to partial rollback and escalation. This is inherent to distributed compensation patterns and is handled by the OnRollbackFailure policy (escalate, ignore, retry).
4. **Memory pressure** — Each execution holds output buffers in memory until routing completes. With max 10 concurrent executions and 10 MB max output, potential memory usage is 100 MB + overhead. Mitigation: output buffering is bounded by `max_output_bytes` per execution; streaming to disk is planned for a future RFC.

## Alternatives Considered

| Alternative | Reason for Rejection |
|---|---|
| Monolithic `execution` crate with internal modules | Violates single-responsibility principle; no independent testability of pipeline stages; harder to evolve individual backends. |
| Direct library calls (no subprocess/WASM/container) | No isolation. Any tool crash would bring down the platform. Must support arbitrary language tools (Python, JS, shell scripts, binaries). |
| External task queue (RabbitMQ, Redis Queue) | Introduces heavyweight external dependencies, adds serialization round-trips, and loses EventBus integration. Bounded in-process channels are simpler and sufficient. |
| Kubernetes Job API | Heavyweight dependency for a single-node OS. Container support is provided via OCI runtime API without requiring an orchestration layer. |
| Push-based recovery (always retry immediately) | Without backoff, repeated failures can saturate the pipeline. Configurable backoff with jitter is standard practice for resilient systems. |

## Open Questions

1. Should the Execution Platform support pluggable execution backends via a trait registration system (similar to ToolRegistry)? Current design hard-codes three backends. A backend registration API could be added in a future RFC.
2. How should chained execution (output of one tool → input of another) be represented in the plan model? Should chaining be a first-class concept (DAG of steps) or handled via the OutputRouter's pipe destination? The pipe destination handles basic chaining; a DAG-based workflow model could be added if needed.
3. Should WASM execution support the Component Model for cross-language tool authoring? Current design targets core WASM with WASI preview 2. Component Model support can be added when the Wasmtime ecosystem stabilizes.
4. Should the coordinator support multi-tenant isolation (separate dispatch queues per session/identity)? Current design uses a shared priority queue with per-plan priority. Per-tenant queues could prevent one tenant from starving another.

## Implementation Plan

1. **Phase 1 — Core types** (effort: medium)
   - `execution-core`: `ExecutionPlan`, `ToolCandidate`, `ToolBinding`, `ToolBudget`, `ExecutionState`, `ExecutionId`, `ExecutionResult`, `ExecutionMetrics`, error types, event types
   - All types must round-trip through Serde
   - State machine implementation with `derive_more` or manual transition validation
   - Estimated: 2-3 sprints

2. **Phase 2 — Registry + Sandbox** (effort: medium)
   - `execution-registry`: ToolRegistry trait, InMemoryToolRegistry, ToolResolver, capability-based lookup, CRUD operations
   - `execution-sandbox`: SandboxEnforcer, SandboxProfile, isolation level handling
   - Integration tests with mock OSAL process management
   - Estimated: 2-3 sprints

3. **Phase 3 — Planner + Dispatcher** (effort: medium)
   - `execution-planner`: ExecutionPlanner trait, schema validation, budget assembly, priority calculation
   - `execution-dispatcher`: priority queue (BinaryHeap), concurrency limiter (Semaphore), backend selector, sandbox creator
   - Property-based tests for queue ordering
   - Estimated: 2-3 sprints

4. **Phase 4 — Runner + Monitor** (effort: large)
   - `execution-runner`: Runner trait, SubprocessRunner (tokio::process), WasmRunner (wasmtime), ContainerRunner (OCI HTTP API)
   - `execution-monitor`: timeout enforcement (tokio timer), resource sampling (OSAL ProcessManager queries), state tracking
   - Chaos tests for backend crashes
   - Estimated: 4-5 sprints

5. **Phase 5 — Results + Recovery** (effort: large)
   - `execution-results`: ResultCollector, OutputParser (JSON/YAML/CSV), OutputRouter, artifact collection
   - `execution-recovery`: RecoveryManager, retry policy evaluation, backoff computation, rollback plan execution, cancellation handler
   - Integration tests with Memory Platform for result persistence
   - Estimated: 3-4 sprints

6. **Phase 6 — Coordinator** (effort: medium)
   - `execution-coordinator`: Pipeline assembly (mpsc channel wiring), Service trait, EventBus subscriptions, ExecutionHandle creation, drain/shutdown logic
   - Stage bypass mode with automatic recovery
   - Integration with Brain Platform (ToolRequirement → ExecutionResult)
   - Chaos tests for stage failure recovery
   - Estimated: 2-3 sprints

## Unresolved Topics

- **Streaming output** — Some tools produce infinite output streams (tail -f, long-running servers). Current design assumes finite output (collect → route → complete). Streaming execution (results streamed as they arrive, without waiting for completion) will be addressed in a future RFC.
- **Execution history retention** — How long to retain ExecutionHistory in Memory for audit/replay. Configurable TTL with ring buffer storage will be addressed in a future RFC.
- **Cross-node execution** — In a multi-node deployment, an execution may need to run on a remote node. This requires distributed coordination and result forwarding, deferred to the Intelligence Integration phase.
- **Plugin execution backends** — Support for custom backends (e.g., Python sub-interpreter, Node.js worker thread) via a `Backend` trait. Deferred until demand for non-standard backends arises.

## References

- [Execution Platform Architecture](../architecture/execution.md)
- [Execution Platform Interfaces](../interfaces/execution.md)
- [Execution Platform Configuration](../configuration/execution.md)
- [Execution Pipeline Design](../execution-pipeline.md)
- [Brain Platform Architecture](../architecture/brain.md)
- [Memory Platform Architecture](../architecture/memory.md)
- [Perception Platform Architecture](../architecture/perception.md)
- [OSAL Architecture](../architecture/osal.md)
- [Core Platform Architecture](../architecture/core.md)
- [Runtime Platform Architecture](../architecture/runtime.md)
- RFC-0001: System Platform
- RFC-0002: Memory Platform
- RFC-0003: Brain Platform
- RFC-0004: Perception Platform
