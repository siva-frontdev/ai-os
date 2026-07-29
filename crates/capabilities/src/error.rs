use thiserror::Error;

#[derive(Debug, Error)]
pub enum CapabilityError {
    #[error("unknown capability: {0}")]
    UnknownCapability(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("missing required input: {0}")]
    MissingInput(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("utf-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

pub type CapabilityResult<T> = Result<T, CapabilityError>;
