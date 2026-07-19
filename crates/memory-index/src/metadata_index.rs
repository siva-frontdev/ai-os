use async_trait::async_trait;
use memory_core::{MemoryId, MemoryResult};

/// Index and search memory objects by metadata key-value pairs.
///
/// Provides efficient lookup of [`MemoryObject`](memory_core::MemoryObject) IDs
/// based on their [`Metadata`](memory_core::Metadata) entries.
#[async_trait]
pub trait MetadataIndex: Send + Sync + std::fmt::Debug {
    /// Index a metadata key-value pair for a memory object.
    ///
    /// Replaces any previously indexed value for the same (id, key) pair.
    async fn index_metadata(&self, id: &MemoryId, key: &str, value: &str) -> MemoryResult<()>;

    /// Find all object IDs that have the given metadata key-value pair.
    async fn search_by_metadata(&self, key: &str, value: &str) -> MemoryResult<Vec<MemoryId>>;

    /// Find all object IDs that have the given metadata key with a value
    /// matching the given prefix.
    async fn search_by_metadata_prefix(&self, key: &str, value_prefix: &str) -> MemoryResult<Vec<MemoryId>>;

    /// Remove all metadata entries for a memory object.
    async fn remove_object(&self, id: &MemoryId) -> MemoryResult<()>;

    /// Rebuild the entire metadata index from scratch.
    async fn rebuild(&self) -> MemoryResult<()>;

    /// Return the total number of indexed metadata key-value pairs.
    async fn len(&self) -> MemoryResult<usize>;

    /// Return true if the index is empty.
    async fn is_empty(&self) -> MemoryResult<bool> {
        Ok(self.len().await? == 0)
    }
}

/// Default no-op implementation of [`MetadataIndex`].
///
/// All search methods return empty results. Indexing methods are no-ops.
/// Used as a placeholder when a real index is not yet configured.
#[derive(Debug)]
pub struct DefaultMetadataIndex;

#[async_trait]
impl MetadataIndex for DefaultMetadataIndex {
    async fn index_metadata(&self, _id: &MemoryId, _key: &str, _value: &str) -> MemoryResult<()> {
        Ok(())
    }

    async fn search_by_metadata(&self, _key: &str, _value: &str) -> MemoryResult<Vec<MemoryId>> {
        Ok(Vec::new())
    }

    async fn search_by_metadata_prefix(&self, _key: &str, _value_prefix: &str) -> MemoryResult<Vec<MemoryId>> {
        Ok(Vec::new())
    }

    async fn remove_object(&self, _id: &MemoryId) -> MemoryResult<()> {
        Ok(())
    }

    async fn rebuild(&self) -> MemoryResult<()> {
        Ok(())
    }

    async fn len(&self) -> MemoryResult<usize> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_metadata_index() {
        let idx = DefaultMetadataIndex;
        let id = MemoryId::new();

        assert!(idx.index_metadata(&id, "key", "value").await.is_ok());
        let results = idx.search_by_metadata("key", "value").await.unwrap();
        assert!(results.is_empty());
        assert!(idx.remove_object(&id).await.is_ok());
        assert!(idx.rebuild().await.is_ok());
        assert_eq!(idx.len().await.unwrap(), 0);
        assert!(idx.is_empty().await.unwrap());
    }

    #[test]
    fn test_default_metadata_index_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultMetadataIndex>();
    }
}
