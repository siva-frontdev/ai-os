use async_trait::async_trait;
use memory_core::{MemoryId, MemoryObject};

use crate::error::WorkingMemoryResult;

/// Session-scoped working memory operations.
///
/// Each session gets an isolated slice of working memory.
/// Sessions are identified by their session UUID and are
/// used to maintain separation between concurrent cognitive
/// contexts (e.g., different conversations or tasks).
#[async_trait]
pub trait WorkingMemorySession: Send + Sync + std::fmt::Debug {
    /// Store an object in the session's working memory.
    async fn store(&self, session_id: &uuid::Uuid, object: MemoryObject)
    -> WorkingMemoryResult<()>;

    /// Recall an object from the session's working memory.
    async fn recall(
        &self,
        session_id: &uuid::Uuid,
        id: &MemoryId,
    ) -> WorkingMemoryResult<Option<MemoryObject>>;

    /// List all object IDs in a session.
    async fn list_session(&self, session_id: &uuid::Uuid) -> WorkingMemoryResult<Vec<MemoryId>>;

    /// Remove all objects for a session.
    async fn clear_session(&self, session_id: &uuid::Uuid) -> WorkingMemoryResult<()>;
}

/// A no-op implementation of [`WorkingMemorySession`] that returns errors on all operations.
///
/// Useful as a default or placeholder when session management is not yet configured.
#[derive(Debug)]
pub struct DefaultWorkingMemorySession;

#[async_trait]
impl WorkingMemorySession for DefaultWorkingMemorySession {
    async fn store(
        &self,
        _session_id: &uuid::Uuid,
        _object: MemoryObject,
    ) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemorySession: not configured".into(),
        ))
    }

    async fn recall(
        &self,
        _session_id: &uuid::Uuid,
        _id: &MemoryId,
    ) -> WorkingMemoryResult<Option<MemoryObject>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemorySession: not configured".into(),
        ))
    }

    async fn list_session(&self, _session_id: &uuid::Uuid) -> WorkingMemoryResult<Vec<MemoryId>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemorySession: not configured".into(),
        ))
    }

    async fn clear_session(&self, _session_id: &uuid::Uuid) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemorySession: not configured".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_working_memory_session_returns_error() {
        let session = DefaultWorkingMemorySession;
        let sid = uuid::Uuid::new_v4();
        let id = MemoryId::new();

        let obj = MemoryObject::builder().content_type("text").build();
        let result = session.store(&sid, obj).await;
        assert!(result.is_err());

        let result = session.recall(&sid, &id).await;
        assert!(result.is_err());

        let result = session.list_session(&sid).await;
        assert!(result.is_err());

        let result = session.clear_session(&sid).await;
        assert!(result.is_err());
    }
}
