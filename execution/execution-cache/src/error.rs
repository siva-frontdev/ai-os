use thiserror::Error;

pub type CacheResult<T> = Result<T, CacheError>;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cache entry not found: {0}")]
    NotFound(String),

    #[error("cache store error: {0}")]
    StoreError(String),

    #[error("learning engine error: {0}")]
    LearningError(String),

    #[error("lock poisoned: {0}")]
    LockPoisoned(String),

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}

impl From<CacheError> for execution_core::ExecutionError {
    fn from(e: CacheError) -> Self {
        match e {
            CacheError::NotFound(c) => Self::NotFound(c),
            CacheError::StoreError(d) => Self::CacheError(d),
            CacheError::LearningError(d) => Self::LearningEngineError(d),
            CacheError::LockPoisoned(d) => Self::CacheError(d),
            CacheError::Execution(inner) => inner,
            CacheError::Core(inner) => Self::Core(inner),
        }
    }
}
