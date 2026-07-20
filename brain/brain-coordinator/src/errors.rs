use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error("goal error: {0}")]
    Goal(String),
    #[error("planning error: {0}")]
    Planning(String),
    #[error("reasoning error: {0}")]
    Reasoning(String),
    #[error("decision error: {0}")]
    Decision(String),
    #[error("policy error: {0}")]
    Policy(String),
    #[error("reflection error: {0}")]
    Reflection(String),
    #[error("learning error: {0}")]
    Learning(String),
    #[error("workflow error: {0}")]
    Workflow(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type CoordinatorResult<T> = Result<T, CoordinatorError>;
