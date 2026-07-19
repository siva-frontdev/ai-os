use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use memory_core::{
    MemoryError, MemoryId, MemoryObject, MemoryResult,
    QueryFilter, SortField, SortOrder, StorageStats,
};

use crate::store::MemoryStore;

/// Thread-safe in-memory implementation of [`MemoryStore`].
///
/// Backed by a `RwLock<HashMap<MemoryId, MemoryObject>>`.
/// Suitable for testing, development, and single-node deployments
/// where persistence is not required.
///
/// # Thread Safety
///
/// All operations are protected by a `std::sync::RwLock`. Read operations
/// take a read lock; write operations take a write lock. Critical sections
/// are short (hash map lookups) and never held across `.await` points.
#[derive(Debug)]
pub struct InMemoryStore {
    data: RwLock<HashMap<MemoryId, MemoryObject>>,
}

impl InMemoryStore {
    /// Create a new empty `InMemoryStore`.
    pub fn new() -> Self {
        Self { data: RwLock::new(HashMap::new()) }
    }

    /// Create a new `InMemoryStore` pre-populated with objects.
    pub fn with_objects(objects: Vec<MemoryObject>) -> Self {
        let mut map = HashMap::new();
        for obj in objects {
            map.insert(obj.id, obj);
        }
        Self { data: RwLock::new(map) }
    }

    /// Return the number of stored objects.
    pub fn len(&self) -> MemoryResult<usize> {
        let map = self.data.read().map_err(|e| MemoryError::Internal(e.to_string()))?;
        Ok(map.len())
    }

    /// Return true if the store is empty.
    pub fn is_empty(&self) -> MemoryResult<bool> {
        Ok(self.len()? == 0)
    }

    /// Remove all objects from the store.
    pub fn clear(&self) -> MemoryResult<()> {
        let mut map = self.data.write().map_err(|e| MemoryError::Internal(e.to_string()))?;
        map.clear();
        Ok(())
    }

    fn apply_filter(map: &HashMap<MemoryId, MemoryObject>, filter: &QueryFilter) -> Vec<MemoryObject> {
        let mut results: Vec<&MemoryObject> = map.values().filter(|obj| filter.matches(obj)).collect();

        // Sort
        match filter.sort_by {
            SortField::Timestamp => results.sort_by_key(|obj| obj.timestamp),
            SortField::Priority => results.sort_by_key(|obj| obj.priority),
            SortField::Importance => {
                results.sort_by(|a, b| a.importance.partial_cmp(&b.importance).unwrap_or(std::cmp::Ordering::Equal));
            }
            SortField::Confidence => {
                results.sort_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal));
            }
            SortField::Version => results.sort_by_key(|obj| obj.version),
            SortField::AccessCount => {
                // InMemoryStore doesn't track access count; fall back to timestamp
                results.sort_by_key(|obj| obj.timestamp);
            }
        }

        if filter.sort_order == SortOrder::Descending {
            results.reverse();
        }

        // Paginate
        let offset = filter.offset.min(results.len());
        let limit = filter.limit.max(1);
        results[offset..]
            .iter()
            .take(limit)
            .map(|obj| (*obj).clone())
            .collect()
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn insert(&self, object: MemoryObject) -> MemoryResult<()> {
        let mut map = self.data.write().map_err(|e| MemoryError::Internal(e.to_string()))?;
        if map.contains_key(&object.id) {
            return Err(MemoryError::ObjectNotFound(object.id));
        }
        map.insert(object.id, object);
        Ok(())
    }

    async fn get(&self, id: &MemoryId) -> MemoryResult<Option<MemoryObject>> {
        let map = self.data.read().map_err(|e| MemoryError::Internal(e.to_string()))?;
        Ok(map.get(id).cloned())
    }

    async fn update(&self, object: MemoryObject) -> MemoryResult<()> {
        let mut map = self.data.write().map_err(|e| MemoryError::Internal(e.to_string()))?;
        if !map.contains_key(&object.id) {
            return Err(MemoryError::ObjectNotFound(object.id));
        }
        map.insert(object.id, object);
        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        let mut map = self.data.write().map_err(|e| MemoryError::Internal(e.to_string()))?;
        map.remove(id).ok_or(MemoryError::ObjectNotFound(*id))?;
        Ok(())
    }

    async fn exists(&self, id: &MemoryId) -> MemoryResult<bool> {
        let map = self.data.read().map_err(|e| MemoryError::Internal(e.to_string()))?;
        Ok(map.contains_key(id))
    }

    async fn insert_batch(&self, objects: &[MemoryObject]) -> MemoryResult<()> {
        let mut map = self.data.write().map_err(|e| MemoryError::Internal(e.to_string()))?;
        for obj in objects {
            if map.contains_key(&obj.id) {
                return Err(MemoryError::ObjectNotFound(obj.id));
            }
        }
        for obj in objects {
            map.insert(obj.id, obj.clone());
        }
        Ok(())
    }

    async fn get_batch(&self, ids: &[MemoryId]) -> MemoryResult<Vec<Option<MemoryObject>>> {
        let map = self.data.read().map_err(|e| MemoryError::Internal(e.to_string()))?;
        Ok(ids.iter().map(|id| map.get(id).cloned()).collect())
    }

    async fn delete_batch(&self, ids: &[MemoryId]) -> MemoryResult<()> {
        let mut map = self.data.write().map_err(|e| MemoryError::Internal(e.to_string()))?;
        for id in ids {
            map.remove(id);
        }
        Ok(())
    }

    async fn query(&self, filter: &QueryFilter) -> MemoryResult<Vec<MemoryObject>> {
        let map = self.data.read().map_err(|e| MemoryError::Internal(e.to_string()))?;
        Ok(Self::apply_filter(&map, filter))
    }

    async fn count(&self, filter: &QueryFilter) -> MemoryResult<u64> {
        let map = self.data.read().map_err(|e| MemoryError::Internal(e.to_string()))?;
        let count = map.values().filter(|obj| filter.matches(obj)).count();
        Ok(count as u64)
    }

    async fn flush(&self) -> MemoryResult<()> {
        // No-op for in-memory store
        Ok(())
    }

    async fn compact(&self) -> MemoryResult<()> {
        // No-op for in-memory store
        Ok(())
    }

    async fn stats(&self) -> MemoryResult<StorageStats> {
        let map = self.data.read().map_err(|e| MemoryError::Internal(e.to_string()))?;
        let total_objects = map.len() as u64;
        let total_bytes: u64 = map.values().map(|obj| obj.content.len() as u64).sum();

        let mut tier_counts = HashMap::new();
        let mut type_counts = HashMap::new();

        for obj in map.values() {
            *tier_counts.entry(obj.tier).or_insert(0) += 1;
            *type_counts.entry(obj.memory_type).or_insert(0) += 1;
        }

        let average_object_size = if total_objects > 0 {
            total_bytes as f64 / total_objects as f64
        } else {
            0.0
        };

        Ok(StorageStats {
            total_objects,
            total_bytes,
            tier_counts,
            type_counts,
            average_object_size,
        })
    }
}

// Extension trait to add filter-matching logic to QueryFilter
trait FilterMatch {
    fn matches(&self, obj: &MemoryObject) -> bool;
}

impl FilterMatch for QueryFilter {
    fn matches(&self, obj: &MemoryObject) -> bool {
        if let Some(tier) = &self.tier {
            if &obj.tier != tier {
                return false;
            }
        }
        if let Some(memory_type) = &self.memory_type {
            if &obj.memory_type != memory_type {
                return false;
            }
        }
        if let Some(tags) = &self.tags {
            if !tags.iter().any(|tag| obj.tags.contains(tag)) {
                return false;
            }
        }
        if let Some(source) = &self.source {
            if &obj.source != source {
                return false;
            }
        }
        if let Some(created_by) = &self.created_by {
            if &obj.created_by != created_by {
                return false;
            }
        }
        if let Some(range) = &self.time_range {
            if obj.timestamp < range.start || obj.timestamp > range.end {
                return false;
            }
        }
        if let Some(min) = self.priority_min {
            if obj.priority < min {
                return false;
            }
        }
        if let Some(min) = self.importance_min {
            if obj.importance < min {
                return false;
            }
        }
        if let Some(min) = self.confidence_min {
            if obj.confidence < min {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::{MemoryTier, MemoryType};

    fn test_object(content_type: &str, tier: MemoryTier, memory_type: MemoryType) -> MemoryObject {
        MemoryObject::builder()
            .content_type(content_type)
            .content(content_type.as_bytes().to_vec())
            .tier(tier)
            .memory_type(memory_type)
            .build()
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let store = InMemoryStore::new();
        let obj = test_object("text", MemoryTier::Working, MemoryType::Working);

        assert!(store.insert(obj.clone()).await.is_ok());
        let retrieved = store.get(&obj.id).await.unwrap();
        assert_eq!(retrieved, Some(obj));
    }

    #[tokio::test]
    async fn test_insert_duplicate() {
        let store = InMemoryStore::new();
        let obj = test_object("text", MemoryTier::Working, MemoryType::Working);

        assert!(store.insert(obj.clone()).await.is_ok());
        assert!(store.insert(obj).await.is_err());
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let store = InMemoryStore::new();
        let id = MemoryId::new();
        assert_eq!(store.get(&id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_update() {
        let store = InMemoryStore::new();
        let mut obj = test_object("text", MemoryTier::Working, MemoryType::Working);

        store.insert(obj.clone()).await.unwrap();

        obj.content_type = "updated".into();
        assert!(store.update(obj.clone()).await.is_ok());

        let retrieved = store.get(&obj.id).await.unwrap().unwrap();
        assert_eq!(retrieved.content_type, "updated");
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let store = InMemoryStore::new();
        let obj = test_object("text", MemoryTier::Working, MemoryType::Working);
        assert!(store.update(obj).await.is_err());
    }

    #[tokio::test]
    async fn test_delete() {
        let store = InMemoryStore::new();
        let obj = test_object("text", MemoryTier::Working, MemoryType::Working);

        store.insert(obj.clone()).await.unwrap();
        assert!(store.delete(&obj.id).await.is_ok());
        assert_eq!(store.get(&obj.id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let store = InMemoryStore::new();
        let id = MemoryId::new();
        assert!(store.delete(&id).await.is_err());
    }

    #[tokio::test]
    async fn test_exists() {
        let store = InMemoryStore::new();
        let obj = test_object("text", MemoryTier::Working, MemoryType::Working);

        assert!(!store.exists(&obj.id).await.unwrap());
        store.insert(obj.clone()).await.unwrap();
        assert!(store.exists(&obj.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let store = InMemoryStore::new();
        let objs: Vec<MemoryObject> = (0..5)
            .map(|i| {
                MemoryObject::builder()
                    .content_type(format!("type-{}", i))
                    .content(vec![i as u8])
                    .build()
            })
            .collect();

        // Insert batch
        assert!(store.insert_batch(&objs).await.is_ok());

        // Get batch
        let ids: Vec<MemoryId> = objs.iter().map(|o| o.id).collect();
        let results = store.get_batch(&ids).await.unwrap();
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.is_some()));

        // Delete batch
        assert!(store.delete_batch(&ids).await.is_ok());
        assert_eq!(store.len().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_query_filter() {
        let store = InMemoryStore::new();

        let working = test_object("working", MemoryTier::Working, MemoryType::Working);
        let episodic = test_object("episodic", MemoryTier::Episodic, MemoryType::Episodic);
        let semantic = test_object("semantic", MemoryTier::Semantic, MemoryType::Semantic);

        store.insert(working.clone()).await.unwrap();
        store.insert(episodic.clone()).await.unwrap();
        store.insert(semantic.clone()).await.unwrap();

        // Filter by tier
        let filter = QueryFilter {
            tier: Some(MemoryTier::Working),
            ..Default::default()
        };
        let results = store.query(&filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, working.id);
    }

    #[tokio::test]
    async fn test_count() {
        let store = InMemoryStore::new();
        for i in 0..10 {
            let obj = MemoryObject::builder()
                .content_type("text")
                .content(vec![i])
                .tier(if i < 5 { MemoryTier::Working } else { MemoryTier::Episodic })
                .build();
            store.insert(obj).await.unwrap();
        }

        let count = store.count(&QueryFilter::default()).await.unwrap();
        assert_eq!(count, 10);

        let filter = QueryFilter {
            tier: Some(MemoryTier::Working),
            ..Default::default()
        };
        assert_eq!(store.count(&filter).await.unwrap(), 5);
    }

    #[tokio::test]
    async fn test_stats() {
        let store = InMemoryStore::new();
        assert!(store.stats().await.is_ok());

        let obj = test_object("text", MemoryTier::Working, MemoryType::Working);
        store.insert(obj).await.unwrap();

        let stats = store.stats().await.unwrap();
        assert_eq!(stats.total_objects, 1);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let store = std::sync::Arc::new(InMemoryStore::new());
        let mut handles = vec![];

        for i in 0..10 {
            let store = store.clone();
            handles.push(tokio::spawn(async move {
                let obj = MemoryObject::builder()
                    .content_type(format!("thread-{}", i))
                    .content(vec![i])
                    .build();
                store.insert(obj).await.unwrap();
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(store.len().unwrap(), 10);
    }

    #[test]
    fn test_in_memory_store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<InMemoryStore>();
    }
}
