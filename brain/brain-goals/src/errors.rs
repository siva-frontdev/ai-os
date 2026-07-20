use brain_core::ids::GoalId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GoalsError {
    #[error("goal {0} not found")]
    NotFound(GoalId),

    #[error("goal {0} already exists")]
    AlreadyExists(GoalId),

    #[error("cycle detected: {0:?}")]
    CycleDetected(Vec<GoalId>),

    #[error("dependency {dependency} not found for goal {goal}")]
    DependencyNotFound { goal: GoalId, dependency: GoalId },

    #[error("validation error: {0}")]
    ValidationError(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type GoalsResult<T> = Result<T, GoalsError>;
