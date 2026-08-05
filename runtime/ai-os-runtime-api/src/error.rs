//! Top-level error type for the Runtime API.
//!
//! Interface and transport failures return [`RuntimeError`]. Per-action
//! failures (unknown capability, invalid input, tool failure) are NOT
//! errors — they are returned inside `ActionResult` with a typed
//! `RuntimeErrorPayload`, so the Planner can react without exception
//! handling.

/// Interface-level failure for a runtime or the runtime manager.
///
/// This is distinct from per-action failures, which are carried inside
/// [`ActionResult`](crate::ActionResult).
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// The runtime has not been initialized before use.
    #[error("runtime not initialized: {0}")]
    NotInitialized(String),
    /// The transport failed (connection, I/O, protocol).
    #[error("transport failure: {0}")]
    Transport(String),
    /// The runtime does not support push subscriptions.
    #[error("runtime does not support push subscriptions")]
    SubscriptionUnsupported,
    /// The action was rejected before execution.
    #[error("invalid action: {0}")]
    InvalidAction(String),
    /// A runtime with this id is already registered.
    #[error("runtime already registered: {0}")]
    DuplicateRuntime(String),
    /// No runtime with this id is registered.
    #[error("unknown runtime: {0}")]
    UnknownRuntime(String),
    /// Any other internal failure.
    #[error("internal runtime error: {0}")]
    Internal(String),
}
