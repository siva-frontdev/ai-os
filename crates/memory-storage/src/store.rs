use async_trait::async_trait;
use memory_core::{
    MemoryError, MemoryId, MemoryObject, MemoryResult, QueryFilter, StorageStats,
};

/// High-level storage abstraction for memory objects.
///
/// `MemoryStore` is the primary interface for all persistence operations
/// in the Memory Platform. It provides CRUD, batch, query, and management
/// operations on [`MemoryObject`] records.
///
/// Implementations include:
/// - [`InMemoryStore`](crate::in_memory::InMemoryStore) — thread-safe HashMap backend
/// - [`MockStore`](crate::mock::MockStore) — configurable mock for testing
///
/// Future backends (SQLite, PostgreSQL, RocksDB) will implement this trait
/// through a [`StorageBackend`](crate::backend::StorageBackend) wrapper.
#[async_trait]
pub trait MemoryStore: Send + Sync + std::fmt::Debug {
    // -- CRUD --

    /// Insert a new memory object.
    ///
    /// Returns an error if an object with the same ID already exists.
    async fn insert(&self, object: MemoryObject) -> MemoryResult<()>;

    /// Retrieve a memory object by ID.
    ///
    /// Returns `Ok(None)` if the object does not exist.
    async fn get(&self, id: &MemoryId) -> MemoryResult<Option<MemoryObject>>;

    /// Update an existing memory object.
    ///
    /// The object's ID determines which record to update.
    /// Returns `ObjectNotFound` if the ID does not exist.
    async fn update(&self, object: MemoryObject) -> MemoryResult<()>;

    /// Delete a memory object by ID.
    ///
    /// Returns `ObjectNotFound` if the ID does not exist.
    async fn delete(&self, id: &MemoryId) -> MemoryResult<()>;

    /// Check if a memory object exists.
    async fn exists(&self, id: &MemoryId) -> MemoryResult<bool>;

    // -- Batch operations --

    /// Insert multiple memory objects in a single batch.
    ///
    /// If any object's ID conflicts with an existing record,
    /// the entire batch is rejected (all-or-nothing).
    async fn insert_batch(&self, objects: &[MemoryObject]) -> MemoryResult<()>;

    /// Retrieve multiple memory objects by their IDs.
    ///
    /// Returns a `Vec` of the same length as `ids`, with `None` entries
    /// for IDs that were not found.
    async fn get_batch(&self, ids: &[MemoryId]) -> MemoryResult<Vec<Option<MemoryObject>>>;

    /// Delete multiple memory objects by their IDs.
    ///
    /// Non-existent IDs are silently ignored.
    async fn delete_batch(&self, ids: &[MemoryId]) -> MemoryResult<()>;

    // -- Query --

    /// Query memory objects by filter criteria.
    ///
    /// Returns up to `filter.limit` results, sorted by `filter.sort_by`.
    async fn query(&self, filter: &QueryFilter) -> MemoryResult<Vec<MemoryObject>>;

    /// Count memory objects matching a filter.
    async fn count(&self, filter: &QueryFilter) -> MemoryResult<u64>;

    // -- Management --

    /// Flush any pending writes to the backing store.
    async fn flush(&self) -> MemoryResult<()>;

    /// Compact the underlying storage (reclaim space, defragment).
    async fn compact(&self) -> MemoryResult<()>;

    /// Return statistics about the store.
    async fn stats(&self) -> MemoryResult<StorageStats>;
}

/// Default no-op implementation of [`MemoryStore`].
///
/// All operations return appropriate error or default values.
/// Used as a placeholder when a real store is not yet configured.
#[derive(Debug)]
pub struct DefaultMemoryStore;

#[async_trait]
impl MemoryStore for DefaultMemoryStore {
    async fn insert(&self, _object: MemoryObject) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn get(&self, _id: &MemoryId) -> MemoryResult<Option<MemoryObject>> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn update(&self, _object: MemoryObject) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn delete(&self, _id: &MemoryId) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn exists(&self, _id: &MemoryId) -> MemoryResult<bool> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn insert_batch(&self, _objects: &[MemoryObject]) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn get_batch(&self, _ids: &[MemoryId]) -> MemoryResult<Vec<Option<MemoryObject>>> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn delete_batch(&self, _ids: &[MemoryId]) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn query(&self, _filter: &QueryFilter) -> MemoryResult<Vec<MemoryObject>> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn count(&self, _filter: &QueryFilter) -> MemoryResult<u64> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn flush(&self) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn compact(&self) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }

    async fn stats(&self) -> MemoryResult<StorageStats> {
        Err(MemoryError::StorageBackendError("DefaultMemoryStore: not configured".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_memory_store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultMemoryStore>();
    }
}
