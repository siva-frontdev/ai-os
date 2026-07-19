use std::fmt;
use uuid::Uuid;

/// Unified error type for semantic memory operations.
#[derive(Debug, thiserror::Error)]
pub enum SemanticError {
    #[error("concept not found: {0}")]
    ConceptNotFound(Uuid),

    #[error("fact not found: {0}")]
    FactNotFound(Uuid),

    #[error("concept already exists: {0}")]
    ConceptAlreadyExists(Uuid),

    #[error("relationship not found: {0}")]
    RelationNotFound(Uuid),

    #[error("invalid confidence: {0}")]
    InvalidConfidence(f32),

    #[error("storage backend error: {0}")]
    StorageBackendError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type SemanticResult<T> = Result<T, SemanticError>;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_variants_display() {
        let cases: Vec<SemanticError> = vec![
            SemanticError::ConceptNotFound(Uuid::new_v4()),
            SemanticError::FactNotFound(Uuid::new_v4()),
            SemanticError::ConceptAlreadyExists(Uuid::new_v4()),
            SemanticError::RelationNotFound(Uuid::new_v4()),
            SemanticError::InvalidConfidence(1.5),
            SemanticError::StorageBackendError("x".into()),
            SemanticError::SerializationError("y".into()),
            SemanticError::Internal("z".into()),
        ];
        for e in &cases {
            let _ = e.to_string();
        }
        assert_eq!(cases.len(), 8);
    }
}
