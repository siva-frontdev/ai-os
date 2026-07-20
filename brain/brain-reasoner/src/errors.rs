use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReasonerError {
    #[error("no valid inference found: {0}")]
    NoValidInference(String),

    #[error("hypothesis generation failed: {0}")]
    HypothesisFailed(String),

    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("inconsistency detected: {0}")]
    Inconsistency(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type ReasonerResult<T> = Result<T, ReasonerError>;
