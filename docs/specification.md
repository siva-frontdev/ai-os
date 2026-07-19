# Master Specification

> This document is the constitution of the AI-native Operating Platform.
>
> Every module, every component, every line of code must satisfy this specification.
>
> When architecture documents, implementation code, and this specification disagree,
> **this specification wins.**

---

## Table of Contents

1. [Project Mission](#1-project-mission)
2. [Non-Functional Requirements](#2-non-functional-requirements)
3. [Performance Goals](#3-performance-goals)
4. [Security Goals](#4-security-goals)
5. [Boot Requirements](#5-boot-requirements)
6. [Shutdown Requirements](#6-shutdown-requirements)
7. [AI Lifecycle](#7-ai-lifecycle)
8. [Event Lifecycle](#8-event-lifecycle)
9. [Memory Lifecycle](#9-memory-lifecycle)
10. [Runtime Lifecycle](#10-runtime-lifecycle)
11. [Failure Recovery](#11-failure-recovery)
12. [Module Contracts](#12-module-contracts)
13. [Coding Contracts](#13-coding-contracts)

---

## 1. Project Mission

### 1.1 Mission Statement

Build an open-source operating platform in which artificial intelligence is a first-class citizen of the system stack — not an application, not a library, not a service running on top, but a fundamental layer woven into process management, resource allocation, scheduling, inter-process communication, and system observability.

### 1.2 Platform Manifest

The platform identifies itself through `platform.toml` at the workspace root. This file is read by the boot process before any other initialization. It defines:

- Platform identity (name, version, edition, label)
- Architecture target
- Host OS and kernel
- Which subsystems are enabled and their configuration
- Provider selections for each capability

Every deployment has exactly one `platform.toml`. The boot process validates this file before loading any module. A missing or invalid manifest is a fatal boot error.

See [Boot Requirements §5.1](#51-boot-sequence) for the complete boot protocol.

### 1.2 Scope

The platform spans from the Linux kernel interface to the intelligence integration layer. It includes:

- System abstraction over hardware and kernel resources
- Core services (event bus, logging, health, lifecycle, configuration)
- Runtime substrate (scheduling, supervision, session, task, context, resource, permission, state)
- Memory substrate (working memory, long-term storage, indexing, retrieval, consolidation, pruning)
- Perception pipeline (sensor input, normalization, classification, attention, enrichment)
- Brain (reasoning, decision-making, goal management, intent recognition, knowledge graph)
- Execution engine (command execution, output capture, resource accounting, routing)
- Intelligence integration (LLM orchestration, tool-use, RAG, prompt management)

### 1.3 Out of Scope

- The platform does not provide a graphical user interface.
- The platform does not replace the Linux kernel.
- The platform does not provide its own filesystem (it uses the host filesystem).
- The platform does not provide its own networking stack (it uses the host networking).
- The platform does not require cloud services. It runs on a single Arch Linux node.

### 1.4 Design Philosophy

The platform is designed around five immutable tenets:

1. **Intelligence is infrastructure.** Memory, perception, reasoning, and execution are platform primitives, not application concerns. Every component has access to these primitives.

2. **Layers have strict boundaries.** A module at layer N depends only on modules at layer < N. No upward dependencies. No circular dependencies. No exceptions.

3. **Events are the only contract between modules.** No module holds a direct reference to another module. All communication is through typed events on the EventBus.

4. **The system observes itself.** Every component logs, every decision is traced, every state transition emits an event. The observability subsystem is itself observable.

5. **Security is structural, not bolted on.** Authorization, authentication, and audit are enforced at architectural boundaries, not added after implementation.

---

## 2. Non-Functional Requirements

### 2.1 Availability

| Requirement | Target | Measurement |
|---|---|---|
| Planned uptime | 99.99% | Annual minutes of planned downtime < 53 |
| Unplanned uptime | 99.9% | Annual minutes of unplanned downtime < 526 |
| Recovery from crash | < 10 seconds | Time from crash to fully operational |
| Recovery from process death | < 2 seconds | Time from SIGKILL to replacement ready |

The platform must survive the failure of any single component without losing events or corrupting state. Crash recovery must be automatic: if the process dies, the init system (systemd) restarts it, and the platform resumes from its last persisted state.

### 2.2 Durability

| Requirement | Target | Measurement |
|---|---|---|
| Event loss on normal restart | Zero | Every event persisted before ack |
| Event loss on crash | < 1 second window | At-most-once delivery guarantee |
| Memory loss on restart | None | Long-term memory persists across restarts |
| Configuration loss | None | Configuration is file-based, survives all restarts |

### 2.3 Scalability

| Requirement | Target | Notes |
|---|---|---|
| Active sessions per node | 10,000 | Simultaneous, stateful sessions |
| Events per second | 10,000+ | Through a single EventBus instance |
| Tasks per second | 1,000+ | Task creation + scheduling + dispatch |
| Memory per active session | < 1 KB | Over and above the session's owned data |
| Concurrent supervised tasks | 1,000 | Tasks being tracked by the Supervisor |

### 2.4 Observability

Every component must expose:

- **Structured logs** at minimum four levels (ERROR, WARN, INFO, DEBUG) with module, file, line, and timestamp.
- **Metrics** for operation count, latency, error count, and resource usage.
- **Health status** reported to the HealthMonitor on a configurable interval.
- **Lifecycle events** published on the EventBus for start, stop, and state transitions.
- **Trace context** carried on every event through the ContextManager.

The observability subsystem must never degrade the performance of the observed subsystem by more than 5%.

### 2.5 Determinism

The platform must produce deterministic results for the same sequence of inputs, subject to:

- Random number generation is seeded and documented.
- Async task scheduling may introduce non-determinism in timing but not in final state.
- The state machine transitions are fully deterministic given the same event sequence.
- Timestamps are monotonically non-decreasing within a single session.

---

## 3. Performance Goals

### 3.1 Latency

| Operation | Target (p50) | Target (p99) | Degradation Threshold |
|---|---|---|---|
| Event dispatch (publish to first handler) | 100 µs | 1 ms | 10 ms |
| Task scheduling (enqueue + dequeue) | 10 µs | 100 µs | 1 ms |
| Context propagation (attach to event) | 1 µs | 10 µs | 100 µs |
| Permission check (role lookup + comparison) | 5 µs | 50 µs | 500 µs |
| Resource usage update (track + persist) | 10 µs | 100 µs | 1 ms |
| Session lookup by ID | 1 µs | 10 µs | 100 µs |
| Task state transition | 5 µs | 50 µs | 500 µs |
| Health check execution | 100 µs | 1 ms | 10 ms |

### 3.2 Throughput

| Operation | Minimum | Target | Stretch |
|---|---|---|---|
| Events through EventBus | 5,000/s | 10,000/s | 50,000/s |
| Task dispatches | 500/s | 1,000/s | 5,000/s |
| Log lines written | 10,000/s | 50,000/s | 100,000/s |
| Health check probes | 100/s | 500/s | 1,000/s |

### 3.3 Resource Consumption

| Resource | Idle | Light Load (1,000 events/s) | Heavy Load (10,000 events/s) |
|---|---|---|---|
| CPU | < 0.5% | < 10% | < 50% |
| Memory (core only) | < 10 MB | < 50 MB | < 200 MB |
| Memory (core + runtime) | < 20 MB | < 100 MB | < 500 MB |
| File descriptors | < 50 | < 100 | < 500 |

### 3.4 Startup Time

| Scenario | Target | Maximum |
|---|---|---|
| Cold start (no cache) | 500 ms | 2 s |
| Warm start (cached artifacts) | 200 ms | 500 ms |
| Restart after crash | 1 s | 5 s |
| Configuration reload | 100 ms | 500 ms |

### 3.5 Measurement

All performance goals are measured on the reference hardware:

- AWS EC2 c6i.large (2 vCPU, 4 GB RAM)
- Arch Linux LTS kernel
- ext4 filesystem on gp3 SSD
- No other user-space workloads

Performance regression tests run on every commit. A regression of more than 20% on any p50 metric blocks the merge.

---

## 4. Security Goals

### 4.1 Security Principles

The platform follows the Saltzer and Schroeder principles of secure design:

1. **Economy of mechanism.** Keep the security surface as small as possible. Simple designs are easier to verify.

2. **Fail-safe defaults.** Default configuration denies access. Every grant is explicit.

3. **Complete mediation.** Every access to every resource is checked for authorization. No cached authorization decisions.

4. **Open design.** Security does not depend on secrecy. The security architecture is public and documented.

5. **Separation of privilege.** Require multiple conditions for sensitive operations. No single permission grants universal access.

6. **Least privilege.** Every component runs with the minimum permissions necessary to function.

7. **Least common mechanism.** Minimize shared resources between components. Each component gets its own isolated state.

8. **Psychological acceptability.** Security mechanisms must not make the system harder to use for authorized operators.

### 4.2 Authentication Requirements

- Every inter-module call must carry a verified identity.
- Every session must be authenticated before it can perform operations.
- The platform must support at least two authentication backends: local identity store and external OIDC provider.
- Authentication failures must be logged at WARN level.
- Rate-limit authentication attempts: max 5 failures per identity per minute, escalating to 1 per minute after 10 failures.

### 4.3 Authorization Requirements

- Every public API entry point must call PermissionChecker before acting.
- Authorization is role-based: Admin, User, ReadOnly.
- Roles are hierarchical: Admin includes all User permissions. User includes all ReadOnly permissions.
- Permissions are atomic: Read, Write, Execute, Delete, Admin.
- Resource-level authorization: a User may Read their own sessions but not others.
- Authorization decisions are logged at DEBUG level. Denials are logged at WARN level.
- The authorization system must complete in under 50 µs (p99).

### 4.4 Audit Requirements

- All authorization denials are recorded with: timestamp, identity, resource, permission, reason.
- All state transitions (session create/destroy, task state change, phase change) are recorded.
- Audit logs are append-only and immutable after writing.
- Audit logs are separated from application logs.
- Audit log retention: minimum 1 year. Configurable up to 7 years.

### 4.5 Data Protection

- Secrets (API keys, tokens, passwords) are never logged, never serialized in plaintext, never committed to version control.
- Secrets at rest are encrypted using AES-256-GCM.
- Secrets in transit (over the EventBus within the same process) are protected by process isolation.
- Long-term memory data at rest is encrypted.
- Personally Identifiable Information (PII) is tagged and subject to configurable retention policies.

### 4.6 Vulnerability Response

- Security vulnerabilities are reported through a private channel.
- Critical vulnerabilities are patched within 48 hours.
- High-severity vulnerabilities are patched within 7 days.
- Medium and low severity are patched in the next release cycle.
- Every security patch includes a regression test for the vulnerability.

---

## 5. Boot Requirements

### 5.1 Boot Sequence

The platform boot sequence is strictly ordered. Each phase must complete before the next begins.

```
Phase 0: Bootstrap Environment
  ├── Load configuration from filesystem
  ├── Initialize random number generator
  ├── Set up signal handlers (SIGTERM, SIGINT, SIGHUP)
  └── Initialize logging subsystem (minimal, file-only)

Phase 1: Core Infrastructure
  ├── Initialize Container (dependency injection registry)
  ├── Initialize EventBus
  ├── Register core services (Logger, HealthMonitor)
  ├── Initialize LifecycleManager
  └── Register core Service implementations

Phase 2: Platform Services
  ├── Initialize Runtime subsystems:
  │   ├── ContextManager
  │   ├── PermissionChecker
  │   ├── StateMachine
  │   ├── SessionManager
  │   ├── TaskManager
  │   ├── ResourceManager
  │   ├── Scheduler
  │   └── Supervisor
  ├── Initialize System subsystems (Phase 4+):
  │   ├── SystemResourceManager
  │   ├── ProcessManager
  │   ├── FileSystemProvider
  │   ├── NetworkManager
  │   ├── SystemEventCollector
  │   └── ServiceManager
  └── Register all as Service implementations

Phase 3: Service Initialization
  ├── LifecycleManager.init_all() — ordered by dependency
  │   └── Each service validates its configuration
  │   └── Each service subscribes to its events
  │   └── Each service registers its health checks
  └── All services report INITIALIZED

Phase 4: Service Start
  ├── LifecycleManager.start_all() — ordered by dependency
  │   └── Each service begins processing
  │   └── Each service emits <module>.started event
  └── All services report RUNNING

Phase 5: Readiness Verification
  ├── HealthMonitor runs initial health probes
  ├── All services must report HEALTHY within 5 seconds
  └── Platform emits Platform.Ready event
  └── Platform enters RuntimePhase::Running
```

### 5.2 Boot Failure Handling

- If any service fails to initialize, the platform logs the failure, emits a `Platform.BootFailed` event with the failing service name and error, and transitions to `RuntimePhase::Failed`.
- If any service fails to start, the platform attempts to stop all already-started services in reverse order, then transitions to `RuntimePhase::Failed`.
- If health probes fail during Phase 5, the platform retries up to 3 times with exponential backoff (100ms, 500ms, 2s). After 3 failures, the platform transitions to `RuntimePhase::Failed`.
- A boot failure must produce enough diagnostic information (logs, last known state, error chain) to diagnose the root cause without a debugger.

### 5.3 Boot Requirements for Every Module

Every module that implements the `Service` trait must satisfy:

- `init()` validates that all dependencies are available and all configuration is valid.
- `init()` subscribes to required events and registers health checks.
- `init()` returns an error if a required dependency is missing or configuration is invalid.
- `start()` begins processing. It must not block the caller for more than 100ms.
- `start()` publishes a `<module>.started` event on the EventBus upon success.
- `stop()` stops processing gracefully. It drains in-flight work and publishes `<module>.stopped`.
- `stop()` must complete within 5 seconds or the platform issues a forced shutdown.

---

## 6. Shutdown Requirements

### 6.1 Shutdown Sequence

Shutdown is initiated by SIGTERM or SIGINT. The sequence is the reverse of boot:

```
Phase 0: Drain
  ├── Platform transitions to RuntimePhase::Draining
  ├── HealthMonitor reports DRAINING (not HEALTHY)
  ├── Scheduler stops accepting new tasks
  ├── SessionManager stops accepting new sessions
  └── EventBus stops accepting new publications

Phase 1: In-Flight Completion
  ├── Wait for running tasks to complete (max 10 seconds)
  ├── Wait for in-flight events to be handled (max 5 seconds)
  └── Cancel remaining tasks with TaskState::Cancelled

Phase 2: Service Stop
  ├── LifecycleManager.stop_all() — reverse dependency order
  │   └── Each service publishes <module>.stopped
  └── All services report STOPPED

Phase 3: Persistence Flush
  ├── Flush all log buffers
  ├── Flush all metric buffers
  ├── Persist long-term memory state
  └── Persist runtime state snapshot

Phase 4: Platform Stopped
  ├── Platform emits Platform.Stopped event
  ├── Platform transitions to RuntimePhase::Stopped
  └── Process exits with code 0
```

### 6.2 Forced Shutdown

If graceful shutdown exceeds the time limit (30 seconds total), the platform initiates forced shutdown:

- Log a FATAL-level message with the names of services that did not stop.
- Skip remaining in-flight work.
- Skip persistence flush.
- Emit `Platform.ForcedShutdown` event.
- Exit with code 1.

### 6.3 Shutdown Guarantees

- In-flight events that have been received by the EventBus but not yet handled may be lost on forced shutdown.
- All persisted data (long-term memory, configuration, audit logs) must be consistent on disk after any shutdown.
- If the process crashes (SIGKILL, power loss), the systemd unit file must set `Restart=on-failure` with a 2-second delay.

---

## 7. AI Lifecycle

### 7.1 Lifecycle States

An AI agent in the platform progresses through these states:

```
         ┌─────────────┐
         │   Created   │
         └──────┬──────┘
                │
         ┌──────▼──────┐
         │ Initialize  │◄───── Restart (from Supervisor)
         └──────┬──────┘
                │
         ┌──────▼──────┐
    ┌───►│    Act      │
    │    └──────┬──────┘
    │           │
    │    ┌──────▼──────┐
    │    │   Observe   │
    │    └──────┬──────┘
    │           │
    │    ┌──────▼──────┐
    │    │   Learn     │
    │    └──────┬──────┘
    │           │
    │    ┌──────▼──────┐
    │    │   Sleep     │────► Resume ──► Act
    │    └──────┬──────┘
    │           │
    │    ┌──────▼──────┐
    └────┤ Terminate   │
         └─────────────┘
```

### 7.2 State Definitions

| State | Description | Max Duration | Allowed Transitions |
|---|---|---|---|
| `Created` | Agent record exists, no resources allocated | Instant | Initialize |
| `Initialize` | Agent is loading configuration, establishing context, allocating resources | 30 seconds | Act, Terminate |
| `Act` | Agent is executing its primary logic, making decisions, issuing commands | Unlimited | Observe, Sleep, Terminate |
| `Observe` | Agent is processing feedback from previous actions, updating state | 60 seconds | Learn, Act, Terminate |
| `Learn` | Agent is updating internal models, consolidating experiences into memory | 120 seconds | Act, Sleep, Terminate |
| `Sleep` | Agent is suspended, no CPU consumed, state persisted to memory | Configurable | Act (resume), Terminate |
| `Terminate` | Agent is cleaning up resources, persisting final state, emitting final events | 10 seconds | (terminal) |

### 7.3 Lifecycle Rules

- An agent in `Sleep` state consumes no CPU and minimal memory (only session metadata).
- An agent in `Act`, `Observe`, or `Learn` state consumes its allocated resource budget.
- If an agent exceeds its state's max duration, the Supervisor issues a warning. If the agent does not respond within a grace period, the Supervisor forces the agent to `Terminate`.
- Transitions are recorded in the audit log with: agent ID, from state, to state, reason, timestamp.
- An agent may be terminated by the system (resource exhaustion, policy violation, shutdown) or by request.

---

## 8. Event Lifecycle

### 8.1 Event Structure

Every event in the system has the following structure:

```
Event
├── event_type: &'static str    — Unique identifier ("module.event_name")
├── trace_id: Uuid              — Correlates events across the system
├── span_id: Uuid               — Identifies this specific event
├── parent_span_id: Option<Uuid>— Links to causal parent
├── timestamp: DateTime<Utc>    — When the event was created
├── source: &'static str        — Publishing module name
├── payload: T                  — Type-specific data
└── metadata: HashMap<String, String> — Extensible key-value pairs
```

### 8.2 Event Lifecycle States

```
Create → Enrich → Publish → Route → Dispatch → Handle → Complete
```

| Phase | Description | Owner |
|---|---|---|
| **Create** | A component constructs an event struct with the payload and event type. | Publisher |
| **Enrich** | The ContextManager attaches trace_id, span_id, parent_span_id, and timestamp. | ContextManager |
| **Publish** | The event is sent to the EventBus. `publish()` is non-blocking and returns immediately. | EventBus |
| **Route** | The EventBus identifies all subscribers registered for the event's TypeId. | EventBus |
| **Dispatch** | Each subscriber's handler is spawned as an async task on the Tokio runtime. | EventBus |
| **Handle** | The subscriber processes the event. Errors are caught and logged, not propagated. | Subscriber |
| **Complete** | The handler finishes. If configured, an acknowledgement is sent to the publisher. | Subscriber |

### 8.3 Event Rules

1. **Immutability.** Once published, an event is immutable. No handler may modify an event in-flight.

2. **At-most-once delivery.** Events are delivered to each subscriber at most once. Subscribers must handle their own retry logic.

3. **Non-blocking publish.** `EventBus.publish()` must never block the caller for more than 1 microsecond.

4. **Isolated handlers.** A failure in one handler must not affect other handlers for the same event. Each handler runs in its own async task.

5. **Ordering within a stream.** Events from the same source that share a correlation identifier (e.g., session ID) must be dispatched in order. The EventBus guarantees FIFO ordering per correlation ID.

6. **No guaranteed ordering across streams.** Events from different sources or different correlation IDs have no ordering guarantees.

7. **Bounded queues.** The EventBus uses bounded channels (tokio::sync::mpsc). Backpressure is applied to publishers when queues are full.

8. **Trace propagation.** Every event must carry valid trace context. The ContextManager enriches events before they reach the EventBus.

9. **Event type uniqueness.** Event type strings must be unique across the entire platform. Convention: `<module>.<event_name>` (e.g., `session.created`, `task.completed`).

10. **Documentation burden.** Every event type must be documented: event_type string, payload structure, when it is published, which modules consume it.

### 8.4 Reserved Event Types

| Event Type | Publisher | Purpose |
|---|---|---|
| `platform.boot_started` | Bootstrap | Boot sequence has begun |
| `platform.boot_completed` | Bootstrap | Boot sequence completed successfully |
| `platform.boot_failed` | Bootstrap | Boot sequence failed |
| `platform.stopped` | LifecycleManager | Graceful shutdown complete |
| `platform.forced_shutdown` | LifecycleManager | Forced shutdown initiated |
| `platform.phase_changed` | StateMachine | RuntimePhase transitioned |
| `runtime.task_restarting` | Supervisor | A task is being restarted per policy |
| `health.status_changed` | HealthMonitor | A health check changed status |
| `session.created` | SessionManager | A new session was created |
| `session.destroyed` | SessionManager | A session was destroyed |

---

## 9. Memory Lifecycle

### 9.1 Memory Architecture

The Memory Platform provides two tiers of storage:

```
                    ┌─────────────────────────────────────┐
                    │         Working Memory (Tier 1)      │
                    │  In-process, LRU-evicted, bounded     │
                    │  Access time: < 1 µs                 │
                    │  Capacity: configurable (default 10K) │
                    └──────────────┬──────────────────────┘
                                   │ Consolidation
                                   │ (background, periodic)
                                   ▼
                    ┌─────────────────────────────────────┐
                    │      Long-Term Storage (Tier 2)       │
                    │  Persistent (RocksDB), indexed        │
                    │  Access time: < 100 µs               │
                    │  Capacity: disk-limited               │
                    └─────────────────────────────────────┘
```

### 9.2 Memory Lifecycle States

```
Create → Store → Index → Retrieve → Consolidate → Prune
```

| Phase | Description | Tier |
|---|---|---|
| **Create** | A new memory entry is constructed with payload, metadata, and embedding. | Application layer |
| **Store** | The entry is written to Working Memory. If working memory is full, the least-recently-used entry is evicted to Long-Term Storage. | Working Memory |
| **Index** | The entry's embedding and metadata are indexed for semantic and keyword search. | Index |
| **Retrieve** | A query searches Working Memory first, then Long-Term Storage. Results are merged and ranked. | Both |
| **Consolidate** | Background process promotes frequently-accessed Working Memory entries to Long-Term Storage with enhanced indexing. | Background |
| **Prune** | Entries exceeding TTL or capacity limits are removed. Working Memory evicts LRU. Long-Term Storage evicts by importance score. | Both |

### 9.3 Memory Rules

1. **Working Memory is transactional.** A write to Working Memory either succeeds or fails atomically. Partial writes are not possible.

2. **Long-Term Storage is durable.** Writes to Long-Term Storage are persisted to disk before the write operation returns.

3. **Consolidation is asynchronous and non-blocking.** Working Memory operations never wait for consolidation to complete.

4. **Retrieval is best-effort.** A query returns the best matching results within the configured time bound. It does not guarantee completeness.

5. **Pruning preserves importance.** When pruning is necessary, entries with lower importance scores are removed first. Importance is calculated from access frequency, recency, and explicit priority.

6. **Memory is isolated per session.** An agent in one session cannot access memory entries from another session unless explicitly shared.

7. **Memory is time-bound.** Every memory entry has a TTL. Expired entries are pruned during the next consolidation cycle.

8. **Memory is accountable.** Memory usage per session is tracked and enforced by the ResourceManager.

---

## 10. Runtime Lifecycle

### 10.1 Runtime Phase State Machine

The Runtime Platform itself follows a state machine with six states and eight valid transitions:

```
                    ┌──────────────┐
                    │   Created    │
                    └──────┬───────┘
                           │ transition_to(Initializing)
                    ┌──────▼───────┐
              ┌─────│ Initializing │◄────┐
              │     └──────┬───────┘     │
              │            │             │
              │     ┌──────▼───────┐     │
              │     │   Running    │─────┤
              │     └──────┬───────┘     │ (auto-restart)
              │            │             │
              │     ┌──────▼───────┐     │
              │     │   Draining   │─────┘
              │     └──────┬───────┘
              │            │
         ┌────┴────┐  ┌───▼────────┐
         │ Failed  │  │  Stopped   │
         └─────────┘  └────────────┘
```

### 10.2 Valid Transitions

| From | To | Trigger |
|---|---|---|
| Created | Initializing | `transition_to(Initializing)` called after boot Phase 0 |
| Initializing | Running | All services initialized and started successfully |
| Running | Draining | SIGTERM received, or `transition_to(Draining)` called |
| Running | Failed | Unrecoverable error in any subsystem |
| Draining | Stopped | All services stopped, in-flight work completed |
| Draining | Running | Shutdown cancelled (SIGHUP received during drain) |
| Draining | Failed | Forced shutdown initiated |
| Failed | Initializing | Auto-restart (if configured) |

### 10.3 Runtime Phase Rules

1. **Each phase is a subsystem access gate.** Certain operations are only valid in certain phases. For example, task scheduling is only valid in `Running` phase.

2. **Phase transitions are logged and emitted as events.** Every transition produces a `platform.phase_changed` event.

3. **Invalid transitions return an error and are logged at WARN level.** They do not change the current phase.

4. **The `Failed` phase is only exited by process restart.** There is no transition from `Failed` to a non-failed state (except the process-level restart).

5. **Auto-restart from `Failed` to `Initializing`** is configurable via the RestartPolicy. By default, auto-restart is disabled for `Failed`.

---

## 11. Failure Recovery

### 11.1 Failure Classification

| Class | Definition | Examples | Recovery Strategy |
|---|---|---|---|
| **Transient** | Temporary, self-correcting | Network timeout, lock contention, resource temporarily unavailable | Retry with backoff |
| **Persistent** | Will not self-correct without intervention | Disk full, invalid configuration, permission denied | Escalate to operator |
| **Byzantine** | Unpredictable, potentially malicious | Memory corruption, logic bug, security breach | Isolate and terminate |
| **Fatal** | System cannot continue | Kernel panic, hardware failure, critical dependency unavailable | Crash and restart |

### 11.2 Recovery Strategies

| Strategy | Applied To | Mechanism |
|---|---|---|
| **Retry** | Transient failures | Exponential backoff: 100ms, 500ms, 2s, 10s. Max 5 retries. |
| **Supervision** | Task failures | RestartPolicy: Never, OnFailure (up to max_retries), Always (up to max_retries) |
| **Circuit Breaker** | External dependency failures | Open after 5 consecutive failures. Half-open after 30s. Close on success. |
| **Bulkhead** | Resource exhaustion | Per-session resource limits enforced by ResourceManager. Saturation prevents new work. |
| **Checkpoint/Restore** | Long-running tasks | Periodic state snapshots. On failure, restore from last checkpoint. |
| **Escalate** | Persistent failures | Log at ERROR level, emit alert event, notify operator channel. |

### 11.3 Recovery Requirements

1. **Every operation that can fail must have a defined recovery strategy.** A bare `?` propagation to the caller is not a recovery strategy.

2. **Recovery strategies must be documented in the module's specification.**

3. **Retry must be idempotent.** The operation being retried must produce the same result whether executed once or multiple times.

4. **Exponential backoff must include jitter** to prevent thundering herd on recovery.

5. **Circuit breakers must be transparent to callers.** The caller receives a `ServiceUnavailable` error, not a raw connection error.

6. **Bulkhead isolation must be enforced at the entry point.** The ResourceManager checks limits before work begins, not after.

7. **Escalation must include enough context for a human operator to diagnose and act.** Minimum: module name, operation, error, timestamp, trace_id, recommended action.

### 11.4 Crash Recovery

When the platform process crashes:

1. The init system (systemd) restarts the process with `Restart=on-failure` and `RestartSec=2s`.
2. On restart, the platform loads its last persisted state from disk.
3. Sessions that were active at the time of crash are marked as `Terminated` and the owning application is notified.
4. Tasks that were in-flight at the time of crash are marked as `Failed` with reason `process_crash`.
5. Long-term memory is recovered from persistent storage.
6. The platform emits `platform.boot_started` with a `recovery=true` flag.
7. Once boot completes, the platform emits `platform.recovery_complete` with counts of recovered/terminated sessions and tasks.

---

## 12. Module Contracts

### 12.1 What Every Module Must Provide

Every module in the platform must provide:

1. **A unique name.** Convention: `ai-os-<name>` (Cargo crate), `ai_os_<name>` (Rust module).

2. **A public error type.** `pub enum <Module>Error` implementing `std::error::Error` with `thiserror`. One error variant per distinct failure mode.

3. **A public trait for its primary capability.** `pub trait <Capability>: Debug + Send + Sync` — dyn-compatible, with a `Default*` implementation.

4. **A Service implementation.** The module's main entry point must implement the `Service` trait from Core.

5. **Submitted health checks.** At least one health check registered with the HealthMonitor during `init()`.

6. **Event subscriptions.** All events consumed by the module are subscribed during `init()`.

7. **Event publications.** All events produced by the module are documented and published at the correct lifecycle points.

8. **Module-level documentation.** Purpose, responsibilities, public interfaces, dependencies, events (published and consumed), thread model, lifecycle, error handling, configuration, testing strategy, future extensions.

### 12.2 What Every Module Must NOT Do

1. **No direct references to other modules.** All inter-module communication must go through the EventBus.

2. **No upward dependencies.** A module at layer N must not depend on (import or reference) a module at layer > N.

3. **No shared mutable state across module boundaries.** Each module owns its state. Access is through the module's public API or events.

4. **No blocking the async runtime.** Any blocking operation must use `tokio::task::spawn_blocking`.

5. **No silent failures.** Every error must be logged, and if the error affects the module's ability to function, the module must transition to a failed state.

6. **No configuration-side effects.** Reading configuration must not modify global state or trigger side effects.

### 12.3 Module Dependency Graph

```
Layer 8: Intelligence    ──── depends on ──── Execution
Layer 7: Execution       ──── depends on ──── Brain
Layer 6: Brain           ──── depends on ──── Perception
Layer 5: Perception      ──── depends on ──── Memory
Layer 4: Memory          ──── depends on ──── Runtime
Layer 3: Runtime         ──── depends on ──── Core
Layer 2: Core            ──── depends on ──── OSAL
Layer 1: OSAL            ──── depends on ──── Linux Kernel (external)
```

OSAL (Layer 1) is the Operating System Abstraction Layer. It depends on no internal crates. Core (Layer 2) depends on OSAL for all OS interaction. The bottom boundary of OSAL is the **platform boundary** — everything above is OS-independent.

### 12.4 Integration Points

Every module integrates with the platform through exactly three mechanisms:

1. **Service lifecycle** (Core) — the module is started and stopped by the LifecycleManager.
2. **EventBus** (Core) — the module publishes and subscribes to events.
3. **HealthMonitor** (Core) — the module reports its health status.

No other integration mechanisms are permitted. A module must not register signal handlers directly, write to the filesystem outside its configured data directory, or spawn threads that are not managed by the Tokio runtime.

---

## 13. Coding Contracts

### 13.1 The Contract Between Developer and Platform

Every developer (human or AI agent) who writes code for this platform agrees to:

1. **I will not introduce unsafe code** unless it is unavoidable (FFI) and fully reviewed.

2. **I will not add a dependency** unless it solves a problem that cannot be solved with existing code, the license is compatible, and the dependency is maintained.

3. **I will not break the trait contract.** If I change a trait, I update all implementations and all tests. If the change is breaking, I add a deprecation path.

4. **I will not hide errors.** Every failure path is represented in the type system via `Result`. No silent swallows, no unwraps in production.

5. **I will not block the runtime.** All I/O is async. CPU-bound work is spawned on a blocking pool.

6. **I will not hold locks across await points** unless using tokio-aware locks.

7. **I will not commit without formatting.** `cargo fmt` before every commit. Zero diff.

8. **I will not commit with warnings.** `cargo clippy` passes with zero warnings.

9. **I will not commit without tests.** New code has tests. Modified code has updated tests.

10. **I will not commit without documentation.** Public items have rustdoc. Architecture changes have ADRs.

### 13.2 The Contract Between Platform and Developer

The platform promises every developer:

1. **The EventBus will deliver my events.** If a subscriber is registered, it will receive the event at-most-once. The platform guarantees delivery ordering within a correlation stream.

2. **The Service lifecycle will respect my dependencies.** My module's `init()` will be called after all its dependencies are initialized and before any module that depends on it.

3. **The HealthMonitor will check my health.** If I register a health check, it will be probed on schedule. If I fail a probe, the platform will react appropriately.

4. **The Logger will record my messages.** Logs will be written asynchronously and will not block my module. Logs will be structured and queryable.

5. **The ContextManager will propagate my trace context.** Events I publish will automatically receive the correct trace_id and span_id.

6. **The ResourceManager will enforce my limits.** If my module or session exceeds its resource budget, the platform will take corrective action (warn, throttle, terminate).

7. **The Supervisor will handle my failures.** Tasks I submit to the Runtime will be supervised according to their RestartPolicy.

8. **The versioning contract will be respected.** The platform follows semantic versioning. Breaking changes are only introduced in major versions with a documented migration path.

### 13.3 Enforcement

- These contracts are enforced by code review, CI gates, and architectural compliance tests.
- CI runs on every PR: build, clippy, fmt, test, audit, and a compliance check that verifies dependency directions and module isolation.
- A PR that violates any contract term must not be merged until the violation is resolved.

---

## Appendix A: Specification Compliance Matrix

Every module must document its compliance with this specification. The compliance matrix is a table at the top of each module's documentation:

| Requirement | Status | Notes |
|---|---|---|
| §5.3 Service lifecycle implementation | Compliant | init/start/stop implemented |
| §8.3 Event rules (all 10) | Compliant | All event rules verified |
| §12.1 Module contract (all 8) | Compliant | All module requirements met |
| §12.2 Prohibitions (all 6) | Compliant | No violations detected |
| §13.1 Developer contract (all 10) | Compliant | All coding standards met |
| §2.4 Observability | Compliant | Logs, metrics, health, events, traces |
| §3 Performance targets | Not measured | Benchmarks not yet implemented |
| §4.3 Authorization | Compliant | PermissionChecker integrated |

## Appendix B: Terminology

Terms used in this specification have the meanings defined in the [Glossary](glossary.md). In case of conflict between this specification and the glossary, this specification takes precedence.

## Appendix C: Amendments

This specification is amended through the ADR process (see `docs/adr/README.md`). Each amendment must:

1. Reference the specific section being amended.
2. State the rationale for the change.
3. Be approved by the project maintainers.
4. Update the specification version number.

| Version | Date | Amendment | Author |
|---|---|---|---|
| 1.0 | 2025-02-01 | Initial specification | Project maintainers |

---

*This specification is the constitution of the AI-native Operating Platform. Every line of code, every module, every deployment must satisfy it. No exception without an ADR.*
