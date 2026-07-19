# Runtime Platform Module

**Module:** `ai_os_runtime`
**Status:** Active (Phase 2)
**Crate:** `runtime/ai_os_runtime/`

## Purpose

The Runtime Platform module implements the execution layer of the AI-native OS. It manages task scheduling, supervision, session scoping, contextual propagation, phase-gated state transitions, per-task resource accounting, and role-based permission enforcement. This module transforms the static abstractions from Core into a live, supervised, multi-tenant execution environment.

## Responsibilities

- Schedule tasks on a priority queue and dispatch to the Tokio runtime.
- Supervise running tasks with configurable restart policies.
- Scopes tasks into user sessions with permission bindings.
- Tracks task lifecycle through a canonical state machine.
- Propagates distributed-trace context across async boundaries.
- Governs system-level phase transitions with validation.
- Accounts and limits resource consumption per task.
- Enforces fine-grained permissions based on role.

## Public Interfaces

### `Scheduler`

Located at `ai_os_runtime::scheduler::Scheduler`. Backed by a `BinaryHeap<ScheduledTask>` with `Ord` reversed so higher-priority tasks dequeue first.

```rust
use ai_os_runtime::scheduler::{Scheduler, Priority, SchedulerStats};

let mut sched = Scheduler::new();
sched.enqueue("task-a", Priority::Critical);
sched.enqueue("task-b", Priority::Low);

let stats = sched.stats();
assert_eq!(stats.pending, 2);
assert_eq!(sched.peek(), Some(&"task-a".into()));
assert_eq!(sched.dequeue(), Some("task-a".into()));
```

`SchedulerStats` exposes `pending`, `running`, `completed`, and `failed` counters. The `remove` method cancels a queued task by ID.

### `Supervisor`

Located at `ai_os_runtime::supervisor::Supervisor`. Implements failure recording and restart decisions.

```rust
use ai_os_runtime::supervisor::{Supervisor, RestartPolicy, SupervisionStatus};

let mut sup = Supervisor::new(RestartPolicy::Always);
sup.record_failure("task-1");

let status = sup.status("task-1");
assert_eq!(status, SupervisionStatus::Running);

if sup.should_restart("task-1") {
    sup.clear("task-1");
}
```

`RestartPolicy` variants:
- `Never` -- do not restart.
- `Always` -- restart unconditionally.
- `OnFailure(n, window)` -- restart up to `n` times per `window` duration.
- `Escalating(initial, max_backoff)` -- restart with exponential backoff.

When a task is restarted, the supervisor publishes a `TaskRestarting` event (see below).

### `SessionManager`

Located at `ai_os_runtime::session::SessionManager`. CRUD over sessions stored in an `RwLock<HashMap<SessionId, Session>>`.

```rust
use ai_os_runtime::session::{SessionManager, Session, Permission};
use std::sync::Arc;

let mut sm = SessionManager::new();
let session = Session::new("alice", "user");
let id = sm.create(session);

if let Some(s) = sm.get(&id) {
    assert_eq!(s.role, "user");
}
sm.delete(&id);
```

Each `Session` carries a `Vec<Permission>` that gates task submission and resource access.

### `TaskManager`

Located at `ai_os_runtime::task::TaskManager`. Drives a state machine: `Pending -> Running -> Completed | Failed | Cancelled`.

```rust
use ai_os_runtime::task::{TaskManager, TaskState, Priority};

let mut tm = TaskManager::new();
let id = tm.create("cmd", Priority::Critical, vec![]);
assert_eq!(tm.state(&id), Some(&TaskState::Pending));

tm.transition(&id, TaskState::Running).unwrap();
tm.transition(&id, TaskState::Completed).unwrap();
```

`Priority` is an ordinal enum: `Critical = 0`, `High = 25`, `Normal = 50`, `Low = 75`, `Background = 100`. Lower values are higher priority.

### `ContextManager`

Located at `ai_os_runtime::context::ContextManager`. Uses `tokio::task_local!` to propagate `Context` across async boundaries.

```rust
use ai_os_runtime::context::{Context, ContextManager};

let ctx = Context::new("trace-1", "span-1", None);
ContextManager::scope(ctx, async {
    let current = ContextManager::current();
    assert_eq!(current.trace_id, "trace-1");

    let child = current.child("span-2");
    assert_eq!(child.parent_span_id, Some("span-1".into()));
}).await;
```

`Context` fields: `trace_id`, `span_id`, `parent_span_id`, `metadata: HashMap<String, String>`.

### `StateMachine`

Located at `ai_os_runtime::phase::StateMachine`. Governs the `RuntimePhase` enum.

```rust
use ai_os_runtime::phase::{StateMachine, RuntimePhase};

let mut sm = StateMachine::new();
assert_eq!(sm.phase(), RuntimePhase::Booting);

sm.transition_to(RuntimePhase::CoreReady).unwrap();
sm.transition_to(RuntimePhase::RuntimeReady).unwrap();
```

Phases (in order):
1. `Booting`
2. `CoreReady`
3. `RuntimeReady`
4. `Active`
5. `Degraded`
6. `Shutdown`

Transitions are validated against a table. For example, `Active -> Booting` is rejected. The table ensures monotonic forward progress except that `Active -> Degraded` and `Degraded -> Active` are both allowed.

### `ResourceManager`

Located at `ai_os_runtime::resources::ResourceManager`. Tracks per-task `ResourceUsage` and enforces `ResourceLimits`.

```rust
use ai_os_runtime::resources::{ResourceManager, ResourceUsage, ResourceLimits};

let mut rm = ResourceManager::new();
let limits = ResourceLimits { max_cpu_ms: 5000, max_memory_kb: 65536 };

rm.register("task-1", limits);
rm.update("task-1", ResourceUsage { cpu_ms: 1200, memory_kb: 8192, io_bytes: 4096 });

let exceeded = rm.check("task-1");
assert!(!exceeded);
```

`ResourceUsage::saturating_add` clamps CPU at `u64::MAX` to prevent overflow. The `check` method compares current usage against the registered limits.

### `PermissionChecker`

Located at `ai_os_runtime::auth::PermissionChecker`. Associates roles with `Permission` enums.

```rust
use ai_os_runtime::auth::{PermissionChecker, Permission, Role};

let checker = PermissionChecker::new();
assert!(checker.has_permission("admin", Permission::Delete));
assert!(!checker.has_permission("readonly", Permission::Write));
```

Three built-in roles: `admin`, `user`, `readonly`. `Permission` variants: `Admin`, `Read`, `Write`, `Execute`, `Delete`. Roles are ordered hierarchically: `admin` inherits all, `user` inherits `Read+Write+Execute`, `readonly` inherits only `Read`.

## Dependencies

| Dependency | Scope | Purpose |
|---|---|---|
| `ai_os_core` | internal | EventBus, Service trait, logging |
| `tokio` | runtime | Async execution, task-local, timers |
| `std::collections::BinaryHeap` | stdlib | Priority queue for scheduler |
| `std::sync::RwLock` | stdlib | Concurrent maps in session/task/resource managers |
| `tracing` | logging | Task lifecycle spans |

## Events Published

| Event | Trigger | Payload |
|---|---|---|
| `runtime::TaskEnqueued` | `Scheduler::enqueue` | `(TaskId, Priority)` |
| `runtime::TaskDequeued` | `Scheduler::dequeue` | `TaskId` |
| `runtime::TaskStarted` | `TaskManager::transition -> Running` | `(TaskId, SessionId)` |
| `runtime::TaskCompleted` | `TaskManager::transition -> Completed` | `(TaskId, Duration)` |
| `runtime::TaskFailed` | `TaskManager::transition -> Failed` | `(TaskId, String)` -- error message |
| `runtime::TaskCancelled` | `TaskManager::cancelled` | `TaskId` |
| `runtime::TaskRestarting` | `Supervisor::should_restart` | `(TaskId, u32)` -- attempt number |
| `runtime::PhaseChanged` | `StateMachine::transition_to` | `(RuntimePhase, RuntimePhase)` |

## Events Consumed

| Event | Consumer | Action |
|---|---|---|
| `core::ServiceStateChanged` | `Scheduler` | Pause/resume scheduling based on service health |
| `perception::AlertEvent` | `Supervisor` | Escalate restarts for alert-triggered failures |
| `system::ResourceExhausted` | `ResourceManager` | Pre-emptively cancel low-priority tasks |

## Thread Model

All managers (`SessionManager`, `TaskManager`, `ResourceManager`) are `Send + Sync` via `Arc<RwLock<T>>`. The scheduler's `BinaryHeap` is wrapped in a `tokio::sync::Mutex` because heap operations (push/pop) mutate internal state and the critical section is short.

- `ContextManager` uses `tokio::task_local!` which is fiber-local; context does not cross threads implicitly.
- `StateMachine` is `Send + Sync` behind a `RwLock`; phase transitions are rare (at most O(phase_count) per boot cycle) so contention is negligible.
- `PermissionChecker` is immutable after construction and wrapped in `Arc`; no locking required.

## Lifecycle

```
Runtime phases (monotonic, validated):
  Booting -> CoreReady -> RuntimeReady -> Active <-> Degraded -> Shutdown

Task lifecycle per task:
  Pending -> Running -> Completed
                    | -> Failed
                    | -> Cancelled
```

The `StateMachine` must reach `RuntimeReady` before the `Scheduler` starts draining its queue. The `Shutdown` phase triggers `TaskManager::cancel_all()` and awaits graceful completion with a hard timeout.

## Error Handling

Error types (all in `ai_os_runtime::error`):

- `ScheduleError`: `QueueFull`, `DuplicateId`, `NotReady`.
- `TaskError`: `InvalidTransition`, `AlreadyExists`, `NotFound`, `SessionMismatch`.
- `SessionError`: `DuplicateSession`, `SessionNotFound`, `PermissionDenied`.
- `PhaseError`: `InvalidTransition`, `AlreadyInPhase`.
- `ResourceError`: `LimitExceeded`, `TaskNotFound`.

Recovery: Invalid state transitions return `Err` and leave state unchanged. Permission denial returns `PermissionDenied` to the caller. Resource overages trigger a `TaskCancelled` event; the supervisor may choose to restart.

## Configuration

```toml
[scheduler]
max_pending = 10_000

[supervisor]
default_policy = "always"

[session]
default_role = "readonly"
session_ttl_secs = 3600

[resources]
default_max_cpu_ms = 10_000
default_max_memory_kb = 262_144
```

Loaded from `aios-runtime.toml`. All values have safe defaults.

## Testing Strategy

- **Unit tests**: Scheduler priority ordering (enqueue 100 tasks with random priorities, verify dequeue order). StateMachine invalid transition rejection. PermissionChecker role hierarchy.
- **Integration tests**: Supervisor restart loop with mocked clock. TaskManager full lifecycle through all states. ResourceManager clamp and limit enforcement.
- **Fuzz tests**: SessionManager CRUD with concurrent readers/writers (via `loom`).
- **Benchmarks**: Scheduler enqueue/dequeue throughput at capacity. RwLock contention in TaskManager under high-frequency transition calls.

## Future Extensions

- Distributed scheduling with a consistent hash ring for multi-node deployments.
- Adaptive supervision: machine-learning-based failure prediction.
- Hierarchical resource accounting (cgroup v2 integration).
- Session quotas and rate limiting per role.
- Preemptive task suspension and resumption (suspend-to-storage for long-running tasks).
- Phase-gated health checks: require n/3 replicas healthy before transitioning to `Active`.
