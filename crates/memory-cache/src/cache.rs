//! Core cache types and the primary `MemoryCache` trait with LRU and no-op
//! implementations.

use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;
use std::time::Duration;

use async_trait::async_trait;
use memory_core::{MemoryObject, Timestamp};
use serde::{Deserialize, Serialize};

use crate::error::{MemoryCacheError, MemoryCacheResult};

/// A cache key, typically a string identifier for a memory object.
pub type CacheKey = String;

/// An entry stored in the cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    /// The cache key for this entry.
    pub key: CacheKey,
    /// The underlying memory object.
    pub object: MemoryObject,
    /// When this entry was placed in the cache.
    pub cached_at: Timestamp,
    /// When this entry expires, if ever.
    pub expires_at: Option<Timestamp>,
    /// How many times this entry has been accessed.
    pub access_count: u64,
    /// Eviction priority (0 = lowest, 255 = highest).
    pub priority: u8,
    /// The size of this entry in bytes.
    pub size_bytes: u64,
}

impl CachedEntry {
    fn new(key: CacheKey, object: MemoryObject, ttl: Option<Duration>, priority: u8) -> Self {
        let content_bytes = object.content.len() as u64;
        let expires_at = ttl.map(|d| {
            let now_ns = Timestamp::now().as_nanos();
            let ttl_ns = d.as_nanos().min(i64::MAX as u128) as i64;
            Timestamp::from_nanos(now_ns.saturating_add(ttl_ns))
        });

        Self {
            key,
            object,
            cached_at: Timestamp::now(),
            expires_at,
            access_count: 0,
            priority,
            size_bytes: content_bytes,
        }
    }

    fn is_expired(&self, now: Timestamp) -> bool {
        self.expires_at.map(|e| now > e).unwrap_or(false)
    }
}

/// A snapshot of cache statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Number of entries currently in the cache.
    pub total_entries: usize,
    /// Maximum number of entries allowed.
    pub max_entries: usize,
    /// Total number of cache hits.
    pub hits: u64,
    /// Total number of cache misses.
    pub misses: u64,
    /// Total number of evictions.
    pub evictions: u64,
    /// Hit rate (hits / (hits + misses)), 0.0 – 1.0.
    pub hit_rate: f64,
    /// Total bytes currently consumed by cached entries.
    pub total_bytes: u64,
    /// Maximum bytes allowed.
    pub max_bytes: u64,
}

/// The primary cache trait. All implementations must be `Send + Sync`.
#[async_trait]
pub trait MemoryCache: Send + Sync + std::fmt::Debug {
    /// Retrieve an entry by key. Returns `None` on a miss or if the entry has
    /// expired. On a hit the entry's access count is incremented.
    async fn get(&self, key: &CacheKey) -> MemoryCacheResult<Option<CachedEntry>>;

    /// Insert an entry with an optional TTL. If the key already exists it is
    /// overwritten. The cache may evict other entries to stay within capacity
    /// limits.
    async fn set(
        &self,
        key: CacheKey,
        object: MemoryObject,
        ttl: Option<Duration>,
    ) -> MemoryCacheResult<()>;

    /// Insert an entry with a specific eviction priority (0 = lowest,
    /// 255 = highest) alongside an optional TTL.
    async fn set_with_priority(
        &self,
        key: CacheKey,
        object: MemoryObject,
        ttl: Option<Duration>,
        priority: u8,
    ) -> MemoryCacheResult<()>;

    /// Remove a single entry by key.
    async fn remove(&self, key: &CacheKey) -> MemoryCacheResult<()>;

    /// Remove all entries whose key contains the given substring pattern.
    async fn invalidate(&self, pattern: &str) -> MemoryCacheResult<()>;

    /// Remove every entry from the cache and reset all counters.
    async fn clear(&self) -> MemoryCacheResult<()>;

    /// Return a `CacheStats` snapshot.
    async fn stats(&self) -> MemoryCacheResult<CacheStats>;

    /// Check whether a key exists in the cache and has not expired.
    async fn contains(&self, key: &CacheKey) -> MemoryCacheResult<bool>;
}

// ---------------------------------------------------------------------------
// LruMemoryCache
// ---------------------------------------------------------------------------

/// Internal state protected by a single `RwLock`.
#[derive(Debug)]
struct LruInner {
    map: HashMap<CacheKey, CachedEntry>,
    lru: VecDeque<CacheKey>,
    hits: u64,
    misses: u64,
    evictions: u64,
    total_bytes: u64,
}

/// A cache that evicts the least-recently-used entry (among the lowest-priority
/// entries) when capacity limits are exceeded.
///
/// Thread-safe via `std::sync::RwLock`. Default limits: 512 entries / 64 MB.
#[derive(Debug)]
pub struct LruMemoryCache {
    inner: RwLock<LruInner>,
    max_entries: usize,
    max_bytes: u64,
}

impl LruMemoryCache {
    /// Creates a new cache with the given maximum entries and maximum bytes.
    pub fn new(max_entries: usize, max_bytes: u64) -> Self {
        LruMemoryCache {
            inner: RwLock::new(LruInner {
                map: HashMap::new(),
                lru: VecDeque::new(),
                hits: 0,
                misses: 0,
                evictions: 0,
                total_bytes: 0,
            }),
            max_entries,
            max_bytes,
        }
    }
}

impl Default for LruMemoryCache {
    fn default() -> Self {
        Self::new(512, 64 * 1024 * 1024)
    }
}

/// Acquire a write lock, converting poison errors.
fn lock_inner(
    lock: &RwLock<LruInner>,
) -> MemoryCacheResult<std::sync::RwLockWriteGuard<'_, LruInner>> {
    lock.write().map_err(|e| MemoryCacheError::Internal(format!("lock poisoned: {e}")))
}

/// Remove the entry with the lowest priority (oldest among equal-priority ties).
fn evict_lowest_priority(
    map: &mut HashMap<CacheKey, CachedEntry>,
    lru: &mut VecDeque<CacheKey>,
) -> Option<u64> {
    // Clone candidate keys to avoid holding an immutable borrow of `lru`
    // while mutably borrowing `map`.
    let candidates: Vec<CacheKey> = lru.iter().cloned().collect();

    let mut target_key: Option<CacheKey> = None;
    let mut target_priority = u8::MAX;

    for key in candidates {
        if let Some(entry) = map.get(&key) {
            if entry.priority < target_priority {
                target_priority = entry.priority;
                target_key = Some(key);
            }
        }
    }

    target_key.and_then(|key| {
        let size = map.remove(&key).map(|e| e.size_bytes);
        lru.retain(|k| *k != key);
        size
    })
}

/// Evict entries until both capacity limits are satisfied.
fn ensure_capacity(inner: &mut LruInner, max_entries: usize, max_bytes: u64) {
    while inner.map.len() > max_entries || inner.total_bytes > max_bytes {
        if let Some(size) = evict_lowest_priority(&mut inner.map, &mut inner.lru) {
            inner.total_bytes = inner.total_bytes.saturating_sub(size);
            inner.evictions += 1;
        } else {
            break;
        }
    }
}

#[async_trait]
impl MemoryCache for LruMemoryCache {
    async fn get(&self, key: &CacheKey) -> MemoryCacheResult<Option<CachedEntry>> {
        let mut inner = lock_inner(&self.inner)?;

        // Clone while holding an immutable borrow to avoid double-borrowing
        // `inner` below (both `map` and `lru` are fields of the same struct).
        let hit: Option<CachedEntry> = inner.map.get(key).cloned();

        match hit {
            None => {
                inner.misses += 1;
                Ok(None)
            }
            Some(mut entry) => {
                if entry.is_expired(Timestamp::now()) {
                    let removed = inner.map.remove(key).expect("entry just checked");
                    inner.total_bytes = inner.total_bytes.saturating_sub(removed.size_bytes);
                    inner.lru.retain(|k| k != key);
                    inner.evictions += 1;
                    inner.misses += 1;
                    Ok(None)
                } else {
                    // Refresh LRU position
                    inner.lru.retain(|k| k != key);
                    inner.lru.push_back(key.clone());

                    entry.access_count += 1;
                    inner.map.insert(key.clone(), entry.clone());

                    inner.hits += 1;
                    Ok(Some(entry))
                }
            }
        }
    }

    async fn set(
        &self,
        key: CacheKey,
        object: MemoryObject,
        ttl: Option<Duration>,
    ) -> MemoryCacheResult<()> {
        self.set_with_priority(key, object, ttl, 128).await
    }

    async fn set_with_priority(
        &self,
        key: CacheKey,
        object: MemoryObject,
        ttl: Option<Duration>,
        priority: u8,
    ) -> MemoryCacheResult<()> {
        let mut inner = lock_inner(&self.inner)?;

        // Remove existing entry with the same key
        if let Some(old) = inner.map.remove(&key) {
            inner.total_bytes = inner.total_bytes.saturating_sub(old.size_bytes);
            inner.lru.retain(|k| *k != key);
        }

        let entry = CachedEntry::new(key.clone(), object, ttl, priority);
        inner.total_bytes = inner.total_bytes.saturating_add(entry.size_bytes);
        inner.map.insert(key.clone(), entry);
        inner.lru.push_back(key);

        ensure_capacity(&mut inner, self.max_entries, self.max_bytes);
        Ok(())
    }

    async fn remove(&self, key: &CacheKey) -> MemoryCacheResult<()> {
        let mut inner = lock_inner(&self.inner)?;

        if let Some(entry) = inner.map.remove(key) {
            inner.total_bytes = inner.total_bytes.saturating_sub(entry.size_bytes);
            inner.lru.retain(|k| k != key);
            inner.evictions += 1;
            Ok(())
        } else {
            Err(MemoryCacheError::EntryNotFound(key.clone()))
        }
    }

    async fn invalidate(&self, pattern: &str) -> MemoryCacheResult<()> {
        let mut inner = lock_inner(&self.inner)?;

        let keys: Vec<CacheKey> = inner
            .lru
            .iter()
            .filter(|k| k.contains(pattern))
            .cloned()
            .collect();

        for key in &keys {
            if let Some(entry) = inner.map.remove(key) {
                inner.total_bytes = inner.total_bytes.saturating_sub(entry.size_bytes);
                inner.evictions += 1;
            }
        }
        inner.lru.retain(|k| !keys.contains(k));

        Ok(())
    }

    async fn clear(&self) -> MemoryCacheResult<()> {
        let mut inner = lock_inner(&self.inner)?;
        inner.map.clear();
        inner.lru.clear();
        inner.hits = 0;
        inner.misses = 0;
        inner.evictions = 0;
        inner.total_bytes = 0;
        Ok(())
    }

    async fn stats(&self) -> MemoryCacheResult<CacheStats> {
        let inner = self
            .inner
            .read()
            .map_err(|e| MemoryCacheError::Internal(format!("lock poisoned: {e}")))?;

        let total_ops = inner.hits + inner.misses;
        let hit_rate = if total_ops > 0 {
            inner.hits as f64 / total_ops as f64
        } else {
            0.0
        };

        Ok(CacheStats {
            total_entries: inner.map.len(),
            max_entries: self.max_entries,
            hits: inner.hits,
            misses: inner.misses,
            evictions: inner.evictions,
            hit_rate,
            total_bytes: inner.total_bytes,
            max_bytes: self.max_bytes,
        })
    }

    async fn contains(&self, key: &CacheKey) -> MemoryCacheResult<bool> {
        let inner = self
            .inner
            .read()
            .map_err(|e| MemoryCacheError::Internal(format!("lock poisoned: {e}")))?;

        match inner.map.get(key) {
            Some(entry) if !entry.is_expired(Timestamp::now()) => Ok(true),
            _ => Ok(false),
        }
    }
}

// ---------------------------------------------------------------------------
// DefaultMemoryCache (no-op)
// ---------------------------------------------------------------------------

/// A no-op cache implementation that discards all entries and always reports a
/// miss.
#[derive(Debug)]
pub struct DefaultMemoryCache;

#[async_trait]
impl MemoryCache for DefaultMemoryCache {
    async fn get(&self, _key: &CacheKey) -> MemoryCacheResult<Option<CachedEntry>> {
        Ok(None)
    }

    async fn set(
        &self,
        _key: CacheKey,
        _object: MemoryObject,
        _ttl: Option<Duration>,
    ) -> MemoryCacheResult<()> {
        Ok(())
    }

    async fn set_with_priority(
        &self,
        _key: CacheKey,
        _object: MemoryObject,
        _ttl: Option<Duration>,
        _priority: u8,
    ) -> MemoryCacheResult<()> {
        Ok(())
    }

    async fn remove(&self, _key: &CacheKey) -> MemoryCacheResult<()> {
        Ok(())
    }

    async fn invalidate(&self, _pattern: &str) -> MemoryCacheResult<()> {
        Ok(())
    }

    async fn clear(&self) -> MemoryCacheResult<()> {
        Ok(())
    }

    async fn stats(&self) -> MemoryCacheResult<CacheStats> {
        Ok(CacheStats {
            total_entries: 0,
            max_entries: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            hit_rate: 0.0,
            total_bytes: 0,
            max_bytes: 0,
        })
    }

    async fn contains(&self, _key: &CacheKey) -> MemoryCacheResult<bool> {
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_obj(content: Vec<u8>) -> MemoryObject {
        MemoryObject::builder()
            .content_type("text/plain")
            .content(content)
            .build()
    }

    #[tokio::test]
    async fn test_get_set() {
        let cache = LruMemoryCache::default();
        let obj = make_obj(vec![42; 100]);
        cache.set("key1".into(), obj.clone(), None).await.unwrap();

        let got = cache.get(&"key1".into()).await.unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap().object.content, vec![42; 100]);
    }

    #[tokio::test]
    async fn test_get_miss() {
        let cache = LruMemoryCache::default();
        let got = cache.get(&"nonexistent".into()).await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let cache = LruMemoryCache::default();
        let obj = make_obj(vec![1; 10]);
        cache
            .set("x".into(), obj, Some(Duration::from_millis(5)))
            .await
            .unwrap();

        assert!(cache.get(&"x".into()).await.unwrap().is_some());

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(cache.get(&"x".into()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let cache = LruMemoryCache::new(3, 10_000);
        for i in 0..3 {
            let obj = make_obj(vec![0u8; 10]);
            cache.set(format!("key{i}"), obj, None).await.unwrap();
        }

        // Access key0 to promote it to the back
        cache.get(&"key0".into()).await.unwrap();

        let obj = make_obj(vec![0u8; 10]);
        cache.set("key3".into(), obj, None).await.unwrap();

        assert!(cache.get(&"key0".into()).await.unwrap().is_some());
        assert!(cache.get(&"key1".into()).await.unwrap().is_none());
        assert!(cache.get(&"key2".into()).await.unwrap().is_some());
        assert!(cache.get(&"key3".into()).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_priority_eviction() {
        let cache = LruMemoryCache::new(3, 10_000);

        let o0 = make_obj(vec![0; 10]);
        cache.set_with_priority("key0".into(), o0, None, 100).await.unwrap();

        let o1 = make_obj(vec![0; 10]);
        cache.set_with_priority("key1".into(), o1, None, 50).await.unwrap();

        let o2 = make_obj(vec![0; 10]);
        cache.set_with_priority("key2".into(), o2, None, 200).await.unwrap();

        let o3 = make_obj(vec![0; 10]);
        cache.set_with_priority("key3".into(), o3, None, 150).await.unwrap();

        // key1 (priority 50) should be evicted as the lowest
        assert!(
            cache.get(&"key1".into()).await.unwrap().is_none(),
            "lowest-priority entry should have been evicted"
        );
        assert!(cache.get(&"key0".into()).await.unwrap().is_some());
        assert!(cache.get(&"key2".into()).await.unwrap().is_some());
        assert!(cache.get(&"key3".into()).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_size_limit_eviction() {
        let cache = LruMemoryCache::new(100, 500);

        let o1 = make_obj(vec![0u8; 300]);
        cache.set("a".into(), o1, None).await.unwrap();

        let o2 = make_obj(vec![0u8; 300]);
        cache.set("b".into(), o2, None).await.unwrap();

        assert!(cache.get(&"a".into()).await.unwrap().is_none());
        assert!(cache.get(&"b".into()).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = LruMemoryCache::default();
        let obj = make_obj(vec![1; 10]);
        cache.set("r".into(), obj, None).await.unwrap();
        assert!(cache.contains(&"r".into()).await.unwrap());

        cache.remove(&"r".into()).await.unwrap();
        assert!(!cache.contains(&"r".into()).await.unwrap());

        let err = cache.remove(&"r".into()).await.unwrap_err();
        assert!(matches!(err, MemoryCacheError::EntryNotFound(_)));
    }

    #[tokio::test]
    async fn test_invalidate_pattern() {
        let cache = LruMemoryCache::new(100, 10_000);
        for i in 0..5 {
            let obj = make_obj(vec![0; 10]);
            cache.set(format!("user:{i}:profile"), obj, None).await.unwrap();
        }
        let obj = make_obj(vec![0; 10]);
        cache.set("admin:config".into(), obj, None).await.unwrap();

        cache.invalidate("user:").await.unwrap();

        assert!(cache.contains(&"admin:config".into()).await.unwrap());
        for i in 0..5 {
            assert!(!cache.contains(&format!("user:{i}:profile")).await.unwrap());
        }
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = LruMemoryCache::default();
        let obj = make_obj(vec![1; 10]);
        cache.set("k".into(), obj, None).await.unwrap();
        assert!(cache.contains(&"k".into()).await.unwrap());

        cache.clear().await.unwrap();
        assert!(!cache.contains(&"k".into()).await.unwrap());

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.total_entries, 0);
    }

    #[tokio::test]
    async fn test_stats_hit_rate() {
        let cache = LruMemoryCache::default();
        let obj = make_obj(vec![1; 10]);
        cache.set("k".into(), obj, None).await.unwrap();

        let _ = cache.get(&"missing".into()).await;
        let _ = cache.get(&"k".into()).await;

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 0.5).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_contains() {
        let cache = LruMemoryCache::default();
        let obj = make_obj(vec![1; 10]);
        cache.set("present".into(), obj, None).await.unwrap();

        assert!(cache.contains(&"present".into()).await.unwrap());
        assert!(!cache.contains(&"absent".into()).await.unwrap());
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let cache = Arc::new(LruMemoryCache::new(1000, 100_000));
        let mut handles = Vec::new();

        for i in 0..20 {
            let c = cache.clone();
            handles.push(tokio::spawn(async move {
                let obj = make_obj(vec![0u8; 100]);
                c.set(format!("key{i}"), obj, None).await.unwrap();
                let _ = c.get(&format!("key{i}")).await;
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let stats = cache.stats().await.unwrap();
        assert!(stats.hits + stats.misses > 0);
    }

    #[tokio::test]
    async fn test_renew_ttl() {
        let cache = LruMemoryCache::default();
        let obj = make_obj(vec![1; 10]);
        cache
            .set("k".into(), obj, Some(Duration::from_millis(5)))
            .await
            .unwrap();

        let obj2 = make_obj(vec![2; 10]);
        cache
            .set("k".into(), obj2, Some(Duration::from_secs(60)))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(cache.contains(&"k".into()).await.unwrap());
    }

    #[test]
    fn test_lru_cache_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LruMemoryCache>();
        assert_send_sync::<DefaultMemoryCache>();
    }
}
