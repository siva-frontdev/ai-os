# Runtime API — AI-OS Runtime Abstraction

**Status:** Draft
**Date:** 2026-08-05
**Scope:** The generic, runtime-agnostic interface between AI-OS cognition and external execution runtimes.
**Non-negotiable:** The Runtime API contains **only generic runtime concepts**. No OpenClaw types, no Home Assistant types, no ROS types, no vendor vocabulary. OpenClaw is one implementation of this interface.

---

## 1. Purpose

AI-OS's brain (World Model, Understanding, Cognitive Loop, Planner, Evolution, Reflection, Attention, Memory, Decision, Communication Logic) must not know what runtime it drives. When the Planner emits an action such as `send_email` or `post_telegram_message`, it addresses a **capability**, not a vendor. The Runtime API is the contract that makes the execution substrate replaceable.

```
AI-OS Brain (cognition)
        │  emits Action (capability + params)
        ▼
   Runtime trait  ◄── generic, stable
        │  implemented by
        ├── openclaw-runtime        (OpenClaw: channels, tools, connectors)
        ├── native-runtime          (OSAL: filesystem, process, network)
        ├── homeassistant-runtime   (future)
        ├── ros-runtime             (future)
        └── robot-runtime           (future)
```

---

## 2. Design Principles

1. **Capability-addressed, not vendor-addressed.** AI-OS plans against namespaced capability IDs (`email.send`, `telegram.post_message`, `browser.navigate`). The runtime maps a capability to a concrete implementation.
2. **Bidirectional I/O.** The runtime both *executes* actions (outbound) and *produces observations* (inbound: incoming messages, device events). Both directions are first-class.
3. **Structured payloads.** All action parameters and results are typed JSON values (`serde_json::Value`) validated against declared schemas — never stringly-typed shell fragments.
4. **Replaceable.** The trait is implemented behind `Arc<dyn Runtime>`. Swapping OpenClaw for another runtime requires zero changes in the cognitive core.
5. **Observable.** Every action is traceable (trace_id), every capability is discoverable, and the runtime reports health.
6. **Fail-safe.** Errors are typed, never panics. Unknown capabilities are rejected before execution.

---

## 3. The Trait

```rust
//! runtime/ai-os-runtime-api/src/lib.rs

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Identifies a runtime instance (e.g. "openclaw", "native", "homeassistant").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeId(pub String);

/// A namespaced capability identifier, e.g. `"email.send"`, `"telegram.post_message"`.
/// Naming convention: `<domain>.<verb>`, lowercase ASCII, dots as separators.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityId(pub String);

/// A capability the runtime can execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: CapabilityId,
    pub name: String,                 // short human label: "Send email"
    pub description: String,          // what it does, when to use it
    pub input_schema: serde_json::Value,   // JSON Schema for Action.input
    pub output_schema: Option<serde_json::Value>, // JSON Schema for ActionResult.output
    pub side_effects: Vec<SideEffect>,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Declared side effects of a capability (for planner transparency & policy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SideEffect {
    SendsMessage,
    MutatesExternalState,
    ReadsExternalState,
    SpawnsProcess,
    AccessesNetwork,
    AccessesFilesystem,
    None,
}

/// An observation the runtime detected (inbound message, device event, state change).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub source: CapabilityId,         // which connector observed it
    pub kind: ObservationKind,
    pub channel_id: Option<String>,  // e.g. telegram chat id
    pub sender: Option<String>,
    pub timestamp_ms: u64,
    pub payload: serde_json::Value,  // message text, event data, ...
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservationKind {
    InboundMessage,
    StatusChanged,
    DeviceEvent,
    ScheduleTrigger,
    Health,
}

/// A request to perform one capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_id: String,            // caller-generated correlation id
    pub capability: CapabilityId,
    pub input: serde_json::Value,     // validated against capability.input_schema
    pub trace_id: Option<String>,
    pub deadline_ms: Option<u64>,     // optional hard deadline
    pub metadata: std::collections::HashMap<String, String>,
}

/// The outcome of an Action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action_id: String,
    pub status: ActionStatus,
    pub output: Option<serde_json::Value>,
    pub error: Option<RuntimeErrorPayload>,
    pub started_ms: u64,
    pub finished_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionStatus {
    Succeeded,
    Failed,
    TimedOut,
    Cancelled,
}

/// Machine-readable error carried inside an ActionResult (never a panic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeErrorPayload {
    pub code: String,                 // stable machine code, e.g. "capability_not_found"
    pub message: String,              // human message
    pub retryable: bool,
}

/// A runtime observation/event subscription item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeEvent {
    Observation(Observation),
    HealthChanged(RuntimeHealth),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeHealth {
    Ready,
    Degraded { reason: String },
    Unavailable { reason: String },
}

/// The Runtime contract. All traits are Send + Sync + Debug and dyn-compatible.
#[async_trait::async_trait]
pub trait Runtime: Debug + Send + Sync {
    /// Unique identifier of this runtime implementation.
    fn id(&self) -> RuntimeId;

    /// Initialize connections/backends. Idempotent. Errors before any use.
    async fn initialize(&self) -> Result<(), RuntimeError>;

    /// List capabilities this runtime can execute right now.
    async fn capabilities(&self) -> Vec<Capability>;

    /// Resolve a capability id to its declared Capability, if supported.
    async fn capability(&self, id: &CapabilityId) -> Option<Capability>;

    /// Execute one action. The returned future completes when the action
    /// reaches a terminal state (Succeeded/Failed/TimedOut/Cancelled).
    async fn execute(&self, action: Action) -> Result<ActionResult, RuntimeError>;

    /// Drain observations that arrived since the last call (pull model).
    /// For push consumers use `subscribe()`.
    async fn observe(&self) -> Vec<Observation>;

    /// Subscribe to runtime events (observations + health). Returns a
    /// stream receiver or an error if the runtime does not support push.
    async fn subscribe(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<RuntimeEvent>, RuntimeError>;

    /// Current health of the runtime.
    async fn health(&self) -> RuntimeHealth;
}

/// Top-level error for interface failures (not per-action failures,
/// which are returned inside ActionResult).
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("runtime not initialized: {0}")]
    NotInitialized(String),
    #[error("transport failure: {0}")]
    Transport(String),
    #[error("runtime does not support push subscriptions")]
    SubscriptionUnsupported,
    #[error("invalid action: {0}")]
    InvalidAction(String),
    #[error("internal runtime error: {0}")]
    Internal(String),
}
```

### Default implementation

Per project conventions, a `DefaultRuntime` is provided for testing and graceful degradation:

```rust
/// A no-op runtime with zero capabilities. Used for tests and for
/// configurations where no execution substrate is configured.
#[derive(Debug)]
pub struct DefaultRuntime;

#[async_trait::async_trait]
impl Runtime for DefaultRuntime {
    fn id(&self) -> RuntimeId { RuntimeId("default".into()) }
    async fn initialize(&self) -> Result<(), RuntimeError> { Ok(()) }
    async fn capabilities(&self) -> Vec<Capability> { Vec::new() }
    async fn capability(&self, _id: &CapabilityId) -> Option<Capability> { None }
    async fn execute(&self, _action: Action) -> Result<ActionResult, RuntimeError> {
        Err(RuntimeError::NotInitialized("default runtime has no capabilities".into()))
    }
    async fn observe(&self) -> Vec<Observation> { Vec::new() }
    async fn subscribe(&self) -> Result<tokio::sync::mpsc::Receiver<RuntimeEvent>, RuntimeError> {
        Err(RuntimeError::SubscriptionUnsupported)
    }
    async fn health(&self) -> RuntimeHealth { RuntimeHealth::Unavailable { reason: "default runtime".into() } }
}
```

---

## 4. Naming & Schema Conventions

| Item | Convention | Example |
|---|---|---|
| Capability id | `<domain>.<verb>`, lowercase | `email.send`, `telegram.post_message` |
| Input/output | JSON Schema (`serde_json::Value`) | `{"type":"object","properties":{"to":{...}}}` |
| Errors | stable machine `code`, never free-form | `capability_not_found` |
| Correlation | caller-generated `action_id`, echoed in result | `act_9f1c…` |
| Tracing | optional `trace_id` propagated through | inherited from Cognitive Loop |

---

## 5. Relationship to Existing AI-OS Layers

```
Layer 7  execution/  ── Execution Platform (execution-* crates): sandbox, monitor, recovery
Layer 6  brain/      ── Planner emits PlannedAction (cognitive_loop → planner)
                          │
Layer 7  runtime-api ── Runtime trait (this document)
                          │  implemented by
                     ┌────┴───────────────────────────────────┐
                     │  openclaw-runtime (adapter → Gateway/MCP)│
                     │  native-runtime    (adapter → OSAL)      │
                     │  ...future runtimes                       │
                     └──────────────────────────────────────────┘
```

- The **Planner** (`brain-coordinator/src/planner`) emits `PlannedAction { action_type, topics, reason }`. The mapping from planner action → `CapabilityId` lives in the **Capability Registry** (brain) and is resolved through a `RuntimeDispatcher` that owns an `Arc<dyn Runtime>`.
- The **Execution Platform** (RFC-0005, `execution/`) remains the sandbox/monitor/recovery pipeline for *process* execution. The Runtime API is a sibling abstraction for *external runtime* execution (messaging, connectors, device runtimes). Where an action must run inside the OS, the Runtime API may delegate to the Execution Platform.
- **Inbound observations** (incoming Telegram messages, email, device events) enter AI-OS through `Runtime::observe()` / `subscribe()` and are fed to the **Cognitive Loop** (World Understanding) exactly like the existing channel-facing inputs (`brain-coordinator/src/companion_host`).

---

## 6. Capability Registry ↔ Runtime Bridge

The brain already has a `CapabilityRegistry` (`brain-coordinator/src/planner/capabilities.rs`) that lists what the Planner may plan. Two registries now exist and must be reconciled:

| Registry | Owner | Content | When populated |
|---|---|---|---|
| Brain `CapabilityRegistry` | AI-OS cognition | Planner-facing capabilities (`respond`, `search_memory`, `observe`, …) | static defaults |
| Runtime `capabilities()` | runtime-api | Executable capabilities (`email.send`, `telegram.post_message`, …) | dynamic, at runtime init |

**Bridge rule:** at startup, `runtime.initialize()` → `runtime.capabilities()` → merge into the brain's `CapabilityRegistry` so the Planner can reason about what is actually executable. The merge is additive; brain capabilities that have no runtime backing remain (e.g. `respond` is always a brain capability). This keeps cognition declarative: **adding a capability = implementing it in a runtime = zero planner changes** (matches the existing capability philosophy in `capabilities.rs`).

---

## 7. Acceptance Criteria

1. `Runtime` trait contains no string containing "openclaw", "gateway", "telegram", "gmail", or any vendor name.
2. A second runtime (e.g. `native-runtime`) can implement the same trait with no trait changes.
3. Replacing `openclaw-runtime` with another runtime requires changes only in `runtime/openclaw-runtime`'s wiring, never in `brain/` or `runtime/ai-os-runtime-api`.
4. Every capability round-trips through JSON Schema validation before execution.
5. Unknown capabilities return `ActionStatus::Failed` with code `capability_not_found`, never a panic.
