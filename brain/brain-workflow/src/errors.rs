use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("step not found: {0}")]
    StepNotFound(String),
    #[error("workflow is already completed: {0}")]
    AlreadyCompleted(String),
    #[error("workflow execution failed: {0}")]
    ExecutionFailed(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type WorkflowResult<T> = Result<T, WorkflowError>;
