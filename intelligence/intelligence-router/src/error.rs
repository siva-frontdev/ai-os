use intelligence_core::error::ModelError;
use thiserror::Error;

pub type RouterResult<T> = Result<T, RouterError>;

impl From<RouterError> for ModelError {
    fn from(e: RouterError) -> Self {
        ModelError::Io(e.to_string())
    }
}

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("no model found for capability {0:?}")]
    NoModelFound(String),
    #[error("all providers unavailable for capability {0:?}")]
    AllProvidersUnavailable(String),
    #[error("routing error: {0}")]
    Other(String),
}
