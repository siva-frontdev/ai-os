use intelligence_core::error::ModelError;
use thiserror::Error;

pub type ContextResult<T> = Result<T, ContextError>;

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("context missing: {0}")]
    Missing(String),
    #[error("truncation error: {0}")]
    TruncationError(String),
    #[error("other: {0}")]
    Other(String),
}

impl From<ContextError> for ModelError {
    fn from(e: ContextError) -> Self {
        match e {
            ContextError::Missing(d) => ModelError::ContextWindowExceeded,
            ContextError::TruncationError(d) => ModelError::ContextWindowExceeded,
            ContextError::Other(d) => ModelError::Io(d),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_format() {
        let e = ContextError::Missing("foo".into());
        assert!(e.to_string().contains("foo"));
    }
    #[test]
    fn from_context_error_converts() {
        let e = ContextError::Other("bar".into());
        let model_err: ModelError = e.into();
        assert!(matches!(model_err, ModelError::Io(_)));
    }
}
