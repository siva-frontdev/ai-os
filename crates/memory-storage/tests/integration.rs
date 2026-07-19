use memory_core::{MemoryId, MemoryObject, MemoryTier, MemoryType, QueryFilter};
use memory_storage::{InMemoryStore, MemoryStore};

fn make_test_obj(content_type: &str, tier: MemoryTier, memory_type: MemoryType) -> MemoryObject {
    MemoryObject::builder()
        .content_type(content_type)
        .content(content_type.as_bytes().to_vec())
        .tier(tier)
        .memory_type(memory_type)
        .build()
}

#[tokio::test]
async fn test_full_lifecycle() {
    let store = InMemoryStore::new();

    // Create
    let obj = make_test_obj("text", MemoryTier::Working, MemoryType::Working);
    assert!(store.insert(obj.clone()).await.is_ok());

    // Read
    let retrieved = store.get(&obj.id).await.unwrap().expect("should exist");
    assert_eq!(retrieved.id, obj.id);
    assert_eq!(retrieved.content_type, "text");

    // Update
    let mut updated = obj;
    updated.content_type = "updated".into();
    assert!(store.update(updated.clone()).await.is_ok());

    let retrieved = store.get(&updated.id).await.unwrap().expect("should exist");
    assert_eq!(retrieved.content_type, "updated");

    // Delete
    assert!(store.delete(&updated.id).await.is_ok());
    assert!(store.get(&updated.id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_batch_end_to_end() {
    let store = InMemoryStore::new();
    let objects: Vec<MemoryObject> = (0..10)
        .map(|i| {
            MemoryObject::builder()
                .content_type(format!("type-{}", i))
                .content(vec![i as u8])
                .tier(if i % 2 == 0 {
                    MemoryTier::Working
                } else {
                    MemoryTier::Episodic
                })
                .memory_type(if i < 5 {
                    MemoryType::Working
                } else {
                    MemoryType::Episodic
                })
                .build()
        })
        .collect();

    // Batch insert
    assert!(store.insert_batch(&objects).await.is_ok());

    // Batch get all
    let ids: Vec<MemoryId> = objects.iter().map(|o| o.id).collect();
    let results = store.get_batch(&ids).await.unwrap();
    assert_eq!(results.len(), 10);
    assert!(results.iter().all(|r| r.is_some()));

    // Query by tier
    let filter = QueryFilter {
        tier: Some(MemoryTier::Working),
        ..Default::default()
    };
    let working = store.query(&filter).await.unwrap();
    assert_eq!(working.len(), 5);

    // Count by tier
    let count = store.count(&filter).await.unwrap();
    assert_eq!(count, 5);

    // Query by memory type
    let filter = QueryFilter {
        memory_type: Some(MemoryType::Episodic),
        ..Default::default()
    };
    let episodic = store.query(&filter).await.unwrap();
    assert_eq!(episodic.len(), 5);

    // Batch delete (half)
    let to_delete: Vec<MemoryId> = ids.iter().take(5).copied().collect();
    assert!(store.delete_batch(&to_delete).await.is_ok());
    assert_eq!(store.len().unwrap(), 5);

    // Verify deleted are gone
    let results = store.get_batch(&to_delete).await.unwrap();
    assert!(results.iter().all(|r| r.is_none()));

    // Stats
    let stats = store.stats().await.unwrap();
    assert_eq!(stats.total_objects, 5);
}

#[tokio::test]
async fn test_concurrent_inserts() {
    let store = std::sync::Arc::new(InMemoryStore::new());
    let mut handles = vec![];

    for i in 0..50 {
        let store = store.clone();
        handles.push(tokio::spawn(async move {
            let obj = make_test_obj(
                &format!("concurrent-{}", i),
                MemoryTier::Working,
                MemoryType::Working,
            );
            store.insert(obj).await.unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(store.len().unwrap(), 50);
}

#[tokio::test]
async fn test_query_pagination() {
    let store = InMemoryStore::new();
    for i in 0..50 {
        let obj = MemoryObject::builder()
            .content_type(format!("item-{}", i))
            .content(vec![i as u8])
            .tier(MemoryTier::Working)
            .build();
        store.insert(obj).await.unwrap();
    }

    // First page
    let filter = QueryFilter {
        limit: 10,
        offset: 0,
        ..Default::default()
    };
    let page1 = store.query(&filter).await.unwrap();
    assert_eq!(page1.len(), 10);

    // Second page
    let filter = QueryFilter {
        limit: 10,
        offset: 10,
        ..Default::default()
    };
    let page2 = store.query(&filter).await.unwrap();
    assert_eq!(page2.len(), 10);

    // Pages should not overlap
    let ids1: Vec<MemoryId> = page1.iter().map(|o| o.id).collect();
    let ids2: Vec<MemoryId> = page2.iter().map(|o| o.id).collect();
    for id in &ids1 {
        assert!(!ids2.contains(id), "page overlap detected");
    }
}

#[tokio::test]
async fn test_store_flush_and_compact() {
    let store = InMemoryStore::new();
    assert!(store.flush().await.is_ok());
    assert!(store.compact().await.is_ok());
}

#[tokio::test]
async fn test_empty_store_operations() {
    let store = InMemoryStore::new();

    assert!(store.is_empty().unwrap());
    assert_eq!(store.len().unwrap(), 0);

    let stats = store.stats().await.unwrap();
    assert_eq!(stats.total_objects, 0);

    let results = store.query(&QueryFilter::default()).await.unwrap();
    assert!(results.is_empty());

    let count = store.count(&QueryFilter::default()).await.unwrap();
    assert_eq!(count, 0);
}
