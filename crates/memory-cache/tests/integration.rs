use std::time::Duration;

use memory_cache::{LruMemoryCache, MemoryCache};
use memory_core::MemoryObject;

fn make_obj(content: Vec<u8>) -> MemoryObject {
    MemoryObject::builder()
        .content_type("text/plain")
        .content(content)
        .build()
}

#[tokio::test]
async fn full_lifecycle() {
    let cache = LruMemoryCache::default();
    let obj = make_obj(vec![1, 2, 3]);

    cache.set("k1".into(), obj.clone(), None).await.unwrap();

    let hit = cache.get(&"k1".into()).await.unwrap().expect("should be a hit");
    assert_eq!(hit.object.content, vec![1, 2, 3]);

    let obj2 = make_obj(vec![4, 5, 6]);
    cache.set("k1".into(), obj2.clone(), None).await.unwrap();

    let hit2 = cache.get(&"k1".into()).await.unwrap().expect("should still be a hit");
    assert_eq!(hit2.object.content, vec![4, 5, 6]);

    cache.remove(&"k1".into()).await.unwrap();

    let miss = cache.get(&"k1".into()).await.unwrap();
    assert!(miss.is_none());
}

#[tokio::test]
async fn ttl_expiration_and_recache() {
    let cache = LruMemoryCache::default();

    let obj = make_obj(vec![0xFF]);
    cache
        .set("ttl".into(), obj, Some(Duration::from_millis(10)))
        .await
        .unwrap();

    assert!(cache.contains(&"ttl".into()).await.unwrap());

    tokio::time::sleep(Duration::from_millis(30)).await;

    assert!(!cache.contains(&"ttl".into()).await.unwrap());
    assert!(cache.get(&"ttl".into()).await.unwrap().is_none());

    let obj2 = make_obj(vec![0xAA]);
    cache.set("ttl".into(), obj2, None).await.unwrap();
    assert!(cache.contains(&"ttl".into()).await.unwrap());

    let hit = cache.get(&"ttl".into()).await.unwrap().unwrap();
    assert_eq!(hit.object.content, vec![0xAA]);
}

#[tokio::test]
async fn size_limit_eviction() {
    let cache = LruMemoryCache::new(100, 600);

    let a = make_obj(vec![0u8; 400]);
    cache.set("a".into(), a, None).await.unwrap();

    let b = make_obj(vec![0u8; 400]);
    cache.set("b".into(), b, None).await.unwrap();

    assert!(cache.get(&"a".into()).await.unwrap().is_none());
    assert!(cache.get(&"b".into()).await.unwrap().is_some());

    let stats = cache.stats().await.unwrap();
    assert_eq!(stats.evictions, 1);
    assert!(stats.total_bytes <= 600);
}

#[tokio::test]
async fn concurrent_access() {
    let cache = std::sync::Arc::new(LruMemoryCache::new(500, 1_000_000));
    let mut handles = Vec::new();

    let n_tasks = 10;
    let ops_per_task = 50;

    for t in 0..n_tasks {
        let c = cache.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..ops_per_task {
                let key = format!("task{t}_item{i}");
                let obj = make_obj(vec![t as u8; 64]);
                c.set(key.clone(), obj, None).await.unwrap();
                let _ = c.get(&key).await;
                let _ = c.contains(&key).await;
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let stats = cache.stats().await.unwrap();
    assert!(stats.hits > 0);
}
