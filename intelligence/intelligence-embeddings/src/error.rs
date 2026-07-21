use intelligence_core::error::ModelError;
use thiserror::Error;

pub type EmbeddingsResult<T> = Result<T, EmbeddingsError>;

#[derive(Debug, Error)]
pub enum EmbeddingsError {
    #[error("embedding error: {0}")]
    Other(String),
}

impl From<EmbeddingsError> for ModelError {
    fn from(e: EmbeddingsError) -> Self {
        ModelError::Io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intelligence_core::error::ModelError;
    #[test]
    fn error_message() {
        let e = EmbeddingsError::Other("oops".into());
        assert!(e.to_string().contains("oops"));
    }
    #[test]
    fn from_embeddings_error_converts() {
        let e = EmbeddingsError::Other("fail".into());
        let model_err: ModelError = e.into();
        assert!(matches!(model_err, ModelError::Io(_)));
    }
}
