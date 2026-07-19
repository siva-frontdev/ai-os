use async_trait::async_trait;
use memory_core::{MemoryId, MemoryResult};

/// Index and search memory objects by tags.
///
/// Provides efficient multi-tag query with `match_all` vs `match_any` semantics.
#[async_trait]
pub trait TagIndex: Send + Sync + std::fmt::Debug {
    /// Index the tags of a memory object.
    ///
    /// Replaces all previously indexed tags for this object.
    async fn index_tags(&self, id: &MemoryId, tags: &[String]) -> MemoryResult<()>;

    /// Find all object IDs that have the given tag.
    async fn search_by_tag(&self, tag: &str) -> MemoryResult<Vec<MemoryId>>;

    /// Find all object IDs matching multiple tags.
    ///
    /// If `match_all` is `true`, results include only objects that have all
    /// specified tags (AND semantics). If `false`, results include objects
    /// that have any of the specified tags (OR semantics).
    async fn search_by_tags(&self, tags: &[String], match_all: bool)
    -> MemoryResult<Vec<MemoryId>>;

    /// Remove a memory object from the tag index.
    async fn remove_object(&self, id: &MemoryId) -> MemoryResult<()>;

    /// Rebuild the entire tag index from scratch.
    async fn rebuild(&self) -> MemoryResult<()>;

    /// Return all unique tag values in the index.
    async fn all_tags(&self) -> MemoryResult<Vec<String>>;

    /// Return the total number of indexed tag entries.
    async fn len(&self) -> MemoryResult<usize>;

    /// Return true if the index is empty.
    async fn is_empty(&self) -> MemoryResult<bool> {
        Ok(self.len().await? == 0)
    }
}

/// Default no-op implementation of [`TagIndex`].
///
/// All search methods return empty results. Indexing methods are no-ops.
#[derive(Debug)]
pub struct DefaultTagIndex;

#[async_trait]
impl TagIndex for DefaultTagIndex {
    async fn index_tags(&self, _id: &MemoryId, _tags: &[String]) -> MemoryResult<()> {
        Ok(())
    }

    async fn search_by_tag(&self, _tag: &str) -> MemoryResult<Vec<MemoryId>> {
        Ok(Vec::new())
    }

    async fn search_by_tags(
        &self,
        _tags: &[String],
        _match_all: bool,
    ) -> MemoryResult<Vec<MemoryId>> {
        Ok(Vec::new())
    }

    async fn remove_object(&self, _id: &MemoryId) -> MemoryResult<()> {
        Ok(())
    }

    async fn rebuild(&self) -> MemoryResult<()> {
        Ok(())
    }

    async fn all_tags(&self) -> MemoryResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn len(&self) -> MemoryResult<usize> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_tag_index() {
        let idx = DefaultTagIndex;
        let id = MemoryId::new();
        let tags = vec!["a".into(), "b".into()];

        assert!(idx.index_tags(&id, &tags).await.is_ok());
        assert!(idx.search_by_tag("a").await.unwrap().is_empty());
        assert!(idx.search_by_tags(&tags, true).await.unwrap().is_empty());
        assert!(idx.remove_object(&id).await.is_ok());
        assert!(idx.rebuild().await.is_ok());
        assert!(idx.all_tags().await.unwrap().is_empty());
        assert_eq!(idx.len().await.unwrap(), 0);
        assert!(idx.is_empty().await.unwrap());
    }

    #[test]
    fn test_default_tag_index_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultTagIndex>();
    }
}
