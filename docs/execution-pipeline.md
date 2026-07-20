# Execution Processing Pipeline

## Overview

The Execution Pipeline is a staged processing flow that transforms a Brain `ToolRequirement` into a completed execution with routed results. Each stage has a single responsibility and communicates with the next stage through bounded asynchronous channels.

```
  ToolRequirement
       │
       v
  ┌──────────────┐
  │   Planner    │  Resolve requirement → candidate, validate schema
  └──────┬───────┘
         │ ExecutionPlan
         v
  ┌──────────────┐
  │  Dispatcher  │  Concurrency limit, priority queue, backend selection
  └──────┬───────┘
         │ RunnerHandle
         v
  ┌──────────────┐     ┌──────────────┐
  │   Runner     │────→│    OSAL      │  Subprocess / WASM / Container
  └──────┬───────┘     └──────────────┘
         │ stdout/stderr/exit
         v
  ┌──────────────┐     ┌──────────────┐
  │   Monitor    │────→│   Timeout    │  Enforce budget, track resources
  └──────┬───────┘     │   Resource   │
         │             └──────────────┘
         v
  ┌──────────────┐
  │   Results    │  Collect output, parse, enrich, route
  └──────┬───────┘
         │ ExecutionResult
         v
  ┌──────────────┐     ┌──────────────┐
  │   Recovery   │────→│   Retry      │  Evaluate retry/rollback policy
  └──────┬───────┘     │   Rollback   │
         │             └──────────────┘
         v
  ┌──────────────┐
  │  Coordinator │  Publish final result, trigger reflection
  └──────────────┘
```

---

## Stage 1: Planner

**Input**: `ToolRequirement` from Brain (via EventBus `brain.decision.made`).

**Output**: `ExecutionPlan` or rejection.

**Behavior**:

1. **Capability resolution**: The Planner queries the `ToolRegistry` for candidates matching `ToolRequirement.capability`. If no candidates match, the requirement is rejected with `NoSuitableCandidate`.

2. **Candidate selection**: If multiple candidates match, the best one is selected by:
   - Version (prefer highest compatible version)
   - Sandbox profile (prefer least restrictive)
   - Registration order (stable tiebreaker)

3. **Input validation**: The `ToolRequirement.input_params` are validated against the candidate's `input_schema` (JSON Schema). Missing required fields or type mismatches produce `SchemaValidationFailed`.

4. **Sandbox resolution**: The `SandboxEnforcer` resolves the sandbox profile for the selected candidate and current `ExecutionContext`.

5. **Budget assembly**: The `ExecutionBudget` is assembled from the requirement's constraints and the coordinator's defaults. The `deadline` is computed as `now + timeout_ms`. If the requirement specifies a `CognitiveBudget`, it is used as the ceiling.

6. **Plan construction**: An `ExecutionPlan` is constructed with the resolved candidate, binding, parameters, sandbox profile, budget, permissions, and default routing rules.

7. **Event emission**: `ExecutionPlanCreated` is published with the plan summary. If planning fails, `ExecutionPlanRejected` is published.

**Backpressure**: The Planner runs synchronously on the coordinator's task. It does not have its own channel — the EventBus subscription provides natural backpressure via the Brain's own backpressure mechanisms.

**Error handling**:
- `NoSuitableCandidate`: Non-retryable. Brain must replan with a different tool.
- `SchemaValidationFailed`: Non-retryable. Brain must fix the input parameters.
- `SandboxEnforcer error`: Transient if sandbox profile not found (may be loaded later), permanent if isolation level unsupported.

**Events emitted**: `ExecutionPlanCreated`, `ExecutionPlanRejected`

---

## Stage 2: Dispatcher

**Input**: `ExecutionPlan` from Planner.

**Output**: `ExecutionHandle` (to caller) and dispatch to Runner.

**Behavior**:

1. **Priority queuing**: The plan is placed into a priority queue ordered by `plan.priority` (lower number = higher priority). Within the same priority, FIFO ordering is used.

2. **Concurrency check**: If `active_executions >= max_concurrency`, the plan remains in the queue. A `Notify` waiter is registered. When an execution completes, the waiter is notified and the highest-priority queued plan is dispatched.

3. **Backend selection**: The `BackendSelector` examines the `ToolBinding` and selects the appropriate backend:
   - Subprocess for `ToolBinding::Subprocess`
   - WASM for `ToolBinding::Wasm`
   - Container for `ToolBinding::Container`
   If the selected backend is at its per-backend concurrency limit, fallback selection is attempted.

4. **Sandbox creation**: The sandbox environment is prepared based on the `SandboxProfile`:
   - **None**: No isolation. Execution runs in-process (reserved for trusted tools).
   - **Process**: Process-level isolation via OSAL ProcessManager with seccomp/apparmor.
   - **Container**: Container-level isolation via OCI runtime.
   - **Wasm**: WASM sandbox via wasmtime runtime with capability limiting.

5. **Dispatch**: The plan is sent to the Runner. The dispatcher creates an `ExecutionHandle` with state `Dispatching`.

6. **Handle return**: The `ExecutionHandle` (with `state_rx` watch channel and `cancel_tx` oneshot) is returned to the caller (the Coordinator, which returns it to the Brain).

**Backpressure**: The dispatch queue has bounded capacity per priority level. If the queue is full:
- Critical priority: Blocks the sender (the Planner task).
- Normal priority: Drops the oldest plan from the queue.
- Low priority: Rejects immediately.

**Events emitted**: `ExecutionDispatched`, `ExecutionQueued`, `BackpressureWarning`

---

## Stage 3: Runner

**Input**: `ExecutionPlan` from Dispatcher.

**Output**: stdout/stderr streams, exit code, resource usage.

**Behavior**:

The Runner has three implementations. Selection is determined by `ToolBinding`.

### SubprocessRunner

1. **Environment setup**: Working directory is created if specified. Environment variables are set according to `ToolBinding::Subprocess.env`. Inherited variables matching `strip_env` are removed.

2. **Spawn**: OSAL `ProcessManager::spawn()` is called with the binary, args, env, and working directory. If the binary is in `denied_binaries` or not in `allowed_binaries`, spawn is rejected.

3. **I/O streaming**: Stdout and stderr pipes are read asynchronously via `tokio::io::AsyncRead`. Output is streamed through `mpsc` channels to the Monitor and Results stages. Each stream chunk is tagged with a sequence number and timestamp.

4. **Process wait**: The child process is awaited via OSAL `ProcessManager::wait()`. Exit code, signal, and resource usage (via `rusage` / `wait4`) are collected.

5. **Resource collection**: CPU time, peak memory, I/O bytes, and wall-clock duration are extracted from the process wait result and formatted as `ExecutionMetrics`.

### WasmRunner

1. **Module loading**: The WASM module is loaded from `ToolBinding::Wasm.module_path`. If precompilation is enabled, a cached compiled module is used.

2. **Instance creation**: A fresh WASM instance is created per execution. WASI is configured according to the sandbox profile (filesystem access, networking).

3. **Execution**: The exported function is called with input parameters serialized to the WASM module's expected format. A fuel limit (if configured) prevents infinite loops.

4. **Output collection**: The function's return value is captured. WASI stdout/stderr are buffered and collected.

5. **Resource collection**: Fuel consumed, wall-clock time, and memory used are collected.

### ContainerRunner

1. **Image handling**: The container image is pulled if `pull_policy` is `always` or `if_not_present` and the image is not cached. Pulls go through configured registry mirrors.

2. **Container creation**: The container is created via the OCI runtime API with the specified command, mounts, network config, and resource limits.

3. **Execution**: The container is started. Stdout/stderr are streamed via the OCI runtime API.

4. **Wait**: The container exit code is collected. Resource usage is queried from the OCI runtime's state.

**Events emitted**: `ExecutionStarted`, `ExecutionCompleted`, `ExecutionFailed`

---

## Stage 4: Monitor

**Input**: `RunnerHandle` from Runner, `ExecutionBudget` from Plan.

**Output**: State change signals (timeout, resource violation, cancellation).

**Behavior**:

1. **Timeout enforcement**: A Tokio timer is set for `budget.deadline`. If the timer fires before the execution completes:
   - The Runner's `cancel()` is called with `TimeoutExceeded`.
   - The Runner sends SIGTERM to the process group.
   - After grace period, SIGKILL is sent if still alive.
   - Partial output is preserved.
   - State transitions to `TimedOut`.

2. **Resource monitoring**: At a configurable interval (default: 1 second), resource usage is sampled:
   - CPU time from OSAL ProcessManager or WASM fuel counter
   - Memory from OSAL ProcessManager or WASM memory stats
   - Output size from ResultCollector's running buffer
   If any exceeds the budget, the execution is cancelled with the appropriate violation error.

3. **Watching**: The Monitor maintains a `HashMap<ExecutionId, ExecutionWatch>` behind `RwLock`. Each watch holds:
   - `deadline: Instant`
   - `cancel_tx: tokio::sync::oneshot::Sender<()>`
   - `resource_accumulator: ExecutionMetrics`
   - `state: ExecutionState`

4. **Unwatching**: On execution completion (any terminal state), the Monitor removes the watch and stops the timer.

5. **Resource logging**: Resource snapshots are periodically recorded to Memory (episodic store) for historical analysis.

**Events emitted**: `ExecutionTimedOut`, `ResourceLimitExceeded`

---

## Stage 5: Results Collector

**Input**: stdout/stderr bytes, artifacts, `RunnerHandle` from Runner. `ExecutionMetrics` from Monitor.

**Output**: `ExecutionResult` with parsed output, artifacts, and metrics.

**Behavior**:

1. **Output collection**: All stdout/stderr bytes are collected into a contiguous buffer. If `max_output_bytes` is exceeded, collection stops and the output is flagged as truncated.

2. **Output parsing**: If output parsing is enabled, the collector attempts to parse stdout bytes using registered parsers:
   - **JSON parser**: Attempts `serde_json::from_slice`. If parsing succeeds, `parsed_output` is set.
   - **YAML parser**: Attempts `serde_yaml::from_slice`. Falls back to raw.
   - **CSV parser**: Attempts CSV parsing. Produces array-of-objects.
   - **Raw parser**: No parsing; leaves `parsed_output` as `None`.

   If all parsers fail, `parsed_output` is set to `None` and `OutputParseFailed` event is published. The raw output is preserved.

3. **Artifact attachment**: Any artifacts produced by the execution (files written to designated output directories, WASM return values) are collected as `ExecutionArtifact` objects.

4. **Result assembly**: An `ExecutionResult` is assembled with:
   - Exit code from Runner
   - Output buffers
   - Parsed output (if any)
   - Artifacts
   - Metrics from Monitor
   - Error info (if state is Failed/TimedOut/Cancelled)
   - Timestamps

5. **Output enrichment** (future, Phase 9): The result may be passed through a Perception enrichment stage for entity resolution, anomaly scoring, or context attachment.

6. **Result storage**: The `ExecutionResult` is converted to `ExecutionHistory` and stored in Memory (episodic store).

**Events emitted**: `ExecutionResultRouted`, `ExecutionArtifactStored`

---

## Stage 6: Output Router

**Input**: `ExecutionResult` from Results Collector.

**Output**: Delivered results to configured destinations.

**Behavior**:

1. **Rule matching**: All registered `RoutingRule` instances are evaluated against the execution result. Rules whose `condition` matches are collected.

2. **Deduplication**: If multiple rules match the same destination, only the first match is used. Rules are evaluated in registration order.

3. **Route execution**: Each matched rule's destination is targeted:
   - **Caller**: Result is sent through the Coordinator back to the Brain (via EventBus or direct channel).
   - **Memory**: Result is stored in the Memory Platform (episodic store) via `MemoryStore::store()`.
   - **Brain**: Result is published as an event for Brain Reflection.
   - **Pipe**: Result is passed as input to another `ToolRequirement` (chained execution).
   - **FileSystem**: Result is written to a file via OSAL `FileSystemProvider::write()`.
   - **EventBus**: Result is published as a typed event on the EventBus.
   - **Log**: Result summary is written to the logger.
   - **Broadcast**: Result is routed to all contained destinations.

4. **Failure handling**: If a route fails:
   - Transient failures (network timeout, memory write contention) are retried once.
   - Permanent failures (invalid path, permission denied) are logged and the route is skipped.
   - A `RouteResult` with `success: false` and error details is included in the routing report.

5. **Acknowledgement**: The Coordinator is notified of completed routing. The execution is finalized.

**Events emitted**: `ExecutionResultRouted`, `ExecutionResultRouteFailed`

---

## Stage 7: Recovery Manager

**Input**: `ExecutionResult` with terminal state (Failed, TimedOut, Cancelled).

**Output**: Recovery decision (retry, rollback, escalate) and execution of recovery.

**Behavior**:

### Retry Flow

```
ExecutionResult received with Failed/TimedOut state
         │
         v
  ┌─ Is execution registered for recovery? ── No ──→ Finalize
  │
  Yes
  │
  v
  ┌─ Is RetryPolicy present? ── No ──→ Check rollback
  │
  Yes
  │
  v
  ┌─ Are retries remaining? ── No ──→ Check rollback
  │
  Yes
  │
  v
  Compute backoff: base_ms × multiplier^attempt + jitter
  │
  v
  Publish ExecutionRetrying event
  │
  v
  Wait backoff duration (tokio::time::sleep)
  │
  v
  Create new ExecutionPlan from original (same plan, new execution_id)
  │
  v
  Submit plan to Dispatcher for re-execution
  │
  v
  ┌─ Re-execution succeeds? ── Yes ──→ Finalize (retry resolved)
  │
  No
  │
  v
  Go to retry evaluation
```

### Rollback Flow

```
Rollback triggered (no retries, policy says rollback, or cancellation requires cleanup)
         │
         v
  ┌─ RollbackPlan exists? ── No ──→ Finalize (no cleanup needed)
  │
  Yes
  │
  v
  Publish ExecutionRollingBack event
  │
  v
  For each CompensationStep in order:
  │
  ├── ExecuteTool:   Submit new ToolRequirement for compensation action
  ├── RestoreSnapshot: Call Memory snapshot restore
  ├── DeleteCreated:   Call OSAL FileSystemProvider::delete
  └── Notify:          Publish notification event
  │
  v
  ┌─ All steps succeeded? ── Yes ──→ Publish ExecutionRolledBack, Finalize
  │
  No (step failed)
  │
  v
  Evaluate OnRollbackFailure:
  │
  ├── Escalate:         Publish ExecutionEscalated, persist full context
  ├── IgnoreAndContinue: Log warning, continue to next step
  └── Retry(n, ms):     Retry step with backoff, up to n times
  │
  v
  ┌─ All steps resolved? ── Yes ──→ Finalize (may have partial rollback)
  │
  No
  │
  v
  Publish ExecutionEscalated
  Persist execution context for manual intervention
```

### Cancellation Flow

```
Cancel requested for ExecutionId
         │
         v
  ┌─ Plan in queue (Pending/Queued)?
  │  Yes → Remove from queue, mark Cancelled, finalize
  │
  ├─ Plan dispatching?
  │  Yes → Abort dispatch, mark Cancelled, finalize
  │
  └─ Plan running?
     │
     v
     Call Runner::cancel()
     │
     v
     Send SIGTERM (subprocess) / Drop instance (WASM) / Stop container
     │
     v
     Wait grace period
     │
     v
     ┌─ Still alive? ── Yes ──→ Send SIGKILL
     │
     No
     │
     v
     Collect partial output and state
     │
     v
     Publish ExecutionCancelled event
     │
     v
     ┌─ RollbackPlan exists? ── Yes ──→ Execute rollback flow
     │
     No
     │
     v
     Finalize
```

**Events emitted**: `ExecutionRetrying`, `ExecutionRollingBack`, `ExecutionRolledBack`, `ExecutionEscalated`

---

## Pipeline Wiring

The Coordinator wires the pipeline at startup:

```
Planner ──mpsc──→ Dispatcher ──mpsc──→ Runner ──mpsc──→ Monitor ──mpsc──→ Results ──mpsc──→ Recovery
```

Each `mpsc` channel has configurable capacity (default: 10,000).

The Monitor also receives direct input from the Runner (via the RunnerHandle) and from a Tokio timer. The Recovery Manager receives direct input from the Results Collector (via the ExecutionResult).

### Bypass Mode

If any stage fails repeatedly (configurable threshold: 5 failures within 60 seconds), the Coordinator places that stage into bypass mode:

- **Planner bypass**: The `ToolRequirement` is forwarded directly to the Dispatcher with a default `ExecutionPlan` that uses the first matching `ToolCandidate`. Input validation is skipped.
- **Dispatcher bypass**: The `ExecutionPlan` is forwarded directly to the Runner. Concurrency limiting and queueing are skipped.
- **Runner bypass**: Not possible — execution must happen somewhere. The Coordinator escalates.
- **Monitor bypass**: No timeout or resource enforcement. Execution runs without monitoring.
- **Results bypass**: Raw output is routed without parsing or enrichment. `parsed_output` is always `None`.
- **Recovery bypass**: No retry or rollback. Failures are final.

When the failing stage recovers (health check passes), bypass mode is automatically disabled.

---

## Thread Model

| Stage | Concurrency |
|---|---|
| Planner | Synchronous on Coordinator task |
| Dispatcher | `RwLock<BinaryHeap>` for queue. Dispatch on Coordinator task. `Notify` for queue waiters. |
| SubprocessRunner | Per-execution `tokio::task` for child process. Separate task for stdout/stderr pipe reads. |
| WasmRunner | WASM compilation on `spawn_blocking`. Instance execution on async task. |
| ContainerRunner | Async HTTP calls to OCI runtime. Health poll on `tokio::time::interval`. |
| Monitor | Per-execution Tokio timer. Resource sampling on `tokio::time::interval`. |
| Results | Synchronous on Runner's completion callback. |
| Recovery | `RwLock<HashMap<ExecutionId, RecoveryState>>`. Retry backoff via `tokio::time::sleep`. Rollback actions dispatched through pipeline. |

---

## Data Flow: Full Execution Lifecycle

```
Brain                   Coordinator            Planner            Dispatcher
  |                         |                     |                   |
  | 1. brain.decision.made  |                     |                   |
  |---(EventBus)----------->|                     |                   |
  |                         | 2. submit           |                   |
  |                         |---(ToolRequirement)─>|                   |
  |                         |                     | 3. resolve        |
  |                         |                     | 4. validate       |
  |                         |<--(ExecutionPlan)----|                   |
  |                         |                     |                   |
  |                         | 5. submit           |                   |
  |                         |---------------------|--(ExecutionPlan)──>|
  |                         |                     |                   |
  |                         |<--(ExecutionHandle)--|-------------------|
  | 6. handle to Brain      |                     |                   |
  |<--(ExecutionHandle)-----|                     |                   |
  |                         |                     |                   |

Dispatcher            Runner               Monitor              Results
  |                     |                     |                     |
  | 7. dispatch         |                     |                     |
  |---(ExecutionPlan)──>|                     |                     |
  |                     | 8. spawn            |                     |
  |                     |---(OSAL)            |                     |
  |                     |                     |                     |
  |                     | 9. stdout/stderr    |                     |
  |                     |---(mpsc stream)─────|─(monitor, buffer)──>|
  |                     |                     |                     |
  |                     | 10. process exits   |                     |
  |                     | 11. collect rusage  |                     |
  |                     |                     |                     |
  |                     | 12. notify          |                     |
  |                     |---(ExecutionResult)─────────────────────>|
  |                     |                     |                     |
  |                     |                     |                     | 13. parse
  |                     |                     |                     | 14. enrich
  |                     |                     |                     | 15. route
  |                     |                     |                     |
  |                     |                     |                     | 16. store to Memory
  |                     |                     |                     | 17. publish completed
  |                     |                     |                     |
  |                     |                     |                     | 18. recovery check
  |                     |                     |                     |

Recovery              Coordinator            Brain
  |                     |                     |
  | 19. retry/rollback  |                     |
  |                     |                     |
  | 20. publish result  |                     |
  |                     |---(EventBus)────────>|
  |                     |                     | 21. reflection
```

---

## References

- [Execution Platform Architecture](architecture/execution.md)
- [Execution Platform Interfaces](interfaces/execution.md)
- [Execution Platform Configuration](configuration/execution.md)
- [RFC-0005: Execution Platform](rfc/RFC-0005-execution-platform.md)
- [Perception Processing Pipeline](perception-pipeline.md) — pipeline design patterns
- [Core Platform Architecture](architecture/core.md)
- [Runtime Platform Architecture](architecture/runtime.md)
- [OSAL Architecture](architecture/osal.md)
- [Memory Platform Architecture](architecture/memory.md)
- [Brain Platform Architecture](architecture/brain.md)
