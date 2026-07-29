use async_trait::async_trait;
use execution_core::*;
use memory_core::Timestamp;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::debug;

#[derive(Debug)]
pub struct InMemoryCapabilityCache {
    entries: RwLock<HashMap<String, CapabilityCacheEntry>>,
    hits: RwLock<u64>,
    misses: RwLock<u64>,
}

impl InMemoryCapabilityCache {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            hits: RwLock::new(0),
            misses: RwLock::new(0),
        }
    }
}

impl Default for InMemoryCapabilityCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CapabilityCache for InMemoryCapabilityCache {
    async fn lookup(
        &self,
        capability_id: &str,
    ) -> execution_core::ExecutionResult<Option<CapabilityCacheEntry>> {
        let entries = self.entries.read().await;
        if let Some(mut entry) = entries.get(capability_id).cloned() {
            entry.last_accessed = Timestamp::now();
            entry.access_count += 1;
            drop(entries);

            let mut hits = self.hits.write().await;
            *hits += 1;

            let mut entries = self.entries.write().await;
            entries.insert(capability_id.to_string(), entry.clone());

            Ok(Some(entry))
        } else {
            drop(entries);
            let mut misses = self.misses.write().await;
            *misses += 1;
            Ok(None)
        }
    }

    async fn store(&self, entry: CapabilityCacheEntry) -> execution_core::ExecutionResult<()> {
        let mut entries = self.entries.write().await;
        entries.insert(entry.capability_id.clone(), entry);
        debug!(count = entries.len(), "capability cache entry stored");
        Ok(())
    }

    async fn invalidate(&self, capability_id: &str) -> execution_core::ExecutionResult<()> {
        let mut entries = self.entries.write().await;
        entries.remove(capability_id);
        debug!(capability = %capability_id, "capability cache invalidated");
        Ok(())
    }

    async fn list_cached(&self) -> execution_core::ExecutionResult<Vec<CapabilityCacheEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.values().cloned().collect())
    }

    async fn lookup_by_provider(
        &self,
        provider_name: &str,
    ) -> execution_core::ExecutionResult<Vec<CapabilityCacheEntry>> {
        let entries = self.entries.read().await;
        Ok(entries
            .values()
            .filter(|e| e.provider_metadata.name == provider_name)
            .cloned()
            .collect())
    }

    async fn hit_rate(&self) -> f64 {
        let hits = *self.hits.read().await;
        let misses = *self.misses.read().await;
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    async fn clear(&self) -> execution_core::ExecutionResult<()> {
        let mut entries = self.entries.write().await;
        entries.clear();
        let mut hits = self.hits.write().await;
        *hits = 0;
        let mut misses = self.misses.write().await;
        *misses = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(capability_id: &str) -> CapabilityCacheEntry {
        CapabilityCacheEntry {
            capability_id: capability_id.into(),
            origin: CapabilityOrigin::DiscoveredLocal,
            provider_metadata: ProviderMetadata::new("test", "cli", "/test"),
            trust: TrustScore::new(CapabilityOrigin::DiscoveredLocal),
            supported_parameters: HashMap::new(),
            required_permissions: Vec::new(),
            adapter: None,
            created_at: Timestamp::now(),
            last_accessed: Timestamp::now(),
            access_count: 0,
        }
    }

    #[tokio::test]
    async fn test_store_and_lookup() {
        let cache = InMemoryCapabilityCache::new();
        let entry = make_entry("test.cap");
        cache.store(entry.clone()).await.unwrap();
        let found = cache.lookup("test.cap").await.unwrap().unwrap();
        assert_eq!(found.capability_id, "test.cap");
    }

    #[tokio::test]
    async fn test_lookup_miss() {
        let cache = InMemoryCapabilityCache::new();
        let result = cache.lookup("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_invalidate() {
        let cache = InMemoryCapabilityCache::new();
        let entry = make_entry("test.cap");
        cache.store(entry).await.unwrap();
        cache.invalidate("test.cap").await.unwrap();
        let result = cache.lookup("test.cap").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_cached() {
        let cache = InMemoryCapabilityCache::new();
        cache.store(make_entry("cap1")).await.unwrap();
        cache.store(make_entry("cap2")).await.unwrap();
        let list = cache.list_cached().await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_lookup_updates_access_count() {
        let cache = InMemoryCapabilityCache::new();
        cache.store(make_entry("test.cap")).await.unwrap();
        let entry = cache.lookup("test.cap").await.unwrap().unwrap();
        assert_eq!(entry.access_count, 1);
        let entry = cache.lookup("test.cap").await.unwrap().unwrap();
        assert_eq!(entry.access_count, 2);
    }

    #[tokio::test]
    async fn test_hit_rate() {
        let cache = InMemoryCapabilityCache::new();
        assert_eq!(cache.hit_rate().await, 0.0);
        cache.store(make_entry("test")).await.unwrap();
        let _ = cache.lookup("test").await;
        let _ = cache.lookup("miss").await;
        let rate = cache.hit_rate().await;
        assert!((rate - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = InMemoryCapabilityCache::new();
        cache.store(make_entry("test")).await.unwrap();
        cache.clear().await.unwrap();
        assert!(cache.lookup("test").await.unwrap().is_none());
    }
}
