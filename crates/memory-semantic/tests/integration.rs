#[cfg(test)]
mod tests {
    use memory_semantic::SemanticError;
    use uuid::Uuid;

    #[test]
    fn all_error_variants_display() {
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
