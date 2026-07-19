use std::fmt;
use uuid::Uuid;

/// Unified error type for knowledge graph operations.
#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    #[error("entity not found: {0}")]
    EntityNotFound(Uuid),

    #[error("fact not found: {0}")]
    FactNotFound(Uuid),

    #[error("entity already exists: {0}")]
    EntityAlreadyExists(Uuid),

    #[error("relation not found: {0}")]
    RelationNotFound(Uuid),

    #[error("no path between entities {0} and {1} within depth {2}")]
    NoPath(Uuid, Uuid, usize),

    #[error("storage backend error: {0}")]
    StorageBackendError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type KnowledgeResult<T> = Result<T, KnowledgeError>;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_variants_display() {
        let cases: Vec<KnowledgeError> = vec![
            KnowledgeError::EntityNotFound(Uuid::new_v4()),
            KnowledgeError::FactNotFound(Uuid::new_v4()),
            KnowledgeError::EntityAlreadyExists(Uuid::new_v4()),
            KnowledgeError::RelationNotFound(Uuid::new_v4()),
            KnowledgeError::NoPath(Uuid::new_v4(), Uuid::new_v4(), 3),
            KnowledgeError::StorageBackendError("x".into()),
            KnowledgeError::SerializationError("y".into()),
            KnowledgeError::Internal("z".into()),
        ];
        for e in &cases {
            let _ = e.to_string();
        }
        assert_eq!(cases.len(), 8);
    }
}
