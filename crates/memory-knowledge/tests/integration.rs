#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn all_error_variants_display() {
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
