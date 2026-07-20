# Execution Platform Architecture

## Purpose

The Execution Platform (Phase 8, Layer 7) is the motor cortex of AI-native OS. It translates abstract `ToolRequirement` objects produced by the Brain Platform's Planner into real executable actions — subprocesses, WASM functions, or container invocations — while enforcing permissions, safety policies, retry strategies, time budgets, and result collection.

The Execution Platform sits between the **Brain Platform** (above, Layer 6) and the **OSAL** (below, Layer 1). It depends on **Runtime** for task scheduling and supervision, on **Memory** for execution history and artifact persistence, on **Perception** for observation enrichment of execution outputs, and on **Core** for EventBus, Service lifecycle, logging, and configuration.

This is where plans become side effects on the system.

---

## Design Principles

1. **Tool Abstraction** — The Brain never constructs OS commands. It emits `ToolRequirement` objects that declare capability needs (e.g., `"read_file"`, `"list_processes"`). The Execution Platform matches requirements to registered `ToolCandidate` implementations and resolves the actual execution environment.

2. **Sandbox by Default** — Every execution runs in a sandbox with configurable isolation boundaries. No execution has ambient OS access. Capabilities are explicitly granted per execution.

3. **Fail-Observably** — Every execution phase emits typed events. Failures capture full context: the tool, the input, the error, the stage. Nothing is silently swallowed.

4. **Controlled Concurrency** — A configurable concurrency limit prevents OS resource exhaustion. Executions beyond the limit are queued and dispatched as capacity frees.

5. **Result Routing as Policy** — Execution results are not hard-coded to a single consumer. Routing rules (to Memory, to the Brain, to another execution, to a log sink) are loaded from configuration and evaluated per-result.

6. **Recoverable by Design** — Every execution carries a retry policy, a rollback procedure, and a timeout. The RecoveryManager handles transient failures without coordinator involvement.

7. **Budget-Driven** — Every execution receives an `ExecutionBudget` that constrains wall-clock time, CPU time, memory, output size, and retry count. Budgets cascade from the Brain's `CognitiveBudget`.

---

## Crate Structure

```
perception/                   (Layer 5 — Downward dependency for observation enrichment)
memory/                       (Layer 4 — Execution history, artifact storage)
brain/                        (Layer 6 — Consumer of ToolRequirement, producer of ToolCandidate)
  |
  v
execution/                    (Layer 7 — This layer)
├── execution-core/           Foundation types, errors, events, base traits
├── execution-registry/       ToolCandidate registry, ToolResolver, capability matching
├── execution-planner/        Execution plan generation from ToolRequirements
├── execution-dispatcher/     Dispatch queuing, backend selection, concurrency limiting
├── execution-runner/         Subprocess, WASM, and container execution backends
├── execution-sandbox/        Sandbox policy, isolation profiles, security contexts
├── execution-monitor/        Runtime monitoring, timeout enforcement, resource tracking
├── execution-results/        Result collection, parsing, routing, artifact management
├── execution-recovery/       Retry policies, rollback execution, recovery strategies
└── execution-coordinator/    Pipeline assembly, lifecycle, orchestration, health
  |
  v
runtime/                      (Layer 3 — Task scheduling, supervision, session management)
crates/ (OSAL)                (Layer 1 — Process, filesystem, network, terminal)
core/                         (Layer 2 — EventBus, Service, Logger, Config)
```

### Dependency Graph

```
execution-core (no deps within execution)
├── execution-registry (core)
├── execution-planner (core, registry)
├── execution-sandbox (core)
├── execution-runner (core, sandbox, OSAL)
├── execution-dispatcher (core, registry, runner)
├── execution-monitor (core, runner, Runtime)
├── execution-results (core, runner, Memory)
├── execution-recovery (core, monitor, results)
└── execution-coordinator (ALL execution crates, Runtime, Memory, Perception, Brain)
```

External dependencies:

| Crate | Depends On | Purpose |
|---|---|---|
| `execution-core` | `ai-os-core`, `memory-core` | EventBus, Service, Timestamp, MemoryId, Serde |
| `execution-registry` | `execution-core` | ToolCandidate registration and lookup |
| `execution-planner` | `execution-core`, `execution-registry` | Translate ToolRequirement into ExecutionPlan |
| `execution-sandbox` | `execution-core` | Sandbox profiles, isolation policies |
| `execution-runner` | `execution-core`, `execution-sandbox`, `osal-core`, `osal-process`, `osal-filesystem`, `osal-network`, `osal-terminal` | Execution in subprocess/WASM/container |
| `execution-dispatcher` | `execution-core`, `execution-registry`, `execution-runner`, `ai-os-runtime` | Concurrency control, backend selection |
| `execution-monitor` | `execution-core`, `execution-runner`, `ai-os-runtime` | Timeout, resource tracking, health |
| `execution-results` | `execution-core`, `execution-runner`, `memory-core` | Output parsing, routing, artifact storage |
| `execution-recovery` | `execution-core`, `execution-monitor`, `execution-results` | Retry, rollback, recovery orchestration |
| `execution-coordinator` | All execution crates, `ai-os-runtime`, `memory-core`, `perception-core`, `brain-core` | Pipeline assembly, lifecycle, health |

---

## Crate Responsibilities

### execution-core

**Foundation crate.** Defines all shared types, error variants, event types, and base traits that every other execution crate depends on.

**Key types:**
- `ExecutionId` — UUID v7, globally unique execution identifier
- `ExecutionRequest` — Input payload: tool, parameters, budget, permissions, sandbox profile
- `ExecutionHandle` — Opaque handle returned on dispatch: id, status watcher, cancellation sender
- `ExecutionBudget` — Time, CPU, memory, output size, retry count limits
- `ExecutionState` — Finite state machine (14 states, see below)
- `ExecutionResult` — Exit code, stdout, stderr, artifacts, resource usage, error info
- `ExecutionArtifact` — Named byte payload produced by execution (files, data blobs)
- `ExecutionMetrics` — CPU time, peak memory, I/O bytes, wall-clock, network bytes
- `ExecutionHistory` — Immutable record of a completed execution for storage in Memory
- `ToolRequirement` — Capability declaration: tool ID, input schema, output expectations
- `ToolCapabilityId` — Namespaced identifier for tool capabilities (e.g., `"fs.read"`)
- `ExecutionError` — Unified error enum with 40+ variants

**Key traits:**
- `Executable` — Trait for anything that can be executed (ToolCandidate, ExecutionPlan)

### execution-registry

**Tool and capability registry.** Manages the set of registered `ToolCandidate` implementations that the platform can execute. Provides capability-based lookup.

**Key types:**
- `ToolCandidate` — A concrete, registered tool: capability, name, version, runner binding, input schema, side-effect declarations
- `ToolBinding` — Maps a `ToolCandidate` to a specific execution backend (subprocess binary, WASM module, container image)
- `ToolRegistry` — Thread-safe registry of all available tools
- `ToolResolver` — Interface for resolving `ToolRequirement` to `ToolCandidate`

**Key traits:**
- `ToolRegistry` — Register, query, list, remove tool candidates
- `ToolResolver` — Resolve requirement to candidate by capability match

### execution-planner

**Execution plan generation.** Takes a `ToolRequirement` from the Brain, resolves it via the `ToolResolver` into a `ToolCandidate`, validates inputs against the candidate's schema, and produces an `ExecutionPlan` — a ready-to-dispatch execution specification.

**Key types:**
- `ExecutionPlan` — Validated, resolved execution ready for dispatch: candidate, input parameters, sandbox profile, budget, routing rules
- `PlanValidator` — Validates that inputs conform to the candidate's input schema

**Key traits:**
- `ExecutionPlanner` — Accept `ToolRequirement`, produce `ExecutionPlan`

### execution-dispatcher

**Dispatch queue and backend selection.** Accepts `ExecutionPlan` objects, applies concurrency limiting, selects the appropriate execution backend, and dispatches to the runner. When concurrency limits are reached, plans are queued and dispatched when capacity becomes available.

**Key types:**
- `DispatchQueue` — Priority-ordered queue of pending executions
- `BackendSelector` — Selects execution backend based on ToolBinding and current load
- `DispatcherConfig` — Max concurrency, queue capacity, priority levels
- `DispatchStatus` — Pending, Queued, Dispatching, Dispatched, Rejected

**Key traits:**
- `Dispatcher` — Submit plan, cancel pending, status query, queue inspection

### execution-runner

**Execution backends.** Runs `ToolCandidate` implementations in their configured execution environment. Three backends are defined; each is an optional feature.

**Key types:**
- `ExecutionEnvironment` — Subprocess (binary+args), Wasm (module+function), Container (image+command)
- `RunnerHandle` — Typed handle per backend that manages the running process lifecycle
- `RunnerConfig` — Per-backend configuration: binary path, WASM engine settings, container runtime endpoint

**Key traits:**
- `Runner` — Start, cancel, stream output, collect result

**Default implementations:**
- `SubprocessRunner` — Spawns OS processes via OSAL ProcessManager, manages stdin/stdout/stderr pipes, collects exit code and resource usage
- `WasmRunner` — Instantiates WASM modules via wasmtime runtime, calls exported functions, captures return values
- `ContainerRunner` — Delegates to OCI runtime via OSAL container interface, manages container lifecycle

### execution-sandbox

**Sandbox policy and isolation.** Defines sandbox profiles that constrain what an execution can do. Profiles are applied before dispatch and enforced by the runner.

**Key types:**
- `SandboxProfile` — Named isolation profile: filesystem access, network access, process spawning, capability restrictions
- `SandboxPolicy` — Policy that maps `ToolCapabilityId` to the minimum required sandbox profile
- `IsolationLevel` — None, Process, Container, Wasm (in increasing isolation)
- `FilesystemAccess` — Read-only, ReadWrite, TempOnly, None
- `NetworkAccess` — None, OutboundOnly, InboundOnly, Full

**Key traits:**
- `SandboxEnforcer` — Resolve the required sandbox profile for a given tool + execution context

### execution-monitor

**Runtime monitoring and enforcement.** Tracks all running executions, enforces timeouts, monitors resource consumption, and provides health probes.

**Key types:**
- `ExecutionWatch` — Per-execution monitoring state: timeout deadline, resource accumulator, cancellation flag
- `MonitorConfig` — Health check interval, timeout grace period, resource sampling interval
- `ResourceSnapshot` — Point-in-time resource usage: CPU%, memory, I/O

**Key traits:**
- `ExecutionMonitor` — Register watch, update resources, cancel, health check

### execution-results

**Result collection, parsing, enrichment, and routing.** Collects stdout/stderr from completed executions, parses structured output, attaches artifacts, enriches with context, and routes results to configured destinations.

**Key types:**
- `ResultCollector` — Aggregates output streams, parses structured output, produces `ExecutionResult`
- `OutputParser` — Parses stdout bytes into structured data (JSON, YAML, CSV, Raw)
- `RoutingRule` — Condition + destination mapping for result delivery
- `RouteDestination` — Memory, Brain, Pipe, FileSystem, EventBus, Caller
- `ExecutionArtifact` — Named output artifact with content type and byte payload

**Key traits:**
- `ResultCollector` — Collect, parse, produce ExecutionResult
- `OutputRouter` — Register routing rules, route results

### execution-recovery

**Retry, rollback, and recovery orchestration.** Watches for execution failures and applies configured recovery strategies: retry with backoff, rollback with compensation, or escalate to operator.

**Key types:**
- `RecoveryStrategy` — Retry, Rollback, Fail, Escalate
- `RetryPolicy` — Max retries, backoff multiplier, max backoff duration, jitter
- `RollbackPlan` — Sequence of compensation actions to undo an execution
- `RecoveryState` — Idle, Retrying, RollingBack, Escalated, Resolved

**Key traits:**
- `RecoveryManager` — Register execution for recovery, notify of failure, execute recovery

### execution-coordinator

**Orchestrator.** Assembles the execution pipeline, manages lifecycle (start/stop/reload), monitors component health, handles reconfiguration, and provides the bridge between the Brain Platform and the execution pipeline.

**Key types:**
- `CoordinatorConfig` — Pipeline assembly parameters, stage ordering, channel sizes
- `PipelineStatus` — Running state, per-stage health, throughput metrics
- `ExecutionSession` — Scoped execution context binding a Brain request to a pipeline flow

**Key traits:**
- `ExecutionCoordinator` — Submit ToolRequirement, cancel execution, status query, lifecycle
- `PipelineManager` — Stage registration, channel wiring, bypass mode

---

## Data Model

### ExecutionRequest — input from Brain to Execution

```rust
pub struct ExecutionRequest {
    pub execution_id: ExecutionId,
    pub requirement: ToolRequirement,
    pub input_params: HashMap<String, serde_json::Value>,
    pub budget: ExecutionBudget,
    pub sandbox_profile: Option<SandboxProfile>,
    pub permissions: Vec<ExecutionPermission>,
    pub routing_rules: Vec<RoutingRule>,
    pub trace_id: String,
    pub parent_execution_id: Option<ExecutionId>,
}
```

### ExecutionBudget — resource limits

```rust
pub struct ExecutionBudget {
    pub timeout_ms: u64,            // Wall-clock deadline from dispatch
    pub max_cpu_ms: u64,            // CPU time limit (0 = unlimited)
    pub max_memory_bytes: u64,      // Peak memory limit
    pub max_output_bytes: u64,      // Total stdout+stderr limit
    pub max_retries: u32,           // Max automatic retry attempts
    pub deadline: Timestamp,        // Absolute deadline computed from timeout
}
```

### ExecutionHandle — returned on successful dispatch

```rust
pub struct ExecutionHandle {
    pub execution_id: ExecutionId,
    pub plan: ExecutionPlan,
    pub state: ExecutionState,
    pub dispatched_at: Timestamp,
    pub deadline: Timestamp,
    pub cancel_tx: tokio::sync::oneshot::Sender<()>,
    pub state_rx: tokio::sync::watch::Receiver<ExecutionState>,
}
```

### ExecutionResult — produced on completion

```rust
pub struct ExecutionResult {
    pub execution_id: ExecutionId,
    pub state: ExecutionState,        // Completed, Failed, TimedOut, Cancelled
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub artifacts: Vec<ExecutionArtifact>,
    pub parsed_output: Option<serde_json::Value>,
    pub metrics: ExecutionMetrics,
    pub error: Option<ExecutionError>,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub trace_id: String,
}
```

### ExecutionArtifact — named byte payload

```rust
pub struct ExecutionArtifact {
    pub name: String,
    pub content_type: String,       // MIME type
    pub data: Vec<u8>,
    pub size_bytes: u64,
}
```

### ExecutionMetrics — resource consumption

```rust
pub struct ExecutionMetrics {
    pub cpu_time_ms: u64,
    pub peak_memory_bytes: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub wall_clock_ms: u64,
    pub network_bytes_in: u64,
    pub network_bytes_out: u64,
}
```

### ToolRequirement — abstract capability declaration from Brain

```rust
pub struct ToolRequirement {
    pub capability: ToolCapabilityId,
    pub input_schema: serde_json::Value,     // JSON Schema for input parameters
    pub output_expectations: Vec<OutputExpectation>,
    pub required_permissions: Vec<ExecutionPermission>,
    pub preferred_sandbox: Option<IsolationLevel>,
    pub priority: u8,                        // 0-255 (higher = more urgent)
}
```

### ToolCandidate — registered, executable tool

```rust
pub struct ToolCandidate {
    pub tool_id: String,
    pub name: String,
    pub version: String,
    pub capability: ToolCapabilityId,
    pub binding: ToolBinding,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
    pub side_effects: Vec<SideEffect>,
    pub sandbox_profile: SandboxProfile,
    pub metadata: HashMap<String, String>,
}
```

### ToolBinding — maps candidate to execution environment

```rust
pub enum ToolBinding {
    Subprocess {
        binary: String,
        default_args: Vec<String>,
        env: HashMap<String, String>,
        working_dir: Option<String>,
    },
    Wasm {
        module_path: String,
        function_name: String,
        allow_net: bool,
        allow_fs: bool,
    },
    Container {
        image: String,
        command: Vec<String>,
        mounts: Vec<Mount>,
        network: NetworkConfig,
    },
}
```

### ExecutionPlan — validated, dispatch-ready plan

```rust
pub struct ExecutionPlan {
    pub execution_id: ExecutionId,
    pub tool: ToolCandidate,
    pub binding: ToolBinding,
    pub parameters: HashMap<String, serde_json::Value>,
    pub sandbox_profile: SandboxProfile,
    pub budget: ExecutionBudget,
    pub permissions: Vec<ExecutionPermission>,
    pub routing_rules: Vec<RoutingRule>,
    pub priority: u8,
}
```

### ExecutionSession — scoped execution context

```
ExecutionSession is created by the Coordinator when a ToolRequirement arrives.
It binds the Brain's CognitiveBudget, the Runtime task context, the trace ID,
and the execution pipeline flow into a single scoped session.

ExecutionSession:
  session_id: SessionId
  execution_id: ExecutionId
  brain_request_id: Option<RequestId>
  runtime_task_id: Option<TaskId>
  trace_id: String
  budget: ExecutionBudget
  permissions: Vec<ExecutionPermission>
  started_at: Timestamp
  state: ExecutionState
```

---

## Execution State Machine

```
                  ┌─────────────────────────────────────────────────┐
                  │                                                 │
                  v                                                 │
  Pending ──→ Queued ──→ Dispatching ──→ Running ──→ Completed     │
    │            │            │              │                      │
    │            │            │              ├──→ Failed            │
    │            │            │              ├──→ TimedOut          │
    │            │            │              └──→ Cancelled         │
    │            │            │                                      │
    └─────→ Cancelled ←──────┴────────── Cancelled                  │
                                                                     │
  Completed ──→ Retrying ──→ Queued (re-dispatch)                    │
    │                                                                 │
    └─────→ RollingBack ──→ Completed (rolled back)                  │
                                                                     │
  Failed ────→ Retrying ──→ Queued (re-dispatch)                     │
    │                                                                 │
    └─────→ RollingBack ──→ Completed (rolled back)                  │
                                                                     │
  TimedOut ──→ Retrying ──→ Queued (re-dispatch)                     │
    │                                                                 │
    └─────→ RollingBack ──→ Completed (rolled back)                  │
                                                                     │
  Cancelled ──→ RollingBack ──→ Completed (rolled back)              │
                                                                     │
  RollingBack ──→ Failed (rollback error) ─────────────→ Escalated   │
                                                                     ┘
```

### State Transitions

| Current State | Event | Next State | Description |
|---|---|---|---|
| `Pending` | Submit to dispatcher | `Queued` | Plan accepted, waiting for capacity |
| `Queued` | Capacity available | `Dispatching` | Selected for dispatch |
| `Queued` | Cancel requested | `Cancelled` | Removed from queue before dispatch |
| `Dispatching` | Runner accepts | `Running` | Execution started on backend |
| `Dispatching` | Runner rejects | `Failed` | Backend unavailable or invalid |
| `Dispatching` | Cancel requested | `Cancelled` | Aborted during dispatch |
| `Running` | Exit code 0 | `Completed` | Successful execution |
| `Running` | Non-zero exit | `Failed` | Execution error |
| `Running` | Timeout exceeded | `TimedOut` | Deadline reached |
| `Running` | Cancel requested | `Cancelled` | Graceful or forced termination |
| `Running` | Resource violation | `Failed` | Budget exceeded (OOM, output too large) |
| `Completed` | Retry policy applies | `Retrying` | Result requires retry (e.g., transient error) |
| `Failed` | Retry policy applies | `Retrying` | Transient failure, retries remaining |
| `Failed` | No retries remaining | `RollingBack` | Starting compensation |
| `TimedOut` | Retry policy applies | `Retrying` | Retry on timeout |
| `TimedOut` | No retries remaining | `RollingBack` | Starting compensation |
| `Cancelled` | Rollback configured | `RollingBack` | Cleanup required |
| `Cancelled` | No rollback | `Completed` | No-op cancellation |
| `Retrying` | Backoff elapsed | `Queued` | Re-entering dispatch |
| `Retrying` | Max retries exhausted | `RollingBack` | Starting compensation |
| `RollingBack` | Compensation complete | `Completed` | Execution rolled back |
| `RollingBack` | Compensation failed | `Escalated` | Requires operator intervention |
| `Escalated` | Operator resolves | `Completed` | Manual resolution |

---

## Execution Pipeline

```
┌──────────┐   ToolRequirement    ┌──────────┐   ExecutionPlan   ┌──────────┐
│  Brain   │ ──────────────────→  │  Planner │ ────────────────→ │Dispatch │
│ (Planner)│                      │          │                   │  Queue  │
└──────────┘                      └──────────┘                   └────┬─────┘
                                                                      │
                                                                      v
┌──────────┐   Enriched Result   ┌──────────┐   Raw Result    ┌──────────┐
│  Memory  │ ←─────────────────  │ Results  │ ←─────────────  │  Runner  │
│ (History)│                     │ Collector│                 │ (Backend)│
└──────────┘                     └──────────┘                 └────┬─────┘
       │                                                            │
       v                                                            v
┌──────────┐                                               ┌──────────┐
│  Brain   │                                               │   OSAL   │
│(Reflect) │                                               │(Process, │
└──────────┘                                               │ Filesys.)│
                                                           └──────────┘

                    ┌──────────┐    ┌──────────┐    ┌──────────┐
                    │ Sandbox  │    │ Monitor  │    │ Recovery │
                    │Enforcer  │    │ (Timeout,│    │ Manager  │
                    │          │    │Resource) │    │ (Retry,  │
                    └──────────┘    └──────────┘    │ Rollback)│
                                                    └──────────┘

All stages communicate through bounded tokio::sync::mpsc channels.
The Coordinator wires the pipeline at startup and manages bypass mode.
```

### Pipeline Stage Details

| Stage | Input | Output | Responsibility |
|---|---|---|---|
| Planner | `ToolRequirement` | `ExecutionPlan` | Resolve requirement → candidate, validate input schema, resolve sandbox profile |
| Dispatch Queue | `ExecutionPlan` | `ExecutionPlan` (dispatched) | Concurrency limit, priority ordering, backend selection |
| Sandbox Enforcer | Candidate + Context | `SandboxProfile` | Resolve sandbox level from policy, prepare isolation |
| Runner | `ExecutionPlan` + Profile | `RunnerHandle` | Spawn backend, pipe I/O, manage lifecycle |
| Monitor | `RunnerHandle` | `ExecutionState` updates | Enforce timeout, track resources, detect violations |
| Results Collector | Completed execution | `ExecutionResult` | Collect output, parse, enrich, produce result |
| Recovery Manager | `ExecutionResult` (failed) | Recovery decision | Evaluate retry policy, execute rollback |

---

## Thread Model

| Subsystem | Concurrency Model |
|---|---|
| `Planner` | Stateless, synchronous. Runs on the caller's async task. |
| `Dispatch Queue` | `tokio::sync::RwLock<BinaryHeap<PrioritizedPlan>>`. Dispatch runs on coordinator task. Waiters use `tokio::sync::Notify`. |
| `Sandbox Enforcer` | Stateless, synchronous. Runs on dispatcher's task. |
| `SubprocessRunner` | Per-execution `tokio::process::Child` task. I/O pipes read on dedicated tasks via `tokio::io::AsyncRead`. |
| `WasmRunner` | WASM compilation on `spawn_blocking`. Instance execution on caller's async task. Fresh instance per execution. |
| `ContainerRunner` | HTTP calls to OCI runtime API via async HTTP client. Container health polled on `tokio::time::interval`. |
| `Monitor` | One Tokio task per active execution for timeout enforcement. Resource sampling on `tokio::time::interval`. |
| `Results Collector` | Runs on runner's completion callback. Stateless. |
| `Recovery Manager` | `tokio::sync::RwLock<HashMap<ExecutionId, RecoveryState>>`. Recovery actions run on coordinator task. |
| `Coordinator` | Own Tokio task for lifecycle management. `watch` channels for stage health. EventBus subscription for reconfiguration. |
| `Execution Registry` | `tokio::sync::RwLock<HashMap<ToolCapabilityId, Vec<ToolCandidate>>>`. Read-heavy. |

**Concurrency limit:** The max concurrent executions setting controls how many Runner tasks can exist simultaneously. Excess plans are queued. The limit applies across all backends.

**Backpressure:** All inter-stage `mpsc` channels have bounded capacities (default: 10,000). When full:
- Critical priority plans block the sender
- Normal priority plans drop oldest from queue
- Low priority plans are rejected immediately

---

## Lifecycle

### Start Order

```
 1. execution-core                    (types registered — no-op)
 2. execution-sandbox                 (profiles loaded from config)
 3. execution-registry                (ToolCandidates registered from config and plugins)
 4. execution-planner                 (schemas loaded, validators initialized)
 5. execution-dispatcher              (dispatch queue initialized, backend selector warmed)
 6. execution-runner                  (backends initialized: WASM engine precompiled, container runtime checked)
 7. execution-monitor                 (monitoring infrastructure started)
 8. execution-results                 (routing rules loaded, parsers registered)
 9. execution-recovery                (recovery strategies loaded)
10. execution-coordinator             (pipeline assembled, Brain bridge connected)
```

The coordinator performs pipeline assembly in `start()`:
1. Create bounded `mpsc` channels between consecutive stages.
2. Register stage handles with PipelineManager.
3. Spawn coordinator lifecycle task.
4. Subscribe to EventBus for cancellation and reconfiguration events.
5. Register health checks with Core's HealthMonitor.
6. Publish `execution.coordinator.started` event.

### Shutdown Order

Shutdown proceeds in reverse with graceful drains:

```
 1. execution-coordinator             (stop accepting new plans, drain pipeline, cancel active executions)
 2. execution-recovery                (persist recovery state for in-flight recoveries)
 3. execution-results                 (flush pending results, close routing sinks)
 4. execution-monitor                 (stop watches, persist resource snapshots)
 5. execution-runner                  (SIGTERM → grace → SIGKILL active executions, close backends)
 6. execution-dispatcher              (clear queue, reject pending plans)
 7. execution-planner                 (close schema registry)
 8. execution-registry                (unregister tool candidates)
 9. execution-sandbox                 (close sandbox profiles)
```

Each stage has a configurable drain timeout (default: 10 seconds). If a stage fails to drain, remaining items are discarded and logged.

### Health Checks

Registered by coordinator with Core's HealthMonitor:

| Check | Probe | Frequency | Failure Action |
|---|---|---|---|
| Runner liveness | Verify backend is responsive | 5 s | Mark backend degraded, retry connection |
| Dispatch queue depth | Log if > 80% capacity | 5 s | Emit backpressure warning |
| Active execution count | Compare to max concurrency | 5 s | Log saturation warning |
| Stage latency | Measure p99 per stage | 10 s | Log warning if exceeded |
| Pipeline throughput | Count plans processed per second | 10 s | Log warning if below threshold |
| Recovery backlog | Count pending recovery actions | 10 s | Escalate if growing |

---

## Event Model

### Published Events

| Event | Type String | Stage | Trigger |
|---|---|---|---|
| `ExecutionPlanCreated` | `execution.plan.created` | Planner | ToolRequirement successfully resolved to ExecutionPlan |
| `ExecutionPlanRejected` | `execution.plan.rejected` | Planner | Requirement could not be resolved (no candidate, invalid input) |
| `ExecutionDispatched` | `execution.dispatched` | Dispatcher | Plan accepted and dispatched to runner |
| `ExecutionQueued` | `execution.queued` | Dispatcher | Plan queued due to concurrency limit |
| `ExecutionStarted` | `execution.started` | Runner | Backend execution started |
| `ExecutionCompleted` | `execution.completed` | Runner | Exit code 0 |
| `ExecutionFailed` | `execution.failed` | Runner/Monitor | Non-zero exit, resource violation, or backend error |
| `ExecutionTimedOut` | `execution.timed_out` | Monitor | Wall-clock timeout exceeded |
| `ExecutionCancelled` | `execution.cancelled` | Monitor | Cancellation requested and confirmed |
| `ExecutionRetrying` | `execution.retrying` | Recovery | Retry policy triggered, backoff starting |
| `ExecutionRollingBack` | `execution.rolling_back` | Recovery | Rollback plan executing |
| `ExecutionRolledBack` | `execution.rolled_back` | Recovery | Rollback completed successfully |
| `ExecutionEscalated` | `execution.escalated` | Recovery | Requires operator intervention |
| `ExecutionResultRouted` | `execution.result.routed` | Results | Result delivered to destination |
| `ExecutionResultRouteFailed` | `execution.result.route_failed` | Results | Result delivery failed |
| `ExecutionArtifactStored` | `execution.artifact.stored` | Results | Artifact persisted to Memory |
| `ResourceLimitExceeded` | `execution.resource.exceeded` | Monitor | Budget limit hit (OOM, output size, CPU) |
| `BackpressureWarning` | `execution.backpressure` | Dispatcher | Queue capacity exceeded threshold |
| `PipelineStageFailed` | `execution.pipeline.stage_failed` | Coordinator | Stage encountered unrecoverable error |
| `CoordinatorStarted` | `execution.coordinator.started` | Coordinator | Pipeline assembled and ready |
| `CoordinatorStopped` | `execution.coordinator.stopped` | Coordinator | Pipeline shut down |

### Consumed Events

| Source | Event | Consumer | Purpose |
|---|---|---|---|
| Brain `brain-coordinator` | `brain.decision.made` | Coordinator | Submit ToolRequirement for execution |
| Brain `brain-planner` | `brain.plan.step_ready` | Coordinator | Plan step requires tool execution |
| Runtime | `runtime.task_cancelled` | Coordinator | Cancel associated execution |
| Runtime | `runtime.session_destroyed` | Coordinator | Cancel all session-scoped executions |
| Core | `core.config_changed` | Coordinator | Reload pipeline configuration |
| Core | `core.service_stopped` | Coordinator | Initiate graceful shutdown |
| Perception | `perception.observation.ready` | Results (future) | Enrich execution results with perception context |

---

## Error Handling

### ExecutionError Hierarchy

```rust
pub enum ExecutionError {
    // Planner errors (1xxx)
    NoSuitableCandidate(ToolCapabilityId),
    InvalidInput { tool: String, reason: String },
    SchemaValidationFailed { tool: String, errors: Vec<String> },

    // Dispatcher errors (2xxx)
    MaxConcurrencyReached,
    QueueFull { priority: u8, queue_size: usize },
    BackendSelectionFailed(String),
    DispatchRejected(String),

    // Runner errors (3xxx)
    SubprocessSpawnFailed(String),
    SubprocessKillFailed(String),
    WasmCompilationFailed(String),
    WasmExecutionFailed(String),
    WasmInstanceNotSend,
    ContainerPullFailed(String),
    ContainerStartFailed(String),
    ContainerExecFailed(String),
    BackendUnavailable(String),

    // Sandbox errors (4xxx)
    SandboxCreationFailed(String),
    SandboxViolation { policy: String, detail: String },
    IsolationLevelNotSupported(IsolationLevel),

    // Monitor errors (5xxx)
    TimeoutExceeded { execution_id: ExecutionId, timeout_ms: u64 },
    MemoryLimitExceeded { peak_bytes: u64, limit_bytes: u64 },
    OutputLimitExceeded { output_bytes: u64, limit_bytes: u64 },
    CpuLimitExceeded { cpu_ms: u64, limit_ms: u64 },

    // Results errors (6xxx)
    OutputParseFailed { format: String, error: String },
    OutputTruncated { captured_bytes: u64, limit_bytes: u64 },
    ArtifactStoreFailed(String),
    RouteFailed { destination: String, error: String },

    // Recovery errors (7xxx)
    RetryExhausted { execution_id: ExecutionId, attempts: u32 },
    RollbackFailed { execution_id: ExecutionId, reason: String },
    RollbackPlanNotFound(ExecutionId),
    RecoveryInProgress(ExecutionId),

    // Coordinator errors (8xxx)
    PipelineAssemblyFailed(String),
    StageTimeout(String),
    ChannelClosed(String),
    ConfigurationError(String),

    // Wrapped errors
    Core(core::CoreError),
    Runtime(runtime::RuntimeError),
    Memory(memory_core::MemoryError),
    Osal(osal_core::OsalError),
    Perception(perception_core::PerceptionError),
}
```

### Recovery Strategies

| Error | Recovery |
|---|---|
| `NoSuitableCandidate` | Return to Brain with available capabilities. Brain may replan with different tool. |
| `InvalidInput` | Return validation errors to Brain. No retry. |
| `MaxConcurrencyReached` | Automatic queue with backpressure notification. Dispatched when capacity frees. |
| `QueueFull` | Reject with backpressure event. Brain should back off and retry. |
| `SubprocessSpawnFailed` | Retry once after 1s (transient resource exhaustion). Permanent if binary missing. |
| `WasmCompilationFailed` | No retry — module is invalid. Return error details to Brain. |
| `ContainerPullFailed` | Retry with alternate registry mirror (if configured). Max 3 attempts. |
| `TimeoutExceeded` | Retry if retries remaining. Send SIGTERM → grace → SIGKILL. Preserve partial output. |
| `MemoryLimitExceeded` | No retry — will OOM again. Route partial output with warning. |
| `OutputLimitExceeded` | No retry. Truncate output, route with truncated flag set. |
| `OutputParseFailed` | Fall back to raw binary output. `parsed_output` set to null. |
| `RouteFailed` | Buffer with bounded retry queue (max 3). Drop oldest if buffer full. |
| `RollbackFailed` | Escalate to operator. Persist full execution context for manual resolution. |

All errors are non-fatal at the platform level. The coordinator ensures the pipeline continues operating even when individual stages fail.

---

## Retry Model

```
Execution completes with Failed/TimedOut state
         │
         v
  ┌─ Does RetryPolicy exist? ── No ──→ Go to Rollback or Finalize
  │
  Yes
  │
  v
  ┌─ Are retries remaining? ── No ──→ Go to Rollback or Escalate
  │
  Yes
  │
  v
  Compute backoff delay: base_ms × multiplier^attempt
  Add jitter: ±25% random
  │
  v
  Wait backoff duration
  │
  v
  Return to Dispatch Queue
  │
  v
  ┌─ Max retries exceeded? ── Yes ──→ Go to Rollback or Escalate
  │
  No
  │
  └────────────────────→ (re-dispatch)
```

**RetryPolicy parameters:**
- `max_retries: u32` — Maximum automatic retry attempts (default: 3)
- `backoff_base_ms: u64` — Initial backoff delay (default: 1000)
- `backoff_multiplier: f64` — Exponential multiplier (default: 2.0)
- `backoff_max_ms: u64` — Maximum backoff cap (default: 60000)
- `jitter: f64` — Random jitter fraction (default: 0.25)
- `retry_on_timeout: bool` — Whether to retry on timeout (default: true)
- `retry_on_exit_codes: Vec<i32>` — Which exit codes trigger retry (default: [])
- `max_total_timeout_ms: u64` — Total wall-clock timeout across all retries (default: 0 = unlimited)

---

## Timeout Model

```
Dispatch ──→ deadline = now + budget.timeout_ms
                  │
                  v
         Monitor starts timer
                  │
                  v
    ┌─ Timer expired? ── No ──→ Continue monitoring
    │
    Yes
    │
    v
    Send SIGTERM to process group
    Wait grace period (budget.timeout_ms × 10%, min 1s, max 10s)
    │
    v
    ┌─ Process still alive? ── No ──→ Collect partial output, mark TimedOut
    │
    Yes
    │
    v
    Send SIGKILL
    Collect partial output, mark TimedOut
```

**Timeout configuration:**
- Per-execution via `ExecutionBudget.timeout_ms`
- Grace period: `max(timeout_ms × 0.1, 1000)` capped at 10,000ms
- Partial output before timeout is preserved and routed with a `timed_out` tag
- Timeout state is recorded in `ExecutionResult.state = TimedOut`

---

## Cancellation Model

```
                    ┌──────────────────────────────┐
                    │   Cancel requested            │
                    │   (EventBus, Runtime, direct) │
                    └──────────────┬───────────────┘
                                   │
                    ┌──────────────v───────────────┐
                    │  Look up ExecutionHandle     │
                    │  by execution_id             │
                    └──────────────┬───────────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              v                    v                    v
      ┌──────────────┐   ┌────────────────┐   ┌────────────────┐
      │ In Queue?    │   │ Dispatching?   │   │ Running?       │
      └──────┬───────┘   └───────┬────────┘   └───────┬────────┘
             v                   v                    v
     ┌──────────────┐   ┌────────────────┐   ┌──────────────────┐
     │ Remove from  │   │ Abort dispatch │   │ Runner.cancel()   │
     │ queue        │   │                │   │                   │
     └──────────────┘   └────────────────┘   └──────────────────┘
             │                   │                    │
             │                   │           ┌────────┴────────┐
             │                   │           v                 v
             │                   │   ┌────────────┐   ┌────────────────┐
             │                   │   │ Subprocess │   │ Wasm/Container │
             │                   │   │ SIGTERM →  │   │ Drop instance  │
             │                   │   │ SIGKILL    │   │ Close conn     │
             │                   │   └────────────┘   └────────────────┘
             │                   │                    │
             └───────────────────┴────────────────────┘
                                   │
                                   v
                     ┌──────────────────────────┐
                     │  Mark state = Cancelled  │
                     │  Collect partial output  │
                     │  Publish Cancelled event │
                     │  Check rollback policy   │
                     └──────────────────────────┘
```

**Cancellation sources:**
1. **Direct cancellation** — Coordinator cancels by execution ID
2. **Runtime task cancellation** — TaskManager cancels the Runtime task, ExecutionMonitor cancels the associated execution
3. **Session cancellation** — Session destroyed, all session-scoped executions cancelled
4. **Budget exhaustion** — Timeout or resource limit automatically triggers cancellation
5. **Escalation** — Recovery escalation cancels remaining retries

**Cancellation guarantees:**
- Best-effort: cancellation signals are delivered but cannot guarantee process termination
- Grace period is enforced before force-kill
- Partial output and state are preserved in `ExecutionResult`
- `ExecutionCancelled` event is published with reason

---

## Rollback Model

```
Execution completed in Failed/TimedOut/Cancelled state
                    │
                    v
         ┌─ RollbackPlan exists? ── No ──→ Finalize normally
         │
         Yes
         │
         v
         Disable timeout for rollback (rollback gets own budget)
         │
         v
         Execute compensation actions in order
         │
         ├──→ Compensation succeeds ──→ Mark Completed, publish RolledBack
         │
         └──→ Compensation fails ──→ Mark Escalated, publish Escalated
                                        Persist full context for manual resolution
```

**RollbackPlan:**
```rust
pub struct RollbackPlan {
    pub execution_id: ExecutionId,
    pub compensation_steps: Vec<CompensationStep>,
    pub timeout_ms: u64,
    pub on_failure: OnRollbackFailure,
}

pub enum CompensationStep {
    ExecuteTool {
        tool: ToolCapabilityId,
        params: HashMap<String, serde_json::Value>,
        description: String,
    },
    RestoreSnapshot {
        snapshot_id: String,
        target: String,
    },
    DeleteCreated {
        path: String,
    },
    Notify {
        channel: String,
        message: String,
    },
}

pub enum OnRollbackFailure {
    Escalate,
    IgnoreAndContinue,
    Retry(max_retries: u32, backoff_ms: u64),
}
```

---

## Permission Model

```
ToolRequirement arrives with required_permissions
         │
         v
  Coordinator checks ExecutionSession permissions
         │
         v
  Dispatch queue checks sandbox profile
         │
         v
  SandboxEnforcer validates required IsolationLevel
         │
         v
  Runner enforces OS-level permissions via OSAL
         │
         v
  Results check routing permissions before delivery
```

**ExecutionPermission** is an enum:
```rust
pub enum ExecutionPermission {
    ReadFile(String),             // Specific file path
    WriteFile(String),            // Specific file path
    ReadDir(String),              // Directory listing scope
    NetworkConnect(String),       // Specific host:port
    NetworkListen(u16),           // Specific port
    ProcessSpawn,                 // Spawn additional processes
    EnvironmentRead(String),      // Specific env var
    SystemTime,                   // Read/set system clock
    SystemInfo,                   // Read OS information
    DbusCall(String),             // Specific D-Bus interface
    Custom(String),               // Extension point
}
```

**Permission enforcement** checks:
1. The `ExecutionSession` has all required `ExecutionPermission` values
2. The `SandboxProfile` allows the requested operations
3. The OSAL `PermissionChecker` validates OS-level access

---

## Capability Model

```
ToolCapabilityId is a namespaced string identifier:

  "fs.read"          — Read file contents
  "fs.write"         — Write file contents
  "fs.list"          — List directory entries
  "fs.metadata"      — Get file metadata
  "fs.delete"        — Delete file or directory
  "process.list"     — List running processes
  "process.spawn"    — Spawn a subprocess
  "process.kill"     — Terminate a process
  "process.info"     — Get process details
  "network.dns"      — DNS lookup
  "network.http"     — HTTP request (outbound)
  "network.connect"  — TCP/UDP connect
  "network.listen"   — TCP/UDP listen
  "terminal.exec"    — Execute terminal command
  "terminal.pty"     — Open PTY session
  "device.info"      — Query hardware information
  "device.control"   — Control hardware device
  "system.info"      — Read OS information
  "system.env"       — Read environment variables
  "user.info"        — Query user information
  "user.session"     — Manage user sessions
  "dbus.call"        — D-Bus method call
  "dbus.listen"      — D-Bus signal subscription
  "custom.*"         — Extension namespace
```

Each `ToolCapabilityId` maps to a `SandboxProfile` via the `SandboxPolicy`:

| Capability | Min Isolation Level | Default Filesystem | Default Network |
|---|---|---|---|
| `fs.read` | None | ReadOnly | None |
| `fs.write` | Process | ReadWrite (scoped) | None |
| `fs.delete` | Process | ReadWrite (scoped) | None |
| `process.spawn` | Container | None | None |
| `process.kill` | Process | None | None |
| `network.http` | Process | None | OutboundOnly |
| `network.listen` | Container | None | InboundOnly |
| `terminal.exec` | Process | ReadOnly | None |
| `device.control` | Container | None | None |
| `system.env` | None | None | None |
| `dbus.call` | Container | None | None |

---

## Scheduler Interaction

Execution Platform integrates with Runtime's Scheduler through the Dispatcher:

1. **Task creation** — When an `ExecutionPlan` is ready for dispatch, the Dispatcher creates a Runtime `Task` via `TaskManager::create()`. The task is associated with the `ExecutionSession`.

2. **Priority mapping** — The Brain's `CognitiveBudget.priority` (0-255) is mapped to Runtime `TaskPriority` (0-100). Default mapping: `brain_priority / 2.55`.

3. **Supervision** — The Runtime `Supervisor` monitors the task. If the task itself fails (not the execution), the Supervisor restarts the task. Execution-level failures are handled by the Recovery Manager, not the Supervisor.

4. **Resource accounting** — The `Monitor` feeds resource usage back to Runtime's `ResourceManager` for aggregate session-level tracking and limit enforcement.

5. **Session binding** — All tasks spawned by an `ExecutionSession` are tagged with the session ID. When a session is destroyed, all bound executions are cancelled.

---

## OSAL Integration

All OS-level operations go through OSAL traits:

| Execution Operation | OSAL Trait | OSAL Crate |
|---|---|---|
| Spawn subprocess | `ProcessManager::spawn()` | `osal-process` |
| Kill subprocess | `ProcessManager::kill()` | `osal-process` |
| Wait for subprocess | `ProcessManager::wait()` | `osal-process` |
| Read subprocess output | `ProcessManager::stdout()/stderr()` | `osal-process` |
| Read/write files | `FileSystemProvider::read()/write()` | `osal-filesystem` |
| List directory | `FileSystemProvider::read_dir()` | `osal-filesystem` |
| Network DNS lookup | `NetworkManager::dns_lookup()` | `osal-network` |
| Open PTY | `TerminalProvider::open_pty()` | `osal-terminal` |
| Query resource usage | `MonitorProvider::cpu_usage()/memory_info()` | `osal-monitoring` |
| Query hardware info | `PlatformInfoProvider::hardware()` | `osal-platform` |
| Check permissions | `PermissionChecker::check()` | `osal-capabilities` |

---

## Brain Integration

Brain → Execution communication:

```
BrainPlanner → ToolRequirement
     │
     v
BrainCoordinator → publish brain.decision.made event
     │
     v
ExecutionCoordinator → subscribe to brain.decision.made
     │
     v
ExecutionPlanner → resolve ToolRequirement → ExecutionPlan
     │
     v
ExecutionPipeline → dispatch, run, collect
     │
     v
ExecutionResults → route result
     │
     ├──→ Memory (execution history)
     ├──→ Brain (brain.result.ready event → BrainReflection)
     ├──→ Pipeline (next execution stage)
     └──→ Other sinks (filesystem, EventBus)
```

The Brain does **not** call Execution Platform traits directly. Communication happens through:
1. **EventBus** — `brain.decision.made` events carry `ToolRequirement` payloads. Coordinator subscribes.
2. **Result events** — `execution.completed`, `execution.failed`, etc. are consumed by Brain Coordinator for reflection and learning.

---

## Memory Integration

| Execution Data | Memory Tier | Purpose |
|---|---|---|
| `ExecutionResult` | Episodic | Complete execution record (traceable) |
| `ExecutionArtifact` | Semantic/Working | Tool outputs for ongoing reasoning |
| `ExecutionMetrics` | Episodic | Resource consumption history |
| `ExecutionHistory` | Episodic | Ordered sequence of all executions |
| Rollback plans | Episodic | Compensation actions for future reference |
| Recovery state | Working | Active recovery tracking |

Memory is written by:
- **Results Collector** — Stores results and artifacts via `MemoryStore`
- **Monitor** — Records resource snapshots periodically
- **Recovery Manager** — Persists recovery state for crash recovery

Memory is read by:
- **Planner** — Loads previous execution outcomes for similar requirements
- **Recovery Manager** — Loads persisted recovery state on coordinator restart

---

## Perception Integration

Perception integration is **future** (Phase 9). Planned touchpoints:

1. **Observation enrichment** — Execution results (stdout, stderr, artifacts) are enriched with perception context (entity resolution, anomaly scoring) before delivery to Brain.
2. **Output monitoring** — Perception's anomaly detector monitors execution output streams for unusual patterns.
3. **Event observation** — Perception's observer layer can be configured to observe execution events for system monitoring purposes.

The integration will be implemented by adding a `PerceptionEnricher` stage in the Results Collector pipeline, after parsing but before routing.

---

## EventBus Integration

| Subscription | Stage | Handler |
|---|---|---|
| `brain.decision.made` | Coordinator | Submit ToolRequirement to pipeline |
| `brain.plan.step_ready` | Coordinator | Submit plan step for execution |
| `runtime.task_cancelled` | Monitor | Cancel associated execution |
| `runtime.session_destroyed` | Monitor | Cancel all session executions |
| `core.config_changed` | Coordinator | Reload pipeline configuration |
| `core.service_stopped` | Coordinator | Initiate graceful shutdown |

| Publication | Stage | Payload |
|---|---|---|
| `execution.plan.created` | Planner | ExecutionId, ToolCapabilityId, plan summary |
| `execution.plan.rejected` | Planner | ToolCapabilityId, reason |
| `execution.dispatched` | Dispatcher | ExecutionId, backend, concurrency |
| `execution.started` | Runner | ExecutionId, backend, pid/instance |
| `execution.completed` | Runner | ExecutionId, exit code, metrics summary |
| `execution.failed` | Monitor | ExecutionId, error, stage |
| `execution.timed_out` | Monitor | ExecutionId, elapsed, partial size |
| `execution.cancelled` | Monitor | ExecutionId, reason, partial size |
| `execution.retrying` | Recovery | ExecutionId, attempt, backoff |
| `execution.rolling_back` | Recovery | ExecutionId, step |
| `execution.rolled_back` | Recovery | ExecutionId |
| `execution.escalated` | Recovery | ExecutionId, reason, context |
| `execution.result.routed` | Results | ExecutionId, destinations |
| `execution.backpressure` | Dispatcher | Queue depth, capacity |
| `execution.pipeline.stage_failed` | Coordinator | Stage name, error, restarts |
| `execution.coordinator.started` | Coordinator | Config summary |
| `execution.coordinator.stopped` | Coordinator | Uptime, total executions |

---

## Performance Targets

| Operation | p50 Latency | p99 Latency | Sustained Throughput |
|---|---|---|---|
| Plan resolution (cached candidate) | 2 µs | 10 µs | 200K/s |
| Dispatch (no queue) | 5 µs | 20 µs | 100K/s |
| Sandbox profile resolution | 1 µs | 5 µs | 500K/s |
| Subprocess spawn (empty binary) | 200 µs | 1 ms | 5K/s |
| WASM instantiate + run (empty) | 100 µs | 500 µs | 10K/s |
| Container start (warm image, cached) | 500 ms | 2 s | 100/s |
| Output collection (1 KB) | 5 µs | 20 µs | 100K/s |
| Output parsing (JSON, 1 KB) | 10 µs | 50 µs | 50K/s |
| Result routing (1 destination) | 5 µs | 20 µs | 100K/s |
| Retry evaluation | 1 µs | 5 µs | 500K/s |
| Rollback step execution | 100 µs | 1 ms | 10K/s |
| **End-to-end (subprocess, cached)** | 500 µs | 5 ms | 2K/s |
| **End-to-end (WASM, cached)** | 300 µs | 3 ms | 3K/s |
| **End-to-end (container, warm)** | 1 s | 5 s | 50/s |

**Resource envelope** (per 1,000 executions/sec with subprocess backend):
- CPU: ~4 cores (mainly process spawn/wait and output parsing)
- Memory: ~1 GB (process overhead, output buffers, artifact storage)
- Network: varies by tool (OCI registry pulls for container backend)
- File descriptors: 3 per active execution (stdin, stdout, stderr) + sandbox overhead

---

## References

- [Architecture Overview](overview.md)
- [Execution Platform Interfaces](../interfaces/execution.md)
- [Execution Platform Configuration](../configuration/execution.md)
- [Execution Pipeline Design](../execution-pipeline.md)
- [RFC-0005: Execution Platform](../rfc/RFC-0005-execution-platform.md)
- [Core Platform Architecture](core.md)
- [Runtime Platform Architecture](runtime.md)
- [OSAL Architecture](osal.md)
- [Memory Platform Architecture](memory.md)
- [Brain Platform Architecture](brain.md)
- [Perception Platform Architecture](perception.md)
