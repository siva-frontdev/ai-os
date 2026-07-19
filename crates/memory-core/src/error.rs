use crate::types::{MemoryId, MemoryTier};

/// Unified error type for all Memory Platform operations.
///
/// Every memory crate returns `MemoryResult<T> = Result<T, MemoryError>`.
/// Variants are organized by subsystem for precise error handling.
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    // -- Lookup errors --
    /// The requested memory object was not found.
    #[error("object not found: {0}")]
    ObjectNotFound(MemoryId),

    /// The requested context was not found.
    #[error("context not found: {0}")]
    ContextNotFound(uuid::Uuid),

    // -- Capacity errors --
    /// A memory tier has reached its capacity limit.
    #[error("tier {0} is full: {1} bytes used of {2}")]
    TierFull(MemoryTier, u64, u64),

    /// A specific capacity limit was exceeded.
    #[error("capacity exceeded for {0}: {1} used of {2}")]
    CapacityExceeded(String, u64, u64),

    // -- Query errors --
    /// The query specification was invalid.
    #[error("invalid query: {0}")]
    InvalidQuery(String),

    // -- Index errors --
    /// Index build or rebuild failed.
    #[error("index build failed: {0}")]
    IndexBuildFailed(String),

    /// Search operation failed.
    #[error("search failed: {0}")]
    SearchFailed(String),

    // -- Learning errors --
    /// Consolidation operation failed for a specific object.
    #[error("consolidation failed for {0}: {1}")]
    ConsolidationFailed(MemoryId, String),

    /// Pruning operation failed.
    #[error("pruning failed: {0}")]
    PruningFailed(String),

    /// Pattern discovery failed.
    #[error("pattern discovery failed: {0}")]
    PatternDiscoveryFailed(String),

    // -- Snapshot errors --
    /// Snapshot operation failed.
    #[error("snapshot failed: {0}")]
    SnapshotFailed(String),

    // -- Storage errors --
    /// Serialization or deserialization error.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// Underlying storage backend error.
    #[error("storage backend error: {0}")]
    StorageBackendError(String),

    // -- Security errors --
    /// The operation was denied due to insufficient permissions.
    #[error("permission denied: {session} cannot access {resource}")]
    PermissionDenied {
        /// The session that was denied access.
        session: String,
        /// The resource that was accessed.
        resource: String,
    },

    // -- Transaction errors --
    /// Transaction operation failed.
    #[error("transaction failed: {0}")]
    TransactionError(String),

    // -- Timeout errors --
    /// The operation exceeded its time bound.
    #[error("operation timed out after {0}ms")]
    Timeout(u64),

    // -- Validation errors --
    /// Input validation failed.
    #[error("validation error: {0}")]
    ValidationError(String),

    // -- Internal errors --
    /// An internal invariant was violated.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience alias for `Result<T, MemoryError>`.
pub type MemoryResult<T> = Result<T, MemoryError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_not_found() {
        let id = MemoryId::new();
        let err = MemoryError::ObjectNotFound(id);
        let msg = err.to_string();
        assert!(msg.starts_with("object not found:"));
    }

    #[test]
    fn test_tier_full() {
        let err = MemoryError::TierFull(MemoryTier::Working, 500, 1024);
        let msg = err.to_string();
        assert!(msg.contains("Working"));
        assert!(msg.contains("500"));
        assert!(msg.contains("1024"));
    }

    #[test]
    fn test_permission_denied() {
        let err = MemoryError::PermissionDenied {
            session: "sess-1".into(),
            resource: "memory:query".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("sess-1"));
        assert!(msg.contains("memory:query"));
    }

    #[test]
    fn test_validation_error() {
        let err = MemoryError::ValidationError("importance out of range".into());
        assert!(err.to_string().contains("importance"));
    }

    #[test]
    fn test_timeout() {
        let err = MemoryError::Timeout(5000);
        assert!(err.to_string().contains("5000"));
    }

    #[test]
    fn test_all_error_variants_are_display() {
        let id = MemoryId::new();
        let errors: Vec<MemoryError> = vec![
            MemoryError::ObjectNotFound(id),
            MemoryError::ContextNotFound(uuid::Uuid::new_v4()),
            MemoryError::TierFull(MemoryTier::Working, 0, 0),
            MemoryError::CapacityExceeded("test".into(), 0, 0),
            MemoryError::InvalidQuery("bad filter".into()),
            MemoryError::IndexBuildFailed("oom".into()),
            MemoryError::SearchFailed("timeout".into()),
            MemoryError::ConsolidationFailed(id, "disk full".into()),
            MemoryError::PruningFailed("lock contention".into()),
            MemoryError::PatternDiscoveryFailed("no data".into()),
            MemoryError::SnapshotFailed("permission denied".into()),
            MemoryError::SerializationError("invalid utf-8".into()),
            MemoryError::StorageBackendError("connection lost".into()),
            MemoryError::PermissionDenied {
                session: "s".into(),
                resource: "r".into(),
            },
            MemoryError::TransactionError("rollback failed".into()),
            MemoryError::Timeout(100),
            MemoryError::ValidationError("bad value".into()),
            MemoryError::Internal("unreachable".into()),
        ];
        for err in &errors {
            let _ = err.to_string();
        }
        assert_eq!(errors.len(), 18);
    }
}
