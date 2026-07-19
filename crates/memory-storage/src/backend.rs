use async_trait::async_trait;
use memory_core::{MemoryError, MemoryResult};

/// Low-level key-value storage backend abstraction.
///
/// `StorageBackend` is the foundation for pluggable database backends.
/// Higher-level traits like [`MemoryStore`](crate::store::MemoryStore)
/// are implemented on top of this interface.
///
/// Implementations must be thread-safe (`Send + Sync`) and handle
/// concurrent access safely.
#[async_trait]
pub trait StorageBackend: Send + Sync + std::fmt::Debug {
    /// Read a value by key.
    ///
    /// Returns `Ok(None)` if the key does not exist.
    async fn get(&self, key: &[u8]) -> MemoryResult<Option<Vec<u8>>>;

    /// Write a key-value pair.
    ///
    /// Overwrites any existing value for the same key.
    async fn put(&self, key: &[u8], value: &[u8]) -> MemoryResult<()>;

    /// Delete a key-value pair.
    ///
    /// Succeeds even if the key does not exist.
    async fn delete(&self, key: &[u8]) -> MemoryResult<()>;

    /// Flush any pending writes to durable storage.
    async fn flush(&self) -> MemoryResult<()>;
}

/// Default no-op implementation of [`StorageBackend`].
#[derive(Debug)]
pub struct DefaultStorageBackend;

#[async_trait]
impl StorageBackend for DefaultStorageBackend {
    async fn get(&self, _key: &[u8]) -> MemoryResult<Option<Vec<u8>>> {
        Err(MemoryError::StorageBackendError("DefaultStorageBackend: not configured".into()))
    }

    async fn put(&self, _key: &[u8], _value: &[u8]) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultStorageBackend: not configured".into()))
    }

    async fn delete(&self, _key: &[u8]) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultStorageBackend: not configured".into()))
    }

    async fn flush(&self) -> MemoryResult<()> {
        Err(MemoryError::StorageBackendError("DefaultStorageBackend: not configured".into()))
    }
}
