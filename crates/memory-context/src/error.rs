use std::fmt;

use uuid::Uuid;

/// Unified error type for context management operations.
#[derive(Debug, thiserror::Error)]
pub enum ContextManagerError {
    #[error("context not found: {0}")]
    ContextNotFound(Uuid),

    #[error("context already exists: {0}")]
    ContextAlreadyExists(Uuid),

    #[error("parent context not found: {0}")]
    ParentNotFound(Uuid),

    #[error("value not found for key '{0}' in context")]
    ValueNotFound(String),

    #[error("invalid operation: {0}")]
    InvalidOperation(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience alias for `Result<T, ContextManagerError>`.
pub type ContextManagerResult<T> = Result<T, ContextManagerError>;

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_context_not_found() {
        let id = Uuid::new_v4();
        let err = ContextManagerError::ContextNotFound(id);
        assert!(err.to_string().contains(&id.to_string()));
    }

    #[test]
    fn test_value_not_found() {
        let err = ContextManagerError::ValueNotFound("foo".into());
        assert!(err.to_string().contains("foo"));
    }

    #[test]
    fn test_all_error_variants_are_display() {
        let errors: Vec<ContextManagerError> = vec![
            ContextManagerError::ContextNotFound(Uuid::new_v4()),
            ContextManagerError::ContextAlreadyExists(Uuid::new_v4()),
            ContextManagerError::ParentNotFound(Uuid::new_v4()),
            ContextManagerError::ValueNotFound("k".into()),
            ContextManagerError::InvalidOperation("bad".into()),
            ContextManagerError::SerializationError("json".into()),
            ContextManagerError::Internal("oops".into()),
        ];
        for err in &errors {
            let _ = err.to_string();
        }
        assert_eq!(errors.len(), 7);
    }
}
