use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReflectionError {
    #[error("no lessons available: {0}")]
    NoLessons(String),
    #[error("comparison failed: {0}")]
    ComparisonFailed(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type ReflectionResult<T> = Result<T, ReflectionError>;
