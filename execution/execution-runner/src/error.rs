use execution_core::ExecutionError;
use thiserror::Error;

pub type RunnerResult<T> = Result<T, RunnerError>;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("process spawn failed: {0}")]
    SpawnFailed(String),

    #[error("process wait failed: {0}")]
    WaitFailed(String),

    #[error("process exited with code {exit_code}")]
    InvalidExitCode { exit_code: i32 },

    #[error("process terminated by signal {signal}")]
    SignalTerminated { signal: String },

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("timeout expired")]
    TimeoutExpired,

    #[error("execution cancelled")]
    Cancelled,

    #[error("output capture failed: {0}")]
    OutputCaptureFailed(String),

    #[error("WASM execution not implemented: {0}")]
    WasmNotImplemented(String),

    #[error("container execution not implemented: {0}")]
    ContainerNotImplemented(String),

    #[error("execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),
}

impl From<RunnerError> for ExecutionError {
    fn from(e: RunnerError) -> Self {
        match e {
            RunnerError::SpawnFailed(msg) => ExecutionError::ProcessSpawnFailed(msg),
            RunnerError::WaitFailed(msg) => ExecutionError::ExecutionPanic(msg),
            RunnerError::InvalidExitCode { exit_code } => {
                ExecutionError::InvalidExitCode { exit_code }
            }
            RunnerError::SignalTerminated { signal } => ExecutionError::SignalTerminated { signal },
            RunnerError::IoError(e) => ExecutionError::ExecutionPanic(e.to_string()),
            RunnerError::TimeoutExpired => ExecutionError::TimeoutExceeded { elapsed_ms: 0 },
            RunnerError::Cancelled => ExecutionError::CancellationFailed("cancelled".into()),
            RunnerError::OutputCaptureFailed(msg) => ExecutionError::ArtifactCollectionFailed(msg),
            RunnerError::WasmNotImplemented(msg) => ExecutionError::BackendUnavailable(msg),
            RunnerError::ContainerNotImplemented(msg) => ExecutionError::BackendUnavailable(msg),
            RunnerError::Execution(e) => e,
            RunnerError::NotFound(msg) => ExecutionError::NotFound(msg),
            RunnerError::InvalidArgument(msg) => ExecutionError::ConfigurationError(msg),
            RunnerError::BackendUnavailable(msg) => ExecutionError::BackendUnavailable(msg),
        }
    }
}
