use memory_core::{MemoryId, RelationType, Timestamp};
use memory_index::*;

/// Verify that all four default index implementations compose together
/// and return expected default values.
#[tokio::test]
async fn test_all_indices_composition() {
    let metadata_idx = DefaultMetadataIndex;
    let tag_idx = DefaultTagIndex;
    let rel_idx = DefaultRelationshipIndex;
    let time_idx = DefaultTimeIndex;

    let id = MemoryId::new();

    // All should accept index operations without error
    assert!(
        metadata_idx
            .index_metadata(&id, "key", "value")
            .await
            .is_ok()
    );
    assert!(tag_idx.index_tags(&id, &["test".into()]).await.is_ok());
    assert!(
        rel_idx
            .index_relationship(&id, &MemoryId::new(), &RelationType::References)
            .await
            .is_ok()
    );
    assert!(time_idx.index_time(&id, Timestamp::now()).await.is_ok());

    // All search methods should return empty (not error)
    assert!(
        metadata_idx
            .search_by_metadata("key", "value")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(tag_idx.search_by_tag("test").await.unwrap().is_empty());
    assert!(rel_idx.find_related(&id, None).await.unwrap().is_empty());
    assert!(
        time_idx
            .search_by_time_range(&(Timestamp::MIN..Timestamp::MAX))
            .await
            .unwrap()
            .is_empty()
    );

    // All should accept remove and rebuild
    assert!(metadata_idx.remove_object(&id).await.is_ok());
    assert!(tag_idx.remove_object(&id).await.is_ok());
    assert!(rel_idx.remove_object(&id).await.is_ok());
    assert!(time_idx.remove_object(&id).await.is_ok());

    assert!(metadata_idx.rebuild().await.is_ok());
    assert!(tag_idx.rebuild().await.is_ok());
    assert!(rel_idx.rebuild().await.is_ok());
    assert!(time_idx.rebuild().await.is_ok());

    // All should report empty
    assert!(metadata_idx.is_empty().await.unwrap());
    assert!(tag_idx.is_empty().await.unwrap());
    assert!(rel_idx.is_empty().await.unwrap());
    assert!(time_idx.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_tag_index_or_vs_and_semantics() {
    let idx = DefaultTagIndex;
    let _id = MemoryId::new();
    let tags = vec!["a".into(), "b".into()];

    // Both OR and AND return empty for default implementation
    let or_results = idx.search_by_tags(&tags, false).await.unwrap();
    assert!(or_results.is_empty());

    let and_results = idx.search_by_tags(&tags, true).await.unwrap();
    assert!(and_results.is_empty());
}

#[tokio::test]
async fn test_relationship_index_direction() {
    let idx = DefaultRelationshipIndex;
    let a = MemoryId::new();
    let b = MemoryId::new();

    idx.index_relationship(&a, &b, &RelationType::References)
        .await
        .unwrap();

    // Outgoing from A should include B (but default returns empty)
    let outgoing = idx.find_related(&a, None).await.unwrap();
    assert!(outgoing.is_empty());

    // Incoming to B should include A (but default returns empty)
    let incoming = idx.find_incoming(&b, None).await.unwrap();
    assert!(incoming.is_empty());
}

#[tokio::test]
async fn test_time_index_temporal_queries() {
    let idx = DefaultTimeIndex;
    let now = Timestamp::now();

    // Default implementation returns None for oldest/newest
    assert!(idx.oldest_timestamp().await.unwrap().is_none());
    assert!(idx.newest_timestamp().await.unwrap().is_none());

    // After indexing, should still be empty (no-op)
    let id = MemoryId::new();
    idx.index_time(&id, now).await.unwrap();
    assert!(idx.oldest_timestamp().await.unwrap().is_none());

    // Range query returns empty
    let results = idx
        .search_by_time_range(&(now..Timestamp::MAX))
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_metadata_index_prefix_search() {
    let idx = DefaultMetadataIndex;
    let id = MemoryId::new();

    idx.index_metadata(&id, "path", "/home/user/docs/file.txt")
        .await
        .unwrap();

    // Prefix search returns empty for default
    let results = idx
        .search_by_metadata_prefix("path", "/home/user")
        .await
        .unwrap();
    assert!(results.is_empty());
}
