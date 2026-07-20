//! `PolicyError` — unified error type for the policy crate.
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("rule not found: {0}")]
    RuleNotFound(String),

    #[error("ruleset not found: {0}")]
    RulesetNotFound(String),

    #[error("invalid policy configuration: {0}")]
    InvalidConfig(String),

    #[error("rule compilation failed: {0}")]
    CompilationFailed(String),

    #[error("policy evaluation error: {0}")]
    EvaluationError(String),

    #[error("guard rail violation: {0}")]
    GuardRailViolation(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("internal policy error: {0}")]
    Internal(String),
}

pub type PolicyResult<T> = Result<T, PolicyError>;
