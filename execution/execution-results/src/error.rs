use thiserror::Error;

pub type ResultsResult<T> = Result<T, ResultsError>;

#[derive(Debug, Error)]
pub enum ResultsError {
    #[error("output parse failed: {0}")]
    OutputParseFailed(String),

    #[error("output too large: {size} bytes exceeds max {max} bytes")]
    OutputTooLarge { size: u64, max: u64 },

    #[error("artifact collection failed: {0}")]
    ArtifactCollectionFailed(String),

    #[error("artifact store failed: {0}")]
    ArtifactStoreFailed(String),

    #[error("enrichment failed: {0}")]
    EnrichmentFailed(String),

    #[error("route to {destination} failed: {error}")]
    RouteFailed { destination: String, error: String },

    #[error("route permission denied: {0}")]
    RoutePermissionDenied(String),

    #[error("no parser found for output: {0}")]
    NoParserFound(String),

    #[error("invalid result state: {0}")]
    InvalidResultState(String),

    #[error("internal lock poisoned")]
    LockPoisoned,

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("runner error: {0}")]
    Runner(#[from] execution_runner::RunnerError),

    #[error("monitor error: {0}")]
    Monitor(#[from] execution_monitor::MonitorError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}
