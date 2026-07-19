use async_trait::async_trait;
use memory_core::{MemoryId, MemoryObject, MemoryResult};

/// Efficient batch operations for memory storage.
///
/// The `BatchOperation` trait groups multiple related operations
/// into a single call, enabling backends to optimize throughput
/// (e.g., using a single database round-trip or transaction).
#[async_trait]
pub trait BatchOperation: Send + Sync + std::fmt::Debug {
    /// Insert multiple memory objects in a single batch.
    ///
    /// Implementations should prefer all-or-nothing semantics:
    /// if any insert fails, the entire batch is rejected.
    async fn insert_batch(&self, objects: &[MemoryObject]) -> MemoryResult<()>;

    /// Retrieve multiple memory objects by their IDs.
    ///
    /// Returns a vector aligned with `ids` — `None` entries
    /// correspond to IDs not found in the store.
    async fn get_batch(&self, ids: &[MemoryId]) -> MemoryResult<Vec<Option<MemoryObject>>>;

    /// Delete multiple memory objects by their IDs.
    ///
    /// Non-existent IDs are silently ignored.
    async fn delete_batch(&self, ids: &[MemoryId]) -> MemoryResult<()>;
}

/// Default no-op implementation of [`BatchOperation`].
#[derive(Debug)]
pub struct DefaultBatchOperation;

#[async_trait]
impl BatchOperation for DefaultBatchOperation {
    async fn insert_batch(&self, _objects: &[MemoryObject]) -> MemoryResult<()> {
        Err(memory_core::MemoryError::StorageBackendError("DefaultBatchOperation: not configured".into()))
    }

    async fn get_batch(&self, _ids: &[MemoryId]) -> MemoryResult<Vec<Option<MemoryObject>>> {
        Err(memory_core::MemoryError::StorageBackendError("DefaultBatchOperation: not configured".into()))
    }

    async fn delete_batch(&self, _ids: &[MemoryId]) -> MemoryResult<()> {
        Err(memory_core::MemoryError::StorageBackendError("DefaultBatchOperation: not configured".into()))
    }
}
