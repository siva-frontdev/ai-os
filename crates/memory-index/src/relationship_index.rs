use async_trait::async_trait;
use memory_core::{MemoryId, MemoryResult, RelationType};

/// Index and search memory objects by their relationships.
///
/// Provides graph adjacency queries: find related objects, find incoming
/// references, and filter by relationship type. This is the foundation
/// for knowledge graph traversal.
#[async_trait]
pub trait RelationshipIndex: Send + Sync + std::fmt::Debug {
    /// Index a relationship from a source object to a target object.
    async fn index_relationship(&self, source: &MemoryId, target: &MemoryId, relation_type: &RelationType) -> MemoryResult<()>;

    /// Find all objects related to the given object, optionally filtered by relation type.
    ///
    /// Returns IDs of objects reachable via outgoing relationships from the source.
    async fn find_related(&self, id: &MemoryId, relation_type: Option<&RelationType>) -> MemoryResult<Vec<MemoryId>>;

    /// Find all objects that reference the given object, optionally filtered by relation type.
    ///
    /// Returns IDs of objects that have incoming relationships to the target.
    async fn find_incoming(&self, id: &MemoryId, relation_type: Option<&RelationType>) -> MemoryResult<Vec<MemoryId>>;

    /// Remove all relationships involving this object (both incoming and outgoing).
    async fn remove_object(&self, id: &MemoryId) -> MemoryResult<()>;

    /// Rebuild the entire relationship index from scratch.
    async fn rebuild(&self) -> MemoryResult<()>;

    /// Return the total number of indexed relationships.
    async fn len(&self) -> MemoryResult<usize>;

    /// Return true if the index is empty.
    async fn is_empty(&self) -> MemoryResult<bool> {
        Ok(self.len().await? == 0)
    }
}

/// Default no-op implementation of [`RelationshipIndex`].
///
/// All search methods return empty results. Indexing methods are no-ops.
#[derive(Debug)]
pub struct DefaultRelationshipIndex;

#[async_trait]
impl RelationshipIndex for DefaultRelationshipIndex {
    async fn index_relationship(&self, _source: &MemoryId, _target: &MemoryId, _relation_type: &RelationType) -> MemoryResult<()> {
        Ok(())
    }

    async fn find_related(&self, _id: &MemoryId, _relation_type: Option<&RelationType>) -> MemoryResult<Vec<MemoryId>> {
        Ok(Vec::new())
    }

    async fn find_incoming(&self, _id: &MemoryId, _relation_type: Option<&RelationType>) -> MemoryResult<Vec<MemoryId>> {
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
    async fn test_default_relationship_index() {
        let idx = DefaultRelationshipIndex;
        let a = MemoryId::new();
        let b = MemoryId::new();

        assert!(idx.index_relationship(&a, &b, &RelationType::References).await.is_ok());
        assert!(idx.find_related(&a, None).await.unwrap().is_empty());
        assert!(idx.find_incoming(&b, Some(&RelationType::References)).await.unwrap().is_empty());
        assert!(idx.remove_object(&a).await.is_ok());
        assert!(idx.rebuild().await.is_ok());
        assert_eq!(idx.len().await.unwrap(), 0);
        assert!(idx.is_empty().await.unwrap());
    }

    #[test]
    fn test_default_relationship_index_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultRelationshipIndex>();
    }
}
