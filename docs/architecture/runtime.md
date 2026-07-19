# Runtime Platform Deep Dive

## Purpose

The Runtime Platform is the operational backbone of AI-native OS. It provides the subsystems that manage task lifecycle, scheduling, supervision, session context, resource accounting, authorization, and runtime state transitions. Every module above this layer depends on the Runtime Platform for execution, scoping, and policy enforcement.

The Runtime Platform (Phase 3) builds directly on the [Core Platform](core.md). It consumes Core's EventBus, Service trait, and Logger interfaces. It exposes its own composite `Runtime` struct that collects all subsystems into a single `Service` implementation that can be registered with Core's `LifecycleManager`.

---

## Responsibilities

- **Scheduler**: Maintain a priority-ordered queue of tasks and provide peek/dequeue/enqueue/remove operations.
- **Supervisor**: Monitor task execution and apply configurable restart policies (Never, OnFailure, Always) with retry tracking.
- **SessionManager**: Create, query, list, and destroy sessions. Each session carries a `PermissionContext` and metadata.
- **TaskManager**: Create tasks, track state transitions (Pending -> Running -> Completed/Failed/Cancelled), and provide session-scoped queries.
- **ContextManager**: Generate and propagate trace/span context via `tokio::task_local!`. Support child context creation for nested operations.
- **StateMachine**: Enforce the runtime phase state machine (Created -> Initializing -> Running -> Draining -> Stopped/Failed) with 8 valid transitions.
- **ResourceManager**: Track per-task CPU and memory usage. Enforce aggregate limits with saturating arithmetic.
- **PermissionChecker**: Evaluate role-based access control against `PermissionContext` objects. Pre-built roles: admin, user, readonly.
- **Runtime**: Composite facade that wires all subsystems together and implements `Service` for lifecycle management.

---

## Subsystem Deep Dives

### Scheduler (`scheduler::PriorityScheduler`)

The scheduler uses `std::collections::BinaryHeap<ScheduledTask>` as its backing data structure. A custom `Ord` implementation ensures tasks are ordered by priority (ascending numeric value = higher urgency) with FIFO tiebreaking on `created_at`.

```rust
fn cmp(&self, other: &Self) -> Ordering {
    other.handle.priority.cmp(&self.handle.priority)
        .then_with(|| other.handle.created_at.cmp(&self.handle.created_at))
}
```

Because `BinaryHeap` is a max-heap, the `cmp` function reverses priority comparison: lower `Priority` values (higher urgency) are ordered first. Equal priorities fall through to creation time comparison (earlier tasks first).

**Priority Constants**: `CRITICAL = 0`, `HIGH = 10`, `NORMAL = 50`, `LOW = 100`. Arbitrary `u8` values are supported for fine-grained control.

**Operations**:
- `enqueue`: O(log n) push onto the heap.
- `dequeue`: O(log n) pop from the heap. Returns `None` if empty.
- `peek`: O(1) reference to the top element without removal.
- `remove`: O(n) linear scan and retain. Returns error if task not found.
- `stats`: Returns atomic enqueue/dequeue counters and current queue depth.

### Supervisor (`supervisor::DefaultSupervisor`)

The supervisor maintains a `HashMap<TaskId, SupervisionStatus>` behind an `RwLock`. Each supervised task has:

- A `RestartPolicy`: `Never`, `OnFailure { max_retries }`, or `Always { max_retries }`.
- A retry counter incremented on each `record_failure` call.
- Timestamp and reason for the last failure.
- An `is_active` flag.

`record_failure` returns `bool` indicating whether the task should be restarted based on its policy. The `cancel_supervision` operation removes a task from supervision entirely.

When a restart is indicated, the supervisor publishes a `TaskRestarting` event (`"runtime.task_restarting"`) so the TaskManager and Scheduler can re-enqueue the task.

### SessionManager (`session::DefaultSessionManager`)

Sessions are stored in a `HashMap<SessionId, Session>` behind an `RwLock`. Each `Session` has:

- `id`: UUID v4-based `SessionId`
- `state`: Active, Idle, Draining, or Destroyed
- `created_at`: timestamp
- `metadata`: arbitrary key-value pairs
- `permission_ctx`: a `PermissionContext` initialized with the `user` role by default

`create_session` returns the `Session` value (cloned from the store). `destroy_session` removes by ID and returns an error if the session does not exist. `list_sessions` returns all sessions; `session_count` returns the count.

Sessions are immutable after creation (except state). The permission context is fixed at creation time.

### TaskManager (`task::DefaultTaskManager`)

Tasks are stored in a `HashMap<TaskId, Task>` behind an `RwLock`. Each `Task` has:

- `id`: UUID v4-based `TaskId`
- `session_id`: optional session association
- `state`: Pending, Running, Completed, Failed, Cancelled
- `context`: propagation context with trace/span IDs
- `created_at`, `started_at`, `completed_at`: timestamps
- `metadata`: arbitrary key-value pairs

**State Machine**:

```
Pending
   |
   v
Running ----+----> Completed
   |         |
   |         +----> Failed
   |
   +----------> Cancelled
```

`update_state` enforces no validation on transitions (the type system does not constrain transitions; this is a future extension). It automatically sets `started_at` when transitioning to `Running` and `completed_at` when transitioning to any terminal state.

`list_tasks_by_session` provides session-scoped task queries. `pending_count` counts tasks in the `Pending` state.

### ContextManager (`context::DefaultContextManager`)

Context propagation uses `tokio::task_local!` to store the current `Context` value:

```rust
tokio::task_local! {
    static CURRENT_CONTEXT: Context;
}
```

The `Context` struct carries:
- `trace_id`: UUID v4, stable across an entire operation chain
- `span_id`: UUID v4, unique per unit of work
- `parent_span_id`: optional link to the parent span
- `metadata`: arbitrary key-value pairs

`run_with_context(ctx, f)` executes a future within a given context scope. `with_new_context(f)` creates a root context. `with_child_context(metadata, f)` creates a child span of the current context.

The `ContextManager` trait's `current()` method reads the thread-local value (falling back to a new root context if none is set). `create_child(parent)` generates a new `span_id` while preserving the `trace_id`.

### StateMachine (`state::DefaultRuntimeState`)

The runtime phase state machine has 6 states and 8 valid transitions:

```
       +-----------+
       |  Created  |
       +-----+-----+
             | init
       +-----v-------+
       | Initializing |
       +-----+--------+
             | start        +--------+
       +-----v----+    +---->Running |
       | Running  |    |   +----+----+
       +-----+----+    |        | drain
             | drain   +--------+
       +-----v-------+
       |  Draining   |---------+ (auto-restart)
       +-----+-------+
             | stop
       +-----v-----+
       |  Stopped  |
       +-----------+

Failed: reachable from Initializing, Running, or Draining (terminal).
```

Transitions are validated against a static `TRANSITIONS` slice. Invalid transitions return `RuntimeError::State`. The `can_transition_to` method allows callers to check before attempting.

The `RuntimePhaseChanged` event (`"runtime.phase_changed"`) is published on every transition.

### ResourceManager (`resource::DefaultResourceManager`)

Tracks `ResourceUsage` per task:

```rust
pub struct ResourceUsage {
    pub cpu_percent: f64,    // 0.0 - 100.0
    pub memory_bytes: u64,
}
```

`saturating_add` clamps `cpu_percent` to a maximum of 100.0 and uses `u64::saturating_add` for `memory_bytes`.

`check_limits(additional)` projects current usage plus the additional amount against `ResourceLimits` (default: 100% CPU, 1 GB memory, 100 tasks). Returns an error if either limit would be exceeded.

`track` inserts or overwrites per-task usage. `reset` removes a task's tracking entry. `total_usage` sums across all tracked tasks.

### PermissionChecker (`permission::DefaultPermissionChecker`)

Role-based authorization with three pre-built roles:

| Role | Permissions |
|---|---|
| `admin` | Read, Write, Execute, Admin |
| `user` | Read, Write |
| `readonly` | Read |

The `check(ctx, required)` method returns `Ok(())` if the context has all required permissions, or `RuntimeError::PermissionDenied` otherwise. Permission is set-based (`HashSet`) for O(1) lookup. The context also carries role strings for more flexible authorization patterns.

### Runtime Facade (`runtime::Runtime`)

The `Runtime` struct collects all subsystem `Arc<dyn Trait>` references into a single unit. It implements the Core `Service` trait:

- `start()`: Transitions `Created -> Initializing -> Running`, publishing `RuntimePhaseChanged` events at each step.
- `stop()`: Transitions `Running -> Draining -> Stopped`, publishing events at each step.

The `Runtime` is designed to be registered with Core's `LifecycleManager` so it starts and stops alongside other platform services.

---

## Public Interfaces

Each subsystem exposes a trait. Implementations are the `Default*` structs.

| Subsystem | Trait | Key Methods |
|---|---|---|
| Scheduler | `Scheduler` | `enqueue`, `dequeue`, `peek`, `remove`, `stats` |
| Supervisor | `Supervisor` | `supervise`, `cancel_supervision`, `record_failure`, `status`, `list_supervised` |
| SessionManager | `SessionManager` | `create_session`, `get_session`, `destroy_session`, `list_sessions` |
| TaskManager | `TaskManager` | `create_task`, `get_task`, `update_state`, `list_tasks`, `list_tasks_by_session` |
| ContextManager | `ContextManager` | `current`, `create_child` |
| StateMachine | `RuntimeStateMachine` | `phase`, `transition`, `can_transition_to`, `is_running` |
| ResourceManager | `ResourceManager` | `track`, `usage`, `total_usage`, `check_limits`, `reset` |
| PermissionChecker | `PermissionChecker` | `check` |

---

## Dependencies

| Crate | Purpose |
|---|---|
| `ai-os-core` | EventBus, Service, Logger, CoreError |
| `tokio` | `task_local!` for context propagation, async runtime |
| `serde` / `serde_json` | Event serialization for task/session/state types |
| `chrono` | Timestamps on tasks, sessions, supervision events |
| `uuid` | ID generation for TaskId, SessionId |
| `thiserror` | `RuntimeError` enum derivation |
| `async-trait` | Async trait methods |
| `ai-os-core` events | `Event` trait implementations for runtime events |

---

## Events Published

| Event | Type String | Source | Trigger |
|---|---|---|---|
| `RuntimePhaseChanged` | `runtime.phase_changed` | StateMachine | Runtime phase transition |
| `SessionCreated` | `runtime.session_created` | SessionManager | New session |
| `SessionDestroyed` | `runtime.session_destroyed` | SessionManager | Session destroyed |
| `TaskCreated` | `runtime.task_created` | TaskManager | New task created |
| `TaskStarted` | `runtime.task_started` | TaskManager | Task transitions to Running |
| `TaskCompleted` | `runtime.task_completed` | TaskManager | Task transitions to Completed |
| `TaskFailed` | `runtime.task_failed` | TaskManager | Task transitions to Failed |
| `TaskCancelled` | `runtime.task_cancelled` | TaskManager | Task transitions to Cancelled |
| `TaskRestarting` | `runtime.task_restarting` | Supervisor | Task restart triggered by policy |
| `ContextCreated` | `runtime.context_created` | ContextManager | Child context created |

---

## Events Consumed

The Supervisor subscribes to `TaskFailed` events. All other runtime subsytems are polled or invoked directly by the `Runtime` facade or by upper-layer modules. No runtime subsystem subscribes to events from other runtime subsystems by default -- this is by design to avoid circular event dependencies.

Upper-layer modules (System, Memory, Brain) subscribe to runtime events as needed.

---

## Data Flow: Session -> Task -> Schedule -> Execute

```
User/Agent                    SessionManager               TaskManager
    |                              |                           |
    |  1. create_session()         |                           |
    |----------------------------->|                           |
    |   Session                    |                           |
    |<-----------------------------|                           |
    |                              |                           |
    |  2. create_task(session_id)  |                           |
    |--------------------------------------------------------->|
    |                              |         Task (Pending)    |
    |<---------------------------------------------------------|
    |                              |                           |

    TaskManager              Scheduler              Supervisor
    |                           |                       |
    |  3. enqueue(handle)       |                       |
    |-------------------------->|                       |
    |                           |                       |
    |  4. dequeue()             |                       |
    |   (Tokio worker)          |                       |
    |-------------------------->|                       |
    |   TaskHandle              |                       |
    |<--------------------------|                       |
    |                           |                       |
    |  5. update_state(Running) |                       |
    |  6. publish(TaskStarted)  |                       |
    |                           |                       |
    |  7. execute task          |                       |
    |                           |                       |
    |  8. update_state(Completed|Failed)                |
    |  9. publish(TaskCompleted |TaskFailed)            |
    |                           |                       |
    |                     [if TaskFailed:]              |
    |                           |                       |
    |  10. record_failure()     |                       |
    |-------------------------------------------------->|
    |                           |   should_restart?     |
    |<--------------------------------------------------|
    |                           |                       |
    |  11. [if yes:]            |                       |
    |      publish(TaskRestarting)                      |
    |      enqueue(handle)      |                       |
    |-------------------------->|                       |
```

---

## Thread Model

| Subsystem | Threading |
|---|---|
| `PriorityScheduler` | `RwLock<BinaryHeap>`. Short critical sections using `std::sync::RwLock`. |
| `DefaultSupervisor` | `RwLock<HashMap>`. `record_failure` under write lock. |
| `DefaultSessionManager` | `RwLock<HashMap>`. Reads under read lock, writes under write lock. |
| `DefaultTaskManager` | `RwLock<HashMap>` plus `AtomicU64` counter. Lock-free counter increment. |
| `DefaultContextManager` | Stateless. `tokio::task_local!` provides per-task storage. No locks. |
| `DefaultRuntimeState` | `RwLock<RuntimePhase>`. Single enum value. |
| `DefaultResourceManager` | `RwLock<HashMap>` for per-task usage + `RwLock<ResourceLimits>` for limits. |
| `DefaultPermissionChecker` | Stateless. No locks. `HashSet` lookups are O(1). |

All `RwLock` instances use `std::sync::RwLock` (not Tokio's) because critical sections are short.

---

## Lifecycle

The `Runtime` struct implements `Service` and follows the Core lifecycle:

1. **Construction**: `Runtime::new(event_bus, logger)` creates all subsystem `Default*` instances.
2. **Registration**: The `Runtime` is registered with Core's `LifecycleManager`.
3. **Start**: `LifecycleManager::start_all()` calls `Runtime::start()`, which:
   - Transitions state `Created -> Initializing`
   - Publishes `RuntimePhaseChanged`
   - Transitions state `Initializing -> Running`
   - Publishes `RuntimePhaseChanged`
4. **Running**: The runtime processes tasks, sessions, and events as they arrive.
5. **Stop**: `LifecycleManager::stop_all()` calls `Runtime::stop()`, which:
   - Transitions state `Running -> Draining`
   - Publishes `RuntimePhaseChanged`
   - Transitions state `Draining -> Stopped`
   - Publishes `RuntimePhaseChanged`

Subsystem instances are created eagerly but do not hold resources until used. The `start()` and `stop()` methods on `Runtime` manage the composite state machine only; individual subsystems do not have their own lifecycle hooks (they are pure data holders).

---

## Error Handling

`RuntimeError` is the unified error type with variants for each subsystem:

| Variant | Source | Condition |
|---|---|---|
| `Scheduler(String)` | PriorityScheduler | Lock poisoned, task not found in queue |
| `Supervisor(String)` | DefaultSupervisor | Lock poisoned, task not supervised |
| `Session(String)` | DefaultSessionManager | Lock poisoned, session not found |
| `Task(String)` | DefaultTaskManager | Lock poisoned, task not found |
| `Context(String)` | ContextManager | Reserved for future validation |
| `State(String)` | DefaultRuntimeState | Invalid state transition |
| `Resource(String)` | DefaultResourceManager | Lock poisoned, limit exceeded, task not tracked |
| `PermissionDenied(String)` | DefaultPermissionChecker | Missing required permissions |
| `Core(CoreError)` | All subsystems | Error from Core dependency |

All runtime errors are propagatable through the event system. The Supervisor specifically handles `TaskFailed` events (which may carry `RuntimeError` context) to apply restart policies.

---

## Future Extensions

- **Scheduler**: Add work-stealing between multiple priority queues. Add deadline-based scheduling with EDF (Earliest Deadline First) ordering.
- **Supervisor**: Add circuit breaker pattern (failure threshold within a time window triggers cooldown). Add exponential backoff between restarts.
- **SessionManager**: Add session timeouts, idle detection, and automatic cleanup. Add session event subscription for state changes.
- **TaskManager**: Add state transition validation (prevent Pending -> Completed without Running). Add task dependency graphs.
- **ContextManager**: Add OpenTelemetry exporter for distributed trace aggregation. Add baggage propagation for cross-service metadata.
- **StateMachine**: Add guards (condition functions) on transitions. Add entry/exit actions. Add hierarchical states.
- **ResourceManager**: Add I/O bandwidth tracking. Add GPU memory tracking. Add soft limits with warning events vs. hard limits with enforcement.
- **PermissionChecker**: Add policy engine integration (external policy file evaluation). Add attribute-based access control (ABAC) in addition to RBAC.

---

## References

- [Architecture Overview](overview.md)
- [Core Platform Deep Dive](core.md)
- [System Platform Blueprint](system.md)
- [Scheduler Source](../../runtime/src/scheduler/mod.rs)
- [Supervisor Source](../../runtime/src/supervisor/mod.rs)
- [Session Source](../../runtime/src/session/mod.rs)
- [Task Source](../../runtime/src/task/mod.rs)
- [Context Source](../../runtime/src/context/mod.rs)
- [State Source](../../runtime/src/state/mod.rs)
- [Resource Source](../../runtime/src/resource/mod.rs)
- [Permission Source](../../runtime/src/permission/mod.rs)
- [Runtime Facade Source](../../runtime/src/runtime.rs)
- [Design Principles](../principles.md)
