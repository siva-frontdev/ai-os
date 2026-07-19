//! Caching policies that control what gets cached, for how long, and at what priority.

use std::time::Duration;
use async_trait::async_trait;
use memory_core::MemoryObject;
use serde::{Deserialize, Serialize};

use crate::error::MemoryCacheResult;

/// A caching policy that decides which objects to cache and for how long.
#[async_trait]
pub trait CachePolicy: Send + Sync + std::fmt::Debug {
    /// Returns `true` if the given object should be cached.
    async fn should_cache(&self, key: &str, object: &MemoryObject) -> MemoryCacheResult<bool>;

    /// Returns the TTL for the given object, or `None` for no expiration.
    async fn ttl_for(&self, key: &str, object: &MemoryObject) -> MemoryCacheResult<Option<Duration>>;

    /// Returns the priority for the given object (0 = lowest, 255 = highest).
    async fn priority_for(&self, key: &str, object: &MemoryObject) -> MemoryCacheResult<u8>;

    /// Returns the maximum number of entries and maximum bytes the cache may hold.
    async fn max_size(&self) -> MemoryCacheResult<(usize, u64)>;
}

/// A default caching policy that caches everything with a 60-second TTL,
/// priority 128, and limits of 512 entries / 64 MB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultCachePolicy;

#[async_trait]
impl CachePolicy for DefaultCachePolicy {
    async fn should_cache(&self, _key: &str, _object: &MemoryObject) -> MemoryCacheResult<bool> {
        Ok(true)
    }

    async fn ttl_for(&self, _key: &str, _object: &MemoryObject) -> MemoryCacheResult<Option<Duration>> {
        Ok(Some(Duration::from_secs(60)))
    }

    async fn priority_for(&self, _key: &str, _object: &MemoryObject) -> MemoryCacheResult<u8> {
        Ok(128)
    }

    async fn max_size(&self) -> MemoryCacheResult<(usize, u64)> {
        Ok((512, 64 * 1024 * 1024))
    }
}

/// A policy that never caches anything (TTL = None / infinite, but `should_cache` returns `false`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlwaysCachePolicy;

#[async_trait]
impl CachePolicy for AlwaysCachePolicy {
    async fn should_cache(&self, _key: &str, _object: &MemoryObject) -> MemoryCacheResult<bool> {
        Ok(true)
    }

    async fn ttl_for(&self, _key: &str, _object: &MemoryObject) -> MemoryCacheResult<Option<Duration>> {
        Ok(None)
    }

    async fn priority_for(&self, _key: &str, _object: &MemoryObject) -> MemoryCacheResult<u8> {
        Ok(255)
    }

    async fn max_size(&self) -> MemoryCacheResult<(usize, u64)> {
        Ok((1024, 256 * 1024 * 1024))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::MemoryObjectBuilder;

    fn make_obj() -> MemoryObject {
        MemoryObjectBuilder::new()
            .content_type("text/plain")
            .content(vec![1, 2, 3])
            .build()
    }

    #[tokio::test]
    async fn test_default_policy_caches_everything() {
        let policy = DefaultCachePolicy;
        assert!(policy.should_cache("key", &make_obj()).await.unwrap());
    }

    #[tokio::test]
    async fn test_default_policy_ttl() {
        let policy = DefaultCachePolicy;
        let ttl = policy.ttl_for("key", &make_obj()).await.unwrap();
        assert!(ttl.is_some());
        assert_eq!(ttl.unwrap().as_secs(), 60);
    }

    #[tokio::test]
    async fn test_default_policy_priority() {
        let policy = DefaultCachePolicy;
        assert_eq!(policy.priority_for("key", &make_obj()).await.unwrap(), 128);
    }

    #[tokio::test]
    async fn test_default_policy_max_size() {
        let policy = DefaultCachePolicy;
        let (entries, bytes) = policy.max_size().await.unwrap();
        assert_eq!(entries, 512);
        assert_eq!(bytes, 64 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_always_cache_policy() {
        let policy = AlwaysCachePolicy;
        assert!(policy.should_cache("k", &make_obj()).await.unwrap());
        assert!(policy.ttl_for("k", &make_obj()).await.unwrap().is_none());
        assert_eq!(policy.priority_for("k", &make_obj()).await.unwrap(), 255);
    }
}
