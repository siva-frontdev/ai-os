use std::sync::RwLock;

use async_trait::async_trait;
use memory_core::{
    MemoryError, MemoryId, MemoryObject, MemoryResult, QueryFilter, StorageStats,
};

use crate::store::MemoryStore;

/// A configurable mock implementation of [`MemoryStore`] for testing.
///
/// `MockStore` records all operations for later assertion and provides
/// configurable return values. It is designed for testing higher-level
/// memory subsystems that depend on a [`MemoryStore`].
///
/// # Usage
///
/// ```rust
/// use memory_storage::MockStore;
///
/// let mock = MockStore::new();
/// assert_eq!(mock.insert_calls(), 0);
/// assert_eq!(mock.get_calls(), 0);
/// ```
#[derive(Debug)]
pub struct MockStore {
    // Storage for pre-configured objects
    objects: RwLock<std::collections::HashMap<MemoryId, MemoryObject>>,
    // Operation counters
    insert_calls: RwLock<u64>,
    get_calls: RwLock<u64>,
    update_calls: RwLock<u64>,
    delete_calls: RwLock<u64>,
    exists_calls: RwLock<u64>,
    query_calls: RwLock<u64>,
    // Configurable error injection
    fail_on_insert: RwLock<bool>,
    fail_on_get: RwLock<bool>,
    fail_on_update: RwLock<bool>,
    fail_on_delete: RwLock<bool>,
}

impl MockStore {
    /// Create a new empty `MockStore`.
    pub fn new() -> Self {
        Self {
            objects: RwLock::new(std::collections::HashMap::new()),
            insert_calls: RwLock::new(0),
            get_calls: RwLock::new(0),
            update_calls: RwLock::new(0),
            delete_calls: RwLock::new(0),
            exists_calls: RwLock::new(0),
            query_calls: RwLock::new(0),
            fail_on_insert: RwLock::new(false),
            fail_on_get: RwLock::new(false),
            fail_on_update: RwLock::new(false),
            fail_on_delete: RwLock::new(false),
        }
    }

    /// Add an object that will be returned by `get()`.
    pub fn add_expected_object(&self, object: MemoryObject) {
        let mut map = self.objects.write().expect("lock");
        map.insert(object.id, object);
    }

    /// Set whether `insert()` should return an error.
    pub fn set_fail_on_insert(&self, fail: bool) {
        *self.fail_on_insert.write().expect("lock") = fail;
    }

    /// Set whether `get()` should return an error.
    pub fn set_fail_on_get(&self, fail: bool) {
        *self.fail_on_get.write().expect("lock") = fail;
    }

    /// Return the number of `insert()` calls.
    pub fn insert_calls(&self) -> u64 {
        *self.insert_calls.read().expect("lock")
    }

    /// Return the number of `get()` calls.
    pub fn get_calls(&self) -> u64 {
        *self.get_calls.read().expect("lock")
    }

    /// Return the number of `update()` calls.
    pub fn update_calls(&self) -> u64 {
        *self.update_calls.read().expect("lock")
    }

    /// Return the number of `delete()` calls.
    pub fn delete_calls(&self) -> u64 {
        *self.delete_calls.read().expect("lock")
    }

    /// Return the number of `query()` calls.
    pub fn query_calls(&self) -> u64 {
        *self.query_calls.read().expect("lock")
    }
}

impl Default for MockStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryStore for MockStore {
    async fn insert(&self, object: MemoryObject) -> MemoryResult<()> {
        *self.insert_calls.write().expect("lock") += 1;
        if *self.fail_on_insert.read().expect("lock") {
            return Err(MemoryError::StorageBackendError("MockStore: insert forced failure".into()));
        }
        let mut map = self.objects.write().expect("lock");
        map.insert(object.id, object);
        Ok(())
    }

    async fn get(&self, id: &MemoryId) -> MemoryResult<Option<MemoryObject>> {
        *self.get_calls.write().expect("lock") += 1;
        if *self.fail_on_get.read().expect("lock") {
            return Err(MemoryError::StorageBackendError("MockStore: get forced failure".into()));
        }
        let map = self.objects.read().expect("lock");
        Ok(map.get(id).cloned())
    }

    async fn update(&self, object: MemoryObject) -> MemoryResult<()> {
        *self.update_calls.write().expect("lock") += 1;
        if *self.fail_on_update.read().expect("lock") {
            return Err(MemoryError::StorageBackendError("MockStore: update forced failure".into()));
        }
        let mut map = self.objects.write().expect("lock");
        if !map.contains_key(&object.id) {
            return Err(MemoryError::ObjectNotFound(object.id));
        }
        map.insert(object.id, object);
        Ok(())
    }

    async fn delete(&self, id: &MemoryId) -> MemoryResult<()> {
        *self.delete_calls.write().expect("lock") += 1;
        if *self.fail_on_delete.read().expect("lock") {
            return Err(MemoryError::StorageBackendError("MockStore: delete forced failure".into()));
        }
        let mut map = self.objects.write().expect("lock");
        map.remove(id);
        Ok(())
    }

    async fn exists(&self, id: &MemoryId) -> MemoryResult<bool> {
        *self.exists_calls.write().expect("lock") += 1;
        let map = self.objects.read().expect("lock");
        Ok(map.contains_key(id))
    }

    async fn insert_batch(&self, objects: &[MemoryObject]) -> MemoryResult<()> {
        let mut map = self.objects.write().expect("lock");
        for obj in objects {
            map.insert(obj.id, obj.clone());
        }
        Ok(())
    }

    async fn get_batch(&self, ids: &[MemoryId]) -> MemoryResult<Vec<Option<MemoryObject>>> {
        let map = self.objects.read().expect("lock");
        Ok(ids.iter().map(|id| map.get(id).cloned()).collect())
    }

    async fn delete_batch(&self, ids: &[MemoryId]) -> MemoryResult<()> {
        let mut map = self.objects.write().expect("lock");
        for id in ids {
            map.remove(id);
        }
        Ok(())
    }

    async fn query(&self, _filter: &QueryFilter) -> MemoryResult<Vec<MemoryObject>> {
        *self.query_calls.write().expect("lock") += 1;
        let map = self.objects.read().expect("lock");
        Ok(map.values().cloned().collect())
    }

    async fn count(&self, _filter: &QueryFilter) -> MemoryResult<u64> {
        let map = self.objects.read().expect("lock");
        Ok(map.len() as u64)
    }

    async fn flush(&self) -> MemoryResult<()> {
        Ok(())
    }

    async fn compact(&self) -> MemoryResult<()> {
        Ok(())
    }

    async fn stats(&self) -> MemoryResult<StorageStats> {
        let map = self.objects.read().expect("lock");
        Ok(StorageStats {
            total_objects: map.len() as u64,
            total_bytes: map.values().map(|o| o.content.len() as u64).sum(),
            tier_counts: std::collections::HashMap::new(),
            type_counts: std::collections::HashMap::new(),
            average_object_size: 0.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::MemoryType;

    #[tokio::test]
    async fn test_mock_tracks_calls() {
        let mock = MockStore::new();
        let obj = MemoryObject::builder()
            .content_type("text")
            .content(vec![])
            .memory_type(MemoryType::Working)
            .build();

        mock.insert(obj.clone()).await.unwrap();
        assert_eq!(mock.insert_calls(), 1);

        let _ = mock.get(&obj.id).await;
        assert_eq!(mock.get_calls(), 1);
    }

    #[tokio::test]
    async fn test_mock_error_injection() {
        let mock = MockStore::new();
        mock.set_fail_on_insert(true);

        let obj = MemoryObject::builder()
            .content_type("text")
            .content(vec![])
            .build();

        assert!(mock.insert(obj).await.is_err());
    }

    #[tokio::test]
    async fn test_mock_preset_object() {
        let mock = MockStore::new();
        let obj = MemoryObject::builder()
            .content_type("text")
            .content(vec![])
            .build();

        mock.add_expected_object(obj.clone());
        let retrieved = mock.get(&obj.id).await.unwrap();
        assert_eq!(retrieved, Some(obj));
    }

    #[test]
    fn test_mock_store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockStore>();
    }
}
