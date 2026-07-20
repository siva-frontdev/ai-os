use thiserror::Error;

pub type DispatchResult<T> = Result<T, DispatchError>;

#[derive(Debug, Error)]
pub enum DispatchError {
    #[error("dispatch queue is full")]
    QueueFull,

    #[error("concurrency limit reached for backend: {0}")]
    ConcurrencyLimitReached(String),

    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("backend fallback failed: {0}")]
    BackendFallbackFailed(String),

    #[error("execution not found: {0}")]
    ExecutionNotFound(String),

    #[error("execution not running: {0}")]
    ExecutionNotRunning(String),

    #[error("cancel failed: {0}")]
    CancelFailed(String),

    #[error("internal lock poisoned")]
    LockPoisoned,

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("planner error: {0}")]
    Planner(#[from] execution_planner::PlannerError),

    #[error("sandbox error: {0}")]
    Sandbox(#[from] execution_sandbox::SandboxError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}
