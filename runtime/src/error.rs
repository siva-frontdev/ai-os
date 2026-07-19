use ai_os_core::error::CoreError;
use thiserror::Error;

/// Unified error type for the entire runtime platform.
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("scheduler error: {0}")]
    Scheduler(String),

    #[error("supervisor error: {0}")]
    Supervisor(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("task error: {0}")]
    Task(String),

    #[error("context error: {0}")]
    Context(String),

    #[error("state error: {0}")]
    State(String),

    #[error("resource error: {0}")]
    Resource(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("core error: {0}")]
    Core(#[from] CoreError),

    #[error("{0}")]
    General(String),
}
