//! The [`Runtime`] trait and its event/health types.

use std::fmt;
use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::action::Action;
use crate::action::ActionResult;
use crate::capability::{Capability, CapabilityId};
use crate::error::RuntimeError;
use crate::observation::Observation;

/// Identifies a runtime instance (e.g. `openclaw`, `native`,
/// `homeassistant`). Unique across all runtimes registered in a manager.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeId(pub String);

impl fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An event pushed to subscribers of a runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeEvent {
    /// A new observation from the runtime.
    Observation(Observation),
    /// The runtime's health changed.
    HealthChanged(RuntimeHealth),
}

/// Current health of a runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeHealth {
    /// Ready to accept actions.
    Ready,
    /// Operational but degraded.
    Degraded {
        /// Why it is degraded.
        reason: String,
    },
    /// Not usable.
    Unavailable {
        /// Why it is unavailable.
        reason: String,
    },
}

/// The Runtime contract.
///
/// All trait methods are dyn-compatible and the trait is
/// `Debug + Send + Sync`, so runtimes are always used behind
/// `Arc<dyn Runtime>`.
#[async_trait::async_trait]
pub trait Runtime: Debug + Send + Sync {
    /// Unique identifier of this runtime implementation.
    fn id(&self) -> RuntimeId;

    /// Initialize connections/backends. Idempotent. Must complete before
    /// any other method is used.
    async fn initialize(&self) -> Result<(), RuntimeError>;

    /// List capabilities this runtime can execute right now.
    async fn capabilities(&self) -> Vec<Capability>;

    /// Resolve a capability id to its declared [`Capability`], if
    /// supported.
    async fn capability(&self, id: &CapabilityId) -> Option<Capability>;

    /// Execute one action. The future completes when the action reaches a
    /// terminal state (`Succeeded`/`Failed`/`TimedOut`/`Cancelled`).
    /// Per-action failures are returned inside the `ActionResult`; `Err`
    /// is reserved for interface/transport failures.
    async fn execute(&self, action: Action) -> Result<ActionResult, RuntimeError>;

    /// Drain observations that arrived since the last call (pull model).
    async fn observe(&self) -> Vec<Observation>;

    /// Subscribe to runtime events (observations + health). Returns a
    /// stream receiver, or an error if the runtime does not support push.
    async fn subscribe(&self) -> Result<tokio::sync::mpsc::Receiver<RuntimeEvent>, RuntimeError>;

    /// Current health of the runtime.
    async fn health(&self) -> RuntimeHealth;
}
