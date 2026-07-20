use thiserror::Error;

pub type CoordinatorResult<T> = Result<T, CoordinatorError>;

#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error("pipeline setup failed: {0}")]
    PipelineSetupFailed(String),

    #[error("stage connection failed: {0}")]
    StageConnectionFailed(String),

    #[error("execution not found: {0}")]
    ExecutionNotFound(String),

    #[error("pipeline stage closed: {0}")]
    StageClosed(String),

    #[error("brain communication failed: {0}")]
    BrainCommunicationFailed(String),

    #[error("lifecycle conflict: {0}")]
    LifecycleConflict(String),

    #[error("internal lock poisoned")]
    LockPoisoned,

    #[error("recovery error: {0}")]
    Recovery(#[from] execution_recovery::RecoveryError),

    #[error("dispatch error: {0}")]
    Dispatch(#[from] execution_dispatcher::DispatchError),

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("results error: {0}")]
    Results(#[from] execution_results::ResultsError),

    #[error("monitor error: {0}")]
    Monitor(#[from] execution_monitor::MonitorError),

    #[error("brain error: {0}")]
    Brain(String),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}
