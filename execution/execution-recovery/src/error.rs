use thiserror::Error;

pub type RecoveryResult<T> = Result<T, RecoveryError>;

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("retry budget exhausted after {attempts} attempts")]
    RetryBudgetExhausted { attempts: u32 },

    #[error("rollback step failed: {step}: {detail}")]
    RollbackStepFailed { step: String, detail: String },

    #[error("rollback retry exhausted for step index {step_index}")]
    RollbackRetryExhausted { step_index: usize },

    #[error("rollback plan missing")]
    RollbackPlanMissing,

    #[error("cancellation failed: {0}")]
    CancellationFailed(String),

    #[error("compensation tool failed: {0}")]
    CompensationToolFailed(String),

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}
