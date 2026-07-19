use async_trait::async_trait;
use memory_core::{CapacityInfo, MemoryId, MemoryObject, StorageStats};

use crate::error::WorkingMemoryResult;

/// Bounded volatile working memory with LRU eviction.
///
/// WorkingMemory holds the AI's active cognitive state — current goals,
/// active tasks, ongoing conversations, and temporary reasoning data.
/// It is bounded in capacity (entry count and byte budget) and uses
/// LRU eviction when full. Evicted entries are emitted as events.
#[async_trait]
pub trait WorkingMemory: Send + Sync + std::fmt::Debug {
    /// Store a memory object. Evicts LRU entries if at capacity.
    async fn store(&self, object: MemoryObject) -> WorkingMemoryResult<()>;

    /// Recall a memory object by ID. Updates LRU position.
    async fn recall(&self, id: &MemoryId) -> WorkingMemoryResult<Option<MemoryObject>>;

    /// Update an existing memory object. Creates new version.
    async fn update(&self, object: MemoryObject) -> WorkingMemoryResult<()>;

    /// Remove a memory object by ID.
    async fn forget(&self, id: &MemoryId) -> WorkingMemoryResult<()>;

    /// Search working memory by keyword (scans content_type + tags + metadata values).
    async fn search(&self, query: &str, k: usize) -> WorkingMemoryResult<Vec<MemoryObject>>;

    /// Return the `n` most recently accessed objects.
    async fn recent(&self, n: usize) -> WorkingMemoryResult<Vec<MemoryObject>>;

    /// Return capacity information.
    async fn capacity(&self) -> WorkingMemoryResult<CapacityInfo>;

    /// Clear all objects from working memory.
    async fn clear(&self) -> WorkingMemoryResult<()>;

    /// Return storage statistics.
    async fn stats(&self) -> WorkingMemoryResult<StorageStats>;
}

use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;

/// LRU-evicted working memory implementation.
///
/// Stores memory objects in a `HashMap` for O(1) lookup and tracks
/// access order via a `VecDeque` where the front holds the most recently
/// used entry and the back holds the least recently used entry.
/// When the store reaches capacity, the least recently used entry is evicted.
#[derive(Debug)]
pub struct LruWorkingMemory {
    objects: RwLock<HashMap<MemoryId, MemoryObject>>,
    access_order: RwLock<VecDeque<MemoryId>>,
    max_entries: usize,
    max_bytes: u64,
}

impl Default for LruWorkingMemory {
    fn default() -> Self {
        Self {
            objects: RwLock::new(HashMap::new()),
            access_order: RwLock::new(VecDeque::new()),
            max_entries: 1024,
            max_bytes: 64 * 1024 * 1024,
        }
    }
}

impl LruWorkingMemory {
    /// Create a new `LruWorkingMemory` with the given capacity limits.
    pub fn new(max_entries: usize, max_bytes: u64) -> Self {
        Self {
            objects: RwLock::new(HashMap::new()),
            access_order: RwLock::new(VecDeque::new()),
            max_entries,
            max_bytes,
        }
    }

    /// Evict entries until we are under both entry and byte limits.
    fn ensure_capacity(&self) -> WorkingMemoryResult<()> {
        loop {
            let (entry_count, byte_count) = {
                let map = self.objects.read().map_err(|e| {
                    crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}"))
                })?;
                let total_bytes: u64 = map.values().map(|o| o.content.len() as u64).sum();
                (map.len() as u64, total_bytes)
            };

            if entry_count <= self.max_entries as u64 && byte_count <= self.max_bytes {
                return Ok(());
            }

            self.evict_one()?;
        }
    }

    /// Remove the single least recently used entry.
    fn evict_one(&self) -> WorkingMemoryResult<()> {
        let id = {
            let mut order = self
                .access_order
                .write()
                .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
            order.pop_back().ok_or_else(|| {
                crate::WorkingMemoryError::Internal("cannot evict from empty store".into())
            })?
        };

        let mut map = self
            .objects
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        map.remove(&id);
        Ok(())
    }

    /// Compute the total content bytes of all stored objects.
    fn total_bytes(&self) -> WorkingMemoryResult<u64> {
        let map = self
            .objects
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        Ok(map.values().map(|o| o.content.len() as u64).sum())
    }

    /// Move an ID to the front of the access order (most recently used).
    fn touch(&self, id: &MemoryId) -> WorkingMemoryResult<()> {
        let mut order = self
            .access_order
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;

        if let Some(pos) = order.iter().position(|x| x == id) {
            order.remove(pos);
        }
        order.push_front(*id);
        Ok(())
    }
}

#[async_trait]
impl WorkingMemory for LruWorkingMemory {
    async fn store(&self, object: MemoryObject) -> WorkingMemoryResult<()> {
        let id = object.id;

        {
            let mut map = self
                .objects
                .write()
                .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
            let mut order = self
                .access_order
                .write()
                .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;

            // If the object already exists, remove the old entry from the order
            if map.contains_key(&id)
                && let Some(pos) = order.iter().position(|x| *x == id)
            {
                order.remove(pos);
            }

            map.insert(id, object);
            order.push_front(id);
        }

        self.ensure_capacity()?;
        Ok(())
    }

    async fn recall(&self, id: &MemoryId) -> WorkingMemoryResult<Option<MemoryObject>> {
        let exists = {
            let map = self
                .objects
                .read()
                .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
            map.contains_key(id)
        };

        if !exists {
            return Ok(None);
        }

        self.touch(id)?;

        let map = self
            .objects
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        Ok(map.get(id).cloned())
    }

    async fn update(&self, object: MemoryObject) -> WorkingMemoryResult<()> {
        let id = object.id;
        let old_version = {
            let mut map = self
                .objects
                .write()
                .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;

            let existing = map
                .get(&id)
                .ok_or(crate::WorkingMemoryError::EntryNotFound(id))?;

            let new_version = existing.version.next();
            let mut updated = object;
            updated.version = new_version;
            map.insert(id, updated);
            new_version
        };

        self.touch(&id)?;
        let _ = old_version;
        Ok(())
    }

    async fn forget(&self, id: &MemoryId) -> WorkingMemoryResult<()> {
        let mut map = self
            .objects
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        let mut order = self
            .access_order
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;

        map.remove(id);
        if let Some(pos) = order.iter().position(|x| x == id) {
            order.remove(pos);
        }
        Ok(())
    }

    async fn search(&self, query: &str, k: usize) -> WorkingMemoryResult<Vec<MemoryObject>> {
        let query_lower = query.to_lowercase();
        let map = self
            .objects
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        let order = self
            .access_order
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;

        let mut results: Vec<&MemoryObject> = map
            .values()
            .filter(|obj| {
                obj.content_type.to_lowercase().contains(&query_lower)
                    || obj
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
                    || obj
                        .metadata
                        .values()
                        .any(|v| v.to_lowercase().contains(&query_lower))
            })
            .collect();

        // Sort by recency (position in access_order, lower index = more recent)
        results.sort_by_key(|obj| {
            order
                .iter()
                .position(|id| *id == obj.id)
                .unwrap_or(usize::MAX)
        });

        Ok(results.into_iter().take(k).cloned().collect())
    }

    async fn recent(&self, n: usize) -> WorkingMemoryResult<Vec<MemoryObject>> {
        let order = self
            .access_order
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        let map = self
            .objects
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;

        let objs: Vec<MemoryObject> = order
            .iter()
            .take(n)
            .filter_map(|id| map.get(id).cloned())
            .collect();
        Ok(objs)
    }

    async fn capacity(&self) -> WorkingMemoryResult<CapacityInfo> {
        let entry_count = {
            let map = self
                .objects
                .read()
                .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
            map.len()
        };
        let byte_count = self.total_bytes()?;

        Ok(CapacityInfo::new(
            self.max_entries,
            entry_count,
            self.max_bytes,
            byte_count,
        ))
    }

    async fn clear(&self) -> WorkingMemoryResult<()> {
        let mut map = self
            .objects
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        let mut order = self
            .access_order
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;

        map.clear();
        order.clear();
        Ok(())
    }

    async fn stats(&self) -> WorkingMemoryResult<StorageStats> {
        let map = self
            .objects
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;

        let total_objects = map.len() as u64;
        let total_bytes: u64 = map.values().map(|o| o.content.len() as u64).sum();
        let average_object_size = if total_objects > 0 {
            total_bytes as f64 / total_objects as f64
        } else {
            0.0
        };

        let mut type_counts = std::collections::HashMap::new();
        let mut tier_counts = std::collections::HashMap::new();
        for obj in map.values() {
            *type_counts.entry(obj.memory_type).or_insert(0u64) += 1;
            *tier_counts.entry(obj.tier).or_insert(0u64) += 1;
        }

        Ok(StorageStats {
            total_objects,
            total_bytes,
            tier_counts,
            type_counts,
            average_object_size,
        })
    }
}

/// A no-op implementation of [`WorkingMemory`] that returns errors on all operations.
///
/// Useful as a default or placeholder when working memory is not yet configured.
#[derive(Debug)]
pub struct DefaultWorkingMemory;

#[async_trait]
impl WorkingMemory for DefaultWorkingMemory {
    async fn store(&self, _object: MemoryObject) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemory: not configured".into(),
        ))
    }

    async fn recall(&self, _id: &MemoryId) -> WorkingMemoryResult<Option<MemoryObject>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemory: not configured".into(),
        ))
    }

    async fn update(&self, _object: MemoryObject) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemory: not configured".into(),
        ))
    }

    async fn forget(&self, _id: &MemoryId) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemory: not configured".into(),
        ))
    }

    async fn search(&self, _query: &str, _k: usize) -> WorkingMemoryResult<Vec<MemoryObject>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemory: not configured".into(),
        ))
    }

    async fn recent(&self, _n: usize) -> WorkingMemoryResult<Vec<MemoryObject>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemory: not configured".into(),
        ))
    }

    async fn capacity(&self) -> WorkingMemoryResult<CapacityInfo> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemory: not configured".into(),
        ))
    }

    async fn clear(&self) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemory: not configured".into(),
        ))
    }

    async fn stats(&self) -> WorkingMemoryResult<StorageStats> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultWorkingMemory: not configured".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::MemoryObject;
    use std::sync::Arc;

    fn make_obj(content_type: &str, content: Vec<u8>) -> MemoryObject {
        MemoryObject::builder()
            .content_type(content_type)
            .content(content)
            .build()
    }

    fn make_tagged_obj(content_type: &str, tags: Vec<&str>) -> MemoryObject {
        let mut obj = MemoryObject::builder().content_type(content_type).build();
        obj.tags = tags.into_iter().map(String::from).collect();
        obj
    }

    #[tokio::test]
    async fn test_store_and_recall() {
        let wm = LruWorkingMemory::default();
        let obj = make_obj("text/plain", b"hello".to_vec());
        let id = obj.id;

        wm.store(obj).await.unwrap();

        let recalled = wm.recall(&id).await.unwrap();
        assert!(recalled.is_some());
        assert_eq!(recalled.unwrap().content, b"hello");
    }

    #[tokio::test]
    async fn test_recall_nonexistent() {
        let wm = LruWorkingMemory::default();
        let id = MemoryId::new();
        let result = wm.recall(&id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_store_evicts_lru_when_full() {
        let wm = LruWorkingMemory::new(3, 10_000);
        let id1 = MemoryId::new();
        let id2 = MemoryId::new();
        let id3 = MemoryId::new();
        let id4 = MemoryId::new();

        wm.store(make_obj("text", b"1".to_vec()).with_id(id1))
            .await
            .unwrap();
        wm.store(make_obj("text", b"2".to_vec()).with_id(id2))
            .await
            .unwrap();
        wm.store(make_obj("text", b"3".to_vec()).with_id(id3))
            .await
            .unwrap();
        // Access id1 to make it most recent
        wm.recall(&id1).await.unwrap();
        // This should evict id2 (least recently used)
        wm.store(make_obj("text", b"4".to_vec()).with_id(id4))
            .await
            .unwrap();

        assert!(wm.recall(&id2).await.unwrap().is_none());
        assert!(wm.recall(&id1).await.unwrap().is_some());
        assert!(wm.recall(&id3).await.unwrap().is_some());
        assert!(wm.recall(&id4).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_update() {
        let wm = LruWorkingMemory::default();
        let obj = make_obj("text/plain", b"v1".to_vec());
        let id = obj.id;

        wm.store(obj).await.unwrap();

        let updated = make_obj("text/plain", b"v2".to_vec()).with_id(id);
        wm.update(updated).await.unwrap();

        let recalled = wm.recall(&id).await.unwrap().unwrap();
        assert_eq!(recalled.content, b"v2");
        assert_eq!(recalled.version.get(), 2);
    }

    #[tokio::test]
    async fn test_update_nonexistent() {
        let wm = LruWorkingMemory::default();
        let obj = make_obj("text", b"v1".to_vec());
        let result = wm.update(obj).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_forget() {
        let wm = LruWorkingMemory::default();
        let obj = make_obj("text/plain", b"hello".to_vec());
        let id = obj.id;

        wm.store(obj).await.unwrap();
        assert!(wm.recall(&id).await.unwrap().is_some());

        wm.forget(&id).await.unwrap();
        assert!(wm.recall(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_search_by_tag() {
        let wm = LruWorkingMemory::default();

        let obj1 = make_tagged_obj("text", vec!["important", "urgent"]);
        let obj2 = make_tagged_obj("text", vec!["normal"]);
        let obj3 = make_tagged_obj("json", vec!["urgent"]);

        wm.store(obj1).await.unwrap();
        wm.store(obj2).await.unwrap();
        wm.store(obj3).await.unwrap();

        let results = wm.search("urgent", 10).await.unwrap();
        assert_eq!(results.len(), 2);

        let results = wm.search("normal", 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_search_by_content_type() {
        let wm = LruWorkingMemory::default();

        let obj1 = make_obj("text/plain", b"data".to_vec());
        let obj2 = make_obj("application/json", b"{}".to_vec());

        wm.store(obj1).await.unwrap();
        wm.store(obj2).await.unwrap();

        let results = wm.search("json", 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_recent() {
        let wm = LruWorkingMemory::default();

        let obj1 = make_obj("text", b"a".to_vec());
        let obj2 = make_obj("text", b"b".to_vec());
        let obj3 = make_obj("text", b"c".to_vec());
        let id1 = obj1.id;

        wm.store(obj1).await.unwrap();
        wm.store(obj2).await.unwrap();
        wm.store(obj3).await.unwrap();

        // Access id1 to make it most recent
        wm.recall(&id1).await.unwrap();

        let recent = wm.recent(2).await.unwrap();
        assert_eq!(recent.len(), 2);
        // Most recent should be id1, then id3
        assert_eq!(recent[0].id, id1);
    }

    #[tokio::test]
    async fn test_clear() {
        let wm = LruWorkingMemory::default();

        wm.store(make_obj("text", b"a".to_vec())).await.unwrap();
        wm.store(make_obj("text", b"b".to_vec())).await.unwrap();

        wm.clear().await.unwrap();

        let cap = wm.capacity().await.unwrap();
        assert_eq!(cap.current_entries, 0);
    }

    #[tokio::test]
    async fn test_capacity_info() {
        let wm = LruWorkingMemory::new(100, 1024);

        let cap = wm.capacity().await.unwrap();
        assert_eq!(cap.max_entries, 100);
        assert_eq!(cap.max_bytes, 1024);
        assert_eq!(cap.current_entries, 0);

        wm.store(make_obj("text", b"hello".to_vec())).await.unwrap();
        let cap = wm.capacity().await.unwrap();
        assert_eq!(cap.current_entries, 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let wm = LruWorkingMemory::default();

        wm.store(make_obj("text/plain", b"hello".to_vec()))
            .await
            .unwrap();
        wm.store(make_obj("application/json", b"{}".to_vec()))
            .await
            .unwrap();

        let stats = wm.stats().await.unwrap();
        assert_eq!(stats.total_objects, 2);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let wm = Arc::new(LruWorkingMemory::new(1000, 10_000_000));

        let mut handles = Vec::new();
        for i in 0..10 {
            let wm = Arc::clone(&wm);
            handles.push(tokio::spawn(async move {
                let obj = make_obj("text", format!("data-{i}").into_bytes());
                let id = obj.id;
                wm.store(obj).await.unwrap();
                let recalled = wm.recall(&id).await.unwrap();
                assert!(recalled.is_some());
            }));
        }

        for h in handles {
            h.await.unwrap();
        }
    }

    #[test]
    fn test_lru_working_memory_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<LruWorkingMemory>();
        assert_sync::<LruWorkingMemory>();
    }

    /// Helper to create an object with a specific ID (for testing eviction).
    trait WithId {
        fn with_id(self, id: MemoryId) -> MemoryObject;
    }

    impl WithId for MemoryObject {
        fn with_id(mut self, id: MemoryId) -> MemoryObject {
            self.id = id;
            self
        }
    }
}
