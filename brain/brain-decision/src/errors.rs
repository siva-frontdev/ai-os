use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecisionError {
    #[error("no decision possible: {0}")]
    NoDecision(String),

    #[error("conflict cannot be resolved: {0}")]
    UnresolvableConflict(String),

    #[error("confidence below threshold: {conf} < {threshold}")]
    LowConfidence { conf: f64, threshold: f64 },

    #[error("policy rejected decision: {0}")]
    PolicyRejection(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type DecisionResult<T> = Result<T, DecisionError>;
