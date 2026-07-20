use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error("plan {0} not found")]
    NotFound(String),

    #[error("validation error: {0}")]
    ValidationError(String),

    #[error("no feasible plan found: {0}")]
    NoFeasiblePlan(String),

    #[error("cycle detected in task graph")]
    CycleDetected,

    #[error("internal error: {0}")]
    Internal(String),
}

pub type PlannerResult<T> = Result<T, PlannerError>;
