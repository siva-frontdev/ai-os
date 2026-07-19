use std::ops::Range;

use async_trait::async_trait;
use memory_core::{MemoryId, MemoryResult, Timestamp};

/// Index and search memory objects by time range.
///
/// Provides efficient temporal queries for finding objects
/// created before, after, or within a time range.
#[async_trait]
pub trait TimeIndex: Send + Sync + std::fmt::Debug {
    /// Index a memory object by its timestamp.
    async fn index_time(&self, id: &MemoryId, timestamp: Timestamp) -> MemoryResult<()>;

    /// Find all object IDs with timestamps within the given range (inclusive).
    async fn search_by_time_range(&self, range: &Range<Timestamp>) -> MemoryResult<Vec<MemoryId>>;

    /// Find the oldest `limit` objects (earliest timestamps).
    async fn search_oldest(&self, limit: usize) -> MemoryResult<Vec<MemoryId>>;

    /// Find the newest `limit` objects (latest timestamps).
    async fn search_newest(&self, limit: usize) -> MemoryResult<Vec<MemoryId>>;

    /// Remove a memory object from the time index.
    async fn remove_object(&self, id: &MemoryId) -> MemoryResult<()>;

    /// Rebuild the entire time index from scratch.
    async fn rebuild(&self) -> MemoryResult<()>;

    /// Return the oldest timestamp in the index, if any.
    async fn oldest_timestamp(&self) -> MemoryResult<Option<Timestamp>>;

    /// Return the newest timestamp in the index, if any.
    async fn newest_timestamp(&self) -> MemoryResult<Option<Timestamp>>;

    /// Return the total number of indexed entries.
    async fn len(&self) -> MemoryResult<usize>;

    /// Return true if the index is empty.
    async fn is_empty(&self) -> MemoryResult<bool> {
        Ok(self.len().await? == 0)
    }
}

/// Default no-op implementation of [`TimeIndex`].
///
/// All search methods return empty results. Indexing methods are no-ops.
#[derive(Debug)]
pub struct DefaultTimeIndex;

#[async_trait]
impl TimeIndex for DefaultTimeIndex {
    async fn index_time(&self, _id: &MemoryId, _timestamp: Timestamp) -> MemoryResult<()> {
        Ok(())
    }

    async fn search_by_time_range(&self, _range: &Range<Timestamp>) -> MemoryResult<Vec<MemoryId>> {
        Ok(Vec::new())
    }

    async fn search_oldest(&self, _limit: usize) -> MemoryResult<Vec<MemoryId>> {
        Ok(Vec::new())
    }

    async fn search_newest(&self, _limit: usize) -> MemoryResult<Vec<MemoryId>> {
        Ok(Vec::new())
    }

    async fn remove_object(&self, _id: &MemoryId) -> MemoryResult<()> {
        Ok(())
    }

    async fn rebuild(&self) -> MemoryResult<()> {
        Ok(())
    }

    async fn oldest_timestamp(&self) -> MemoryResult<Option<Timestamp>> {
        Ok(None)
    }

    async fn newest_timestamp(&self) -> MemoryResult<Option<Timestamp>> {
        Ok(None)
    }

    async fn len(&self) -> MemoryResult<usize> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_time_index() {
        let idx = DefaultTimeIndex;
        let id = MemoryId::new();

        assert!(idx.index_time(&id, Timestamp::now()).await.is_ok());
        assert!(
            idx.search_by_time_range(&(Timestamp::MIN..Timestamp::MAX))
                .await
                .unwrap()
                .is_empty()
        );
        assert!(idx.search_oldest(10).await.unwrap().is_empty());
        assert!(idx.search_newest(10).await.unwrap().is_empty());
        assert!(idx.remove_object(&id).await.is_ok());
        assert!(idx.rebuild().await.is_ok());
        assert!(idx.oldest_timestamp().await.unwrap().is_none());
        assert!(idx.newest_timestamp().await.unwrap().is_none());
        assert_eq!(idx.len().await.unwrap(), 0);
        assert!(idx.is_empty().await.unwrap());
    }

    #[test]
    fn test_default_time_index_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultTimeIndex>();
    }
}
