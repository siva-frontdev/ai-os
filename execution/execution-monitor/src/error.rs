use thiserror::Error;

pub type MonitorResult<T> = Result<T, MonitorError>;

#[derive(Debug, Error)]
pub enum MonitorError {
    #[error("execution {0} not found")]
    ExecutionNotFound(String),

    #[error("watch {0} already exists")]
    WatchAlreadyExists(String),

    #[error("watch {0} is not active")]
    WatchNotActive(String),

    #[error("timeout setup failed: {0}")]
    TimeoutSetupFailed(String),

    #[error("resource tracking failed: {0}")]
    ResourceTrackingFailed(String),

    #[error("internal lock poisoned")]
    LockPoisoned,

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}
