use thiserror::Error;

/// Unified error type for the entire core platform.
///
/// Every module converts its specific failures into this enum,
/// giving callers a single error type to handle.
#[derive(Debug, Error)]
pub enum CoreError {
    // ── Config ────────────────────────────────────────────
    #[error("configuration key `{key}` not found")]
    ConfigNotFound { key: String },

    #[error("failed to parse configuration key `{key}`: {detail}")]
    ConfigParse { key: String, detail: String },

    #[error("{0}")]
    Config(String),

    // ── Container ─────────────────────────────────────────
    #[error("a service implementing `{type_name}` is already registered")]
    ServiceAlreadyRegistered { type_name: &'static str },

    #[error("no service registered for `{type_name}`")]
    ServiceNotRegistered { type_name: &'static str },

    // ── Event Bus ─────────────────────────────────────────
    #[error("no handler registered for event `{event_type}`")]
    NoHandler { event_type: &'static str },

    #[error("event handler for `{event_type}` failed: {detail}")]
    HandlerFailed {
        event_type: &'static str,
        detail: String,
    },

    // ── Lifecycle ─────────────────────────────────────────
    #[error("service `{name}` failed to start: {detail}")]
    StartFailed { name: String, detail: String },

    #[error("service `{name}` failed to stop: {detail}")]
    StopFailed { name: String, detail: String },

    #[error("service `{name}` state error: {detail}")]
    State { name: String, detail: String },

    // ── Registry ──────────────────────────────────────────
    #[error("service `{name}` is not registered")]
    ServiceNotFound { name: String },

    #[error("service `{name}` is already registered")]
    ServiceConflict { name: String },

    // ── Health ────────────────────────────────────────────
    #[error("health check `{name}` failed: {detail}")]
    HealthCheck { name: String, detail: String },

    // ── Joint / Lock ──────────────────────────────────────
    #[error("internal lock poisoned")]
    LockPoisoned,

    #[error("{0}")]
    General(String),
}

impl CoreError {
    pub fn config<T: Into<String>>(msg: T) -> Self {
        CoreError::Config(msg.into())
    }
}
