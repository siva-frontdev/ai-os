use memory_core::MemoryObject;
use memory_working::*;

fn make_obj(content_type: &str, content: Vec<u8>) -> MemoryObject {
    MemoryObject::builder()
        .content_type(content_type)
        .content(content)
        .build()
}

#[tokio::test]
async fn full_lifecycle_store_recall_update_forget() {
    let wm = LruWorkingMemory::default();

    let obj = make_obj("text/plain", b"initial".to_vec());
    let id = obj.id;

    // Store
    wm.store(obj).await.unwrap();
    let cap = wm.capacity().await.unwrap();
    assert_eq!(cap.current_entries, 1);

    // Recall
    let recalled = wm.recall(&id).await.unwrap();
    assert!(recalled.is_some());
    assert_eq!(recalled.unwrap().content, b"initial");

    // Update
    let updated = make_obj("text/plain", b"modified".to_vec()).with_id(id);
    wm.update(updated).await.unwrap();

    let recalled = wm.recall(&id).await.unwrap().unwrap();
    assert_eq!(recalled.content, b"modified");
    assert!(recalled.version.get() >= 2);

    // Forget
    wm.forget(&id).await.unwrap();
    let recalled = wm.recall(&id).await.unwrap();
    assert!(recalled.is_none());

    let cap = wm.capacity().await.unwrap();
    assert_eq!(cap.current_entries, 0);
}

#[tokio::test]
async fn lru_eviction_exceeding_capacity() {
    let wm = LruWorkingMemory::new(3, 10_000);

    let obj1 = make_obj("text", b"1".to_vec());
    let obj2 = make_obj("text", b"2".to_vec());
    let obj3 = make_obj("text", b"3".to_vec());
    let id1 = obj1.id;
    let id2 = obj2.id;
    let id3 = obj3.id;

    // Fill to capacity
    wm.store(obj1).await.unwrap();
    wm.store(obj2).await.unwrap();
    wm.store(obj3).await.unwrap();

    // Access id1 to make it most recent
    wm.recall(&id1).await.unwrap();

    // Add a 4th item — should evict the LRU item (id2)
    let obj4 = make_obj("text", b"4".to_vec());
    let id4 = obj4.id;
    wm.store(obj4).await.unwrap();

    // id2 should be evicted
    assert!(wm.recall(&id2).await.unwrap().is_none());
    // id1, id3, id4 should still be present
    assert!(wm.recall(&id1).await.unwrap().is_some());
    assert!(wm.recall(&id3).await.unwrap().is_some());
    assert!(wm.recall(&id4).await.unwrap().is_some());

    let cap = wm.capacity().await.unwrap();
    assert_eq!(cap.current_entries, 3);
}

#[tokio::test]
async fn scratchpad_read_write_clear() {
    let pad = InMemoryScratchpad::new();

    pad.set("reasoning", b"step1: analyze".to_vec())
        .await
        .unwrap();
    pad.set("intermediate", b"42".to_vec()).await.unwrap();

    let val = pad.get("reasoning").await.unwrap();
    assert_eq!(val, Some(b"step1: analyze".to_vec()));

    assert_eq!(pad.len().await.unwrap(), 2);

    let mut keys = pad.keys().await.unwrap();
    keys.sort();
    assert_eq!(keys, vec!["intermediate", "reasoning"]);

    pad.remove("intermediate").await.unwrap();
    assert_eq!(pad.len().await.unwrap(), 1);

    pad.clear().await.unwrap();
    assert_eq!(pad.len().await.unwrap(), 0);
}

#[tokio::test]
async fn attention_queue_ordering() {
    let am = PriorityAttentionManager::new();

    // Enqueue items with various priorities
    am.enqueue("low-priority".into(), 10).await.unwrap();
    am.enqueue("high-priority".into(), 100).await.unwrap();
    am.enqueue("medium-priority".into(), 50).await.unwrap();

    // Verify ordering via list
    let items = am.list().await.unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].0, "high-priority");
    assert_eq!(items[1].0, "medium-priority");
    assert_eq!(items[2].0, "low-priority");

    // Dequeue should return highest priority first
    assert_eq!(am.dequeue().await.unwrap(), Some("high-priority".into()));
    assert_eq!(am.dequeue().await.unwrap(), Some("medium-priority".into()));
    assert_eq!(am.dequeue().await.unwrap(), Some("low-priority".into()));
    assert_eq!(am.dequeue().await.unwrap(), None);
}

#[tokio::test]
async fn attention_queue_peek() {
    let am = PriorityAttentionManager::new();
    am.enqueue("important".into(), 200).await.unwrap();
    am.enqueue("less-important".into(), 50).await.unwrap();

    let peeked = am.peek().await.unwrap();
    assert_eq!(peeked, Some(("important".into(), 200)));

    // Peek should not remove
    assert_eq!(am.len().await.unwrap(), 2);
}

#[tokio::test]
async fn attention_queue_remove_specific() {
    let am = PriorityAttentionManager::new();
    am.enqueue("keep".into(), 50).await.unwrap();
    am.enqueue("remove-me".into(), 30).await.unwrap();
    am.enqueue("also-keep".into(), 40).await.unwrap();

    am.remove("remove-me").await.unwrap();

    let items = am.list().await.unwrap();
    assert_eq!(items.len(), 2);
    for (name, _) in &items {
        assert_ne!(name, "remove-me");
    }
}

#[tokio::test]
async fn session_isolation_not_supported_in_basic_impl() {
    // The LruWorkingMemory does not provide session isolation.
    // Verify that the DefaultWorkingMemorySession returns errors.
    let session = DefaultWorkingMemorySession;
    let sid = uuid::Uuid::new_v4();
    let obj = make_obj("text", b"data".to_vec());

    let result = session.store(&sid, obj).await;
    assert!(
        result.is_err(),
        "DefaultWorkingMemorySession should return error"
    );
}

/// Helper trait to set a specific ID on a MemoryObject for testing.
trait WithId {
    fn with_id(self, id: memory_core::MemoryId) -> MemoryObject;
}

impl WithId for MemoryObject {
    fn with_id(mut self, id: memory_core::MemoryId) -> MemoryObject {
        self.id = id;
        self
    }
}
