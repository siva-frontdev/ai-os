use memory_core::MemoryId;
use thiserror::Error;

/// Errors returned by the working memory subsystem.
///
/// Each variant carries contextual information about what failed
/// so that callers can respond appropriately.
#[derive(Debug, Error)]
pub enum WorkingMemoryError {
    /// The requested memory object was not found in working memory.
    #[error("object not found: {0}")]
    EntryNotFound(MemoryId),

    /// The requested session was not found.
    #[error("session not found: {0}")]
    SessionNotFound(uuid::Uuid),

    /// The working memory tier has reached its entry capacity.
    #[error("tier full: {0} entries used of {1}")]
    TierFull(u64, u64),

    /// The working memory has exceeded its byte capacity.
    #[error("capacity exceeded: {0} bytes used of {1}")]
    CapacityExceeded(u64, u64),

    /// The provided query string was invalid.
    #[error("invalid query: {0}")]
    InvalidQuery(String),

    /// An error propagated from the underlying storage backend.
    #[error("storage backend error: {0}")]
    StorageBackendError(String),

    /// Serialization or deserialization of memory data failed.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// An internal invariant was violated within working memory.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience alias for `Result<T, WorkingMemoryError>`.
pub type WorkingMemoryResult<T> = Result<T, WorkingMemoryError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_not_found() {
        let id = MemoryId::new();
        let err = WorkingMemoryError::EntryNotFound(id);
        let msg = err.to_string();
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_session_not_found() {
        let id = uuid::Uuid::new_v4();
        let err = WorkingMemoryError::SessionNotFound(id);
        let msg = err.to_string();
        assert!(msg.contains("session"));
        assert!(msg.contains(&id.to_string()));
    }

    #[test]
    fn test_tier_full() {
        let err = WorkingMemoryError::TierFull(500, 1024);
        let msg = err.to_string();
        assert!(msg.contains("500"));
        assert!(msg.contains("1024"));
    }

    #[test]
    fn test_capacity_exceeded() {
        let err = WorkingMemoryError::CapacityExceeded(500, 1024);
        let msg = err.to_string();
        assert!(msg.contains("500"));
        assert!(msg.contains("1024"));
    }

    #[test]
    fn test_invalid_query() {
        let err = WorkingMemoryError::InvalidQuery("bad syntax".into());
        assert!(err.to_string().contains("bad syntax"));
    }

    #[test]
    fn test_storage_backend_error() {
        let err = WorkingMemoryError::StorageBackendError("disk full".into());
        assert!(err.to_string().contains("disk full"));
    }

    #[test]
    fn test_serialization_error() {
        let err = WorkingMemoryError::SerializationError("invalid utf-8".into());
        assert!(err.to_string().contains("invalid utf-8"));
    }

    #[test]
    fn test_internal() {
        let err = WorkingMemoryError::Internal("lock poisoned".into());
        assert!(err.to_string().contains("lock poisoned"));
    }

    #[test]
    fn test_all_variants_are_display() {
        let id = MemoryId::new();
        let errors: Vec<WorkingMemoryError> = vec![
            WorkingMemoryError::EntryNotFound(id),
            WorkingMemoryError::SessionNotFound(uuid::Uuid::new_v4()),
            WorkingMemoryError::TierFull(0, 0),
            WorkingMemoryError::CapacityExceeded(0, 0),
            WorkingMemoryError::InvalidQuery("".into()),
            WorkingMemoryError::StorageBackendError("".into()),
            WorkingMemoryError::SerializationError("".into()),
            WorkingMemoryError::Internal("".into()),
        ];
        for err in &errors {
            let _ = err.to_string();
        }
        assert_eq!(errors.len(), 8);
    }
}
