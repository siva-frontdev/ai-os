# ADR-0005: Runtime Platform Design

## Status

Accepted

## Date

2025-02-01

## Context

The AI-native OS platform needs a runtime substrate that manages the lifecycle of AI agents and provides the foundational services required by all higher-level modules. This runtime is the heart of the platform — it is what makes the platform an "operating platform" rather than just a library.

### Problem

AI agents in this platform are not simple request-response handlers. They are long-lived, stateful, asynchronous processes that:

- Have a defined lifecycle with discrete states (created, initialized, running, paused, failed, terminated) and valid transitions between them.
- Consume resources (CPU, memory, GPU memory, network bandwidth, concurrent task slots) that must be tracked, limited, and enforced.
- Require permissions to access system capabilities (filesystem paths, network endpoints, external APIs, tool execution).
- Need scheduling — the platform decides when an agent runs, for how long, and with what priority. Agents do not self-schedule.
- May fail and need supervision: automatic restart, exponential backoff, escalation to human operator, or termination.
- Operate within sessions that persist across interactions. A session maintains conversation history, user identity, tool state, and configuration.
- Carry context (conversation history, user identity, tool state, cross-event correlation data) that propagates through event chains.
- Support hierarchical task structures: an agent may spawn subtasks, each with its own resource budget and lifecycle.

Additionally, the runtime must support these functions while remaining modular, testable, and compliant with the clean architecture ([ADR-0002](./0002-clean-architecture.md)) and event-driven ([ADR-0004](./0004-event-driven.md)) patterns already established.

### Functional Breakdown

The runtime decomposes into eight distinct subsystems:

| # | Subsystem           | Primary Responsibility | Key Operations |
|---|---------------------|------------------------|----------------|
| 1 | Scheduler           | Decides which agent runs next | enqueue, dequeue, cancel, park, resume, stats |
| 2 | Supervisor          | Monitors agent health, handles failures | watch, unwatch, report_failure, health, list_watched |
| 3 | SessionManager      | Manages persistent sessions | create, get, update, delete, list, expire |
| 4 | TaskManager         | Manages discrete tasks within an agent | submit, cancel, status, list, await_completion |
| 5 | ContextManager      | Manages context propagation and window limits | push, pop, truncate, snapshot, merge |
| 6 | StateMachine        | Manages agent state transitions | transition, valid_transitions, current_state, force_state |
| 7 | ResourceManager     | Tracks and enforces resource limits | reserve, release, usage, limits, enforce |
| 8 | PermissionChecker   | Evaluates agent permissions | check, check_batch, roles_for, permitted_actions |

### Subsystem Interaction

The subsystems interact through the EventBus to coordinate agent lifecycle. A typical agent execution flow:

```
1. SessionManager receives CreateSession event
   -> publishes SessionCreated

2. StateMachine receives SessionCreated
   -> creates agent in Created state
   -> publishes AgentCreated

3. Scheduler receives AgentCreated
   -> enqueues agent in pending queue
   -> publishes AgentEnqueued

4. Scheduler dequeues agent (when resources available)
   -> calls ResourceManager.reserve()
   -> if successful, publishes AgentScheduled

5. StateMachine receives AgentScheduled
   -> transitions agent to Running state
   -> publishes AgentRunning

6. Agent executes tasks via TaskManager

7. On completion:
   -> TaskManager publishes TaskCompleted
   -> StateMachine transitions to Idle or Completed

8. On failure:
   -> Supervisor receives AgentFailed
   -> applies supervision policy (restart, escalate, terminate)
   -> publishes AgentRestart or AgentTerminated
```

### Cross-Cutting Concerns

**Lifecycle hooks**: The StateMachine supports lifecycle hooks that fire on state transitions:
- `on_enter(state, agent_id)` — Called when entering a state.
- `on_exit(state, agent_id)` — Called when exiting a state.
- `on_transition(from, to, agent_id)` — Called for every transition.

These hooks are implemented as EventBus publish calls, enabling other subsystems to react to state changes.

**Resource accounting model**: The ResourceManager uses a two-phase commit model:
1. `reserve(agent_id, resources)` — Reserves resources without consuming them. Returns a reservation ID.
2. `commit(reservation_id)` — Confirms the reservation and begins tracking consumption.
3. `release(agent_id)` — Releases all resources held by the agent.

This prevents race conditions where multiple agents compete for the same resources.

**Permission model**: The PermissionChecker implements role-based access control (RBAC):
- Roles are defined statically in policy files (YAML or TOML).
- Agents are assigned roles at creation time.
- Each action (read file, execute tool, access network) requires a specific permission.
- Permissions are checked synchronously during event handling (before the action is taken).
- Deny-by-default: if no rule explicitly permits an action, it is denied.

### Design Constraints

From prior ADRs, the runtime design must respect:

- **Crate isolation** ([ADR-0002](./0002-clean-architecture.md)): The `runtime/` crate depends on `core/` but not on any higher layer. Subsystems within `runtime/` communicate through the EventBus, not through direct function calls.
- **EventBus communication** ([ADR-0004](./0004-event-driven.md)): All cross-subsystem communication uses the EventBus. A subsystem publishes an event; interested subsystems handle it.
- **Rust trait constraints** ([ADR-0003](./0003-rust.md)): All subsystem traits must be `Send + Sync + dyn`-compatible for EventBus dispatch. No generic methods on these traits.
- **Async-first**: All runtime operations are async using Tokio. Blocking operations (filesystem I/O in PermissionChecker policy loading) are wrapped in `spawn_blocking`.

## Decision

The runtime platform is implemented as a set of eight subsystems, each defined as a Rust trait in the `runtime/` crate with a default implementation. The `Runtime` struct composes all eight subsystems and wires them to the EventBus.

### Trait Design

Each subsystem is defined as an `#[async_trait]` trait with `Send + Sync` supertraits:

```rust
/// Subsystem 1: Scheduler
/// Decides which agent runs next and for how long.
#[async_trait]
pub trait Scheduler: Send + Sync {
    /// Enqueue an agent with a given priority.
    async fn enqueue(&self, agent_id: AgentId, priority: Priority) -> Result<(), RuntimeError>;
    /// Dequeue the next agent to run (blocking if queue is empty).
    async fn dequeue(&self) -> Option<ScheduledAgent>;
    /// Cancel a previously enqueued agent.
    async fn cancel(&self, agent_id: AgentId) -> Result<(), RuntimeError>;
    /// Temporarily park an agent (move to hold state).
    async fn park(&self, agent_id: AgentId) -> Result<(), RuntimeError>;
    /// Resume a parked agent.
    async fn resume(&self, agent_id: AgentId) -> Result<(), RuntimeError>;
    /// Return current scheduler statistics.
    fn stats(&self) -> SchedulerStats;
}

/// Subsystem 2: Supervisor
/// Monitors agent health and applies failure policies.
#[async_trait]
pub trait Supervisor: Send + Sync {
    /// Start watching an agent with a supervision policy.
    async fn watch(&self, agent_id: AgentId, policy: SupervisionPolicy);
    /// Stop watching an agent.
    async fn unwatch(&self, agent_id: AgentId);
    /// Report an agent failure. The supervisor applies the policy.
    async fn report_failure(&self, agent_id: AgentId, failure: AgentFailure);
    /// Return current supervisor health.
    fn health(&self) -> SupervisorHealth;
    /// List all watched agents.
    async fn list_watched(&self) -> Vec<AgentId>;
}

/// Subsystem 3: SessionManager
/// Manages persistent agent sessions.
#[async_trait]
pub trait SessionManager: Send + Sync {
    async fn create(&self, config: SessionConfig) -> Result<Session, RuntimeError>;
    async fn get(&self, session_id: SessionId) -> Result<Session, RuntimeError>;
    async fn update(&self, session: Session) -> Result<(), RuntimeError>;
    async fn delete(&self, session_id: SessionId) -> Result<(), RuntimeError>;
    async fn list(&self, filter: SessionFilter) -> Result<Vec<Session>, RuntimeError>;
    async fn expire(&self, session_id: SessionId) -> Result<(), RuntimeError>;
}

/// Subsystem 4: TaskManager
/// Manages discrete tasks submitted by agents.
#[async_trait]
pub trait TaskManager: Send + Sync {
    async fn submit(&self, task: Task) -> Result<TaskId, RuntimeError>;
    async fn cancel(&self, task_id: TaskId) -> Result<(), RuntimeError>;
    async fn status(&self, task_id: TaskId) -> Result<TaskStatus, RuntimeError>;
    async fn list(&self, agent_id: AgentId) -> Result<Vec<TaskSummary>, RuntimeError>;
    async fn await_completion(&self, task_id: TaskId) -> Result<TaskResult, RuntimeError>;
}

/// Subsystem 5: ContextManager
/// Manages context propagation and window limits.
#[async_trait]
pub trait ContextManager: Send + Sync {
    async fn push(&self, agent_id: AgentId, entry: ContextEntry) -> Result<(), RuntimeError>;
    async fn pop(&self, agent_id: AgentId) -> Result<Option<ContextEntry>, RuntimeError>;
    async fn truncate(&self, agent_id: AgentId, max_entries: usize) -> Result<(), RuntimeError>;
    async fn snapshot(&self, agent_id: AgentId) -> Result<ContextSnapshot, RuntimeError>;
    async fn merge(&self, agent_id: AgentId, snapshot: ContextSnapshot) -> Result<(), RuntimeError>;
}

/// Subsystem 6: StateMachine
/// Manages agent state transitions.
#[async_trait]
pub trait StateMachine: Send + Sync {
    async fn transition(&self, agent_id: AgentId, to: AgentState) -> Result<(), RuntimeError>;
    async fn valid_transitions(&self, agent_id: AgentId) -> Result<Vec<AgentState>, RuntimeError>;
    async fn current_state(&self, agent_id: AgentId) -> Result<AgentState, RuntimeError>;
    async fn force_state(&self, agent_id: AgentId, state: AgentState) -> Result<(), RuntimeError>;
}

/// Subsystem 7: ResourceManager
/// Tracks and enforces resource limits.
#[async_trait]
pub trait ResourceManager: Send + Sync {
    async fn reserve(&self, agent_id: AgentId, resources: ResourceBudget) -> Result<ReservationId, RuntimeError>;
    async fn commit(&self, reservation_id: ReservationId) -> Result<(), RuntimeError>;
    async fn release(&self, agent_id: AgentId) -> Result<(), RuntimeError>;
    async fn usage(&self, agent_id: AgentId) -> Result<ResourceUsage, RuntimeError>;
    async fn limits(&self, agent_id: AgentId) -> Result<ResourceBudget, RuntimeError>;
    async fn enforce(&self, agent_id: AgentId) -> Result<(), RuntimeError>;
}

/// Subsystem 8: PermissionChecker
/// Evaluates agent permissions for actions.
#[async_trait]
pub trait PermissionChecker: Send + Sync {
    async fn check(&self, agent_id: AgentId, action: Action) -> Result<bool, RuntimeError>;
    async fn check_batch(&self, agent_id: AgentId, actions: Vec<Action>) -> Result<Vec<bool>, RuntimeError>;
    async fn roles_for(&self, agent_id: AgentId) -> Result<Vec<Role>, RuntimeError>;
    async fn permitted_actions(&self, agent_id: AgentId) -> Result<Vec<Action>, RuntimeError>;
}
```

### Default Implementations

Each trait has a default implementation in a separate submodule:

- `runtime/src/scheduler/default.rs` — Multi-level feedback queue with three priority bands. Agents are preempted after a configurable time quantum. Starvation prevention via priority boosting for long-waiting low-priority agents.
- `runtime/src/supervisor/default.rs` — Supports three policies: `RestartAlways` (infinite retries with exponential backoff), `RestartOnFailure` (max N retries, then escalate), `RestartNever` (terminate on first failure). Escalation publishes an `EscalationRequired` event for human operator notification.
- `runtime/src/session/default.rs` — In-memory session store with optional SQLite persistence. Sessions expire after a configurable TTL. Expired sessions trigger `SessionExpired` events.
- `runtime/src/task/default.rs` — Task queue with dependency tracking. Supports task chaining (output of task A is input to task B). Publish `TaskSubmitted`, `TaskStarted`, `TaskProgress`, `TaskCompleted`, `TaskFailed` events.
- `runtime/src/context/default.rs` — Ring buffer per agent for context entries. Supports configurable max context window (in tokens for LLM, in entries for tool calls). Window overflow triggers summarization request.
- `runtime/src/state_machine/default.rs` — Deterministic finite automaton with explicit transition matrix. Invalid transitions return `RuntimeError::InvalidTransition`. Supports lifecycle hooks via EventBus events.
- `runtime/src/resource/default.rs` — Token bucket per resource type. CPU tracked in millicores, memory in megabytes, GPU memory in megabytes. Hard limits for CPU and memory; advisory limits for GPU memory. Exceeded limits trigger `ResourceExceeded` events.
- `runtime/src/permission/default.rs` — RBAC with TOML policy file. Roles defined as sets of permitted action patterns. Default policy: deny all. Development mode policy: allow all.

### Runtime Composition

```rust
pub struct Runtime {
    pub scheduler: Arc<dyn Scheduler>,
    pub supervisor: Arc<dyn Supervisor>,
    pub session_manager: Arc<dyn SessionManager>,
    pub task_manager: Arc<dyn TaskManager>,
    pub context_manager: Arc<dyn ContextManager>,
    pub state_machine: Arc<dyn StateMachine>,
    pub resource_manager: Arc<dyn ResourceManager>,
    pub permission_checker: Arc<dyn PermissionChecker>,
    eventbus: Arc<dyn EventBus>,
}

impl Runtime {
    pub async fn new(config: RuntimeConfig, eventbus: Arc<dyn EventBus>) -> Result<Self, RuntimeError> {
        // 1. Create each subsystem with its config.
        let scheduler = Arc::new(DefaultScheduler::new(config.scheduler));
        let supervisor = Arc::new(DefaultSupervisor::new(config.supervisor));
        // ... (similar for remaining 6 subsystems)

        // 2. Wire event subscriptions.
        let runtime = Self { scheduler, supervisor, /* ... */ eventbus };

        // 3. Register all event handlers.
        runtime.register_handlers().await;

        // 4. Start background tasks.
        runtime.start_background_tasks().await;

        Ok(runtime)
    }

    async fn register_handlers(&self) {
        self.eventbus.subscribe::<AgentCreated, _>(self.scheduler.clone()).await;
        self.eventbus.subscribe::<AgentScheduled, _>(self.supervisor.clone()).await;
        // ... (all other event subscriptions)
    }
}
```

### Event Subscriptions

During initialization, each subsystem subscribes to relevant events:

| Subsystem           | Subscribes To                                         |
|---------------------|-------------------------------------------------------|
| Scheduler           | AgentCreated, AgentPaused, AgentResumed, AgentFailed  |
| Supervisor          | AgentScheduled, HeartbeatTimeout, AgentFailed, ResourceExceeded |
| SessionManager      | SessionCreateRequest, SessionDeleteRequest             |
| TaskManager         | TaskSubmitRequest, TaskCancelRequest                   |
| ContextManager      | ContextPushRequest, ContextTruncateRequest             |
| StateMachine        | AgentCreated, AgentScheduled, AgentCompleted, AgentFailed, AgentTerminated |
| ResourceManager     | ResourceReserveRequest, ResourceReleaseRequest          |
| PermissionChecker   | PermissionCheckRequest                                 |

## Consequences

### Positive

- **Clean separation of concerns**: Each subsystem has a single responsibility. Changes to scheduling policy (e.g., from MLFQ to EDF) do not affect permission logic or session management.
- **Testability**: Each subsystem can be tested in isolation with a mock EventBus and mock dependencies for other subsystems. The trait-based design enables dependency injection in tests.
- **Swap implementations**: The trait-based design allows replacing any subsystem independently. For example, the default scheduler (multi-level feedback queue) could be replaced with a completely different scheduling algorithm (e.g., earliest deadline first) without changing any other subsystem.
- **Parallel development**: Multiple developers can work on different subsystems simultaneously. The only coordination point is the EventBus event types, which are documented and reviewed.
- **EventBus alignment**: The subscription model maps naturally to the event-driven architecture. Subsystems react to events without knowing who produced them. New subsystems can be added by subscribing to existing events.
- **Graceful degradation**: If a subsystem fails (e.g., PermissionChecker policy file is missing), the Runtime can start with a fallback (deny-all) and log the degradation. The platform continues operating with reduced functionality.

### Negative

- **Trait method restrictions**: The `Send + Sync + dyn`-compatible constraint means trait methods in the subsystem traits cannot use generics. Methods like `enqueue<T: Into<Priority>>` are not possible. All types must be concrete or boxed.
- **Indirection overhead**: Event dispatch between subsystems adds latency. For tightly coupled operations (e.g., permission check before resource allocation), the round-trip through the EventBus adds approximately 4-6 microseconds. This is acceptable for agent management operations but noticeable for high-frequency operations.
- **Configuration complexity**: Eight subsystems each have their own configuration struct. The combined configuration surface has approximately 40-50 parameters. Documentation and sensible defaults are critical to avoid operator confusion.
- **Startup ordering**: Subsystems must be initialized in the correct order: EventBus first, then subsystems (no dependencies among them), then event handler registrations, then background tasks. The `Runtime::new()` method encapsulates this ordering, but misconfiguration can lead to events being missed during the startup window.
- **Feature parity drift**: The default implementations may not all be equally mature. Early phases may have a well-tested Scheduler but a minimal PermissionChecker. This creates an uneven user experience and may mask design issues in less-mature subsystems.
- **Event spaghetti**: With eight subsystems and dozens of event types, the event flow graph can become complex. Understanding the full lifecycle of an agent requires tracing through the EventBus subscription graph.

## Compliance

1. **Trait signature freeze**: The eight trait signatures in `runtime/src/lib.rs` can only be changed via a new ADR. Additive changes (new methods with default implementations) are allowed without an ADR, but removal or signature changes require ADR approval.

2. **Implementation-to-trait mapping**: Each trait must have at least one implementation. CI checks that no trait is defined without a corresponding default implementation module in `runtime/src/*/default.rs`. The check runs `rg "pub trait" runtime/src/lib.rs` and verifies each trait has a corresponding module.

3. **Event subscription audit**: A CI integration test verifies that each subsystem's event subscriptions are registered during `Runtime::new()`. The test initializes a `Runtime`, inspects the EventBus subscriber registry via `handler_count()`, and asserts that the expected number of handlers are registered for each event type.

4. **Resource limit enforcement**: Integration tests verify that the ResourceManager correctly limits agents to their configured budgets. Tests create agents that deliberately exceed limits and verify that the Scheduler pauses or terminates them. These tests run in CI with resource limits configured to small values.

5. **Negative tests**: Each subsystem must have tests for failure modes:
   - Supervisor: agent crash, policy escalation, max retries exceeded.
   - ResourceManager: limit exceeded, double release, invalid reservation.
   - Scheduler: empty queue, cancel non-existent agent, priority inversion.
   - StateMachine: invalid transition, double transition, force state recovery.

6. **Documentation**: Each trait must have doc comments describing its contract (preconditions, postconditions, error conditions). Each default implementation module must have module-level docs describing the algorithm, configuration options, and trade-offs.

7. **StateMachine transition matrix test**: The transition matrix must be tested exhaustively: for every (from_state, to_state) pair, verify that the transition is either accepted or rejected with a clear error. This test is generated from the matrix definition to ensure completeness.

## Notes

- During Phase 3 implementation, the Supervisor and Scheduler were initially in separate submodules but were combined under a single `runtime-supervisor` crate to reduce EventBus overhead for the tight supervisor-scheduler loop. They are split again in this ADR for conceptual clarity. The default implementations may share internal types as an optimization.
- The PermissionChecker is currently a stub implementation (allow-all for development, deny-all for production). A full RBAC implementation with policy file loading, role hierarchy, and action pattern matching is planned for Phase 4.
- The ResourceManager's GPU memory tracking is advisory only because GPU memory accounting on Linux requires NVIDIA's NVML library or AMD's ROCm SMI, which are not available in all deployment environments. CPU and memory tracking use cgroups v2, which is available on all modern Linux systems.
- ADR-NNNN (planned for Phase 4) will address the StateMachine state transition matrix in detail, including the exact set of states, transitions, and the conditions for each transition.
- The `Runtime` struct's `new()` method starts background tasks (supervision health check loop, scheduler worker, resource accounting tick). These tasks are cancelled when `Runtime` is dropped, using Tokio's cancellation tokens.
- The resource accounting model (two-phase reserve/commit) was adopted after a race condition was discovered during Phase 3 testing where two agents could simultaneously "reserve" the last available resource slot.

## References

- [ADR-0001: Project Vision and Scope](./0001-project-vision.md) — Establishes phased roadmap; runtime is Phases 3-4.
- [ADR-0002: Clean Architecture with Layered Modules](./0002-clean-architecture.md) — Runtime crate depends on core; subsystems communicate via EventBus.
- [ADR-0003: Rust as Implementation Language](./0003-rust.md) — Trait constraints for Send+Sync+dyn compatibility; async-trait for subsystem traits.
- [ADR-0004: Event-Driven Architecture via EventBus](./0004-event-driven.md) — All subsystem interactions go through the EventBus.
- [Runtime Source: runtime/src/lib.rs](../../runtime/src/lib.rs)
- [Runtime Configuration: runtime/src/config.rs](../../runtime/src/config.rs)
- [Runtime Subsystems: runtime/src/scheduler/](../../runtime/src/scheduler/)
- [Runtime Subsystems: runtime/src/supervisor/](../../runtime/src/supervisor/)
- [Runtime Subsystems: runtime/src/state_machine/](../../runtime/src/state_machine/)
