//! Cache statistics tracking — hit/miss/eviction counters and hit-rate computation.

use std::sync::atomic::{AtomicU64, Ordering};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::MemoryCacheResult;

/// Tracks cache statistics such as hits, misses, evictions, and hit rate.
#[async_trait]
pub trait CacheStatistics: Send + Sync + std::fmt::Debug {
    /// Record a cache hit for the given key with the observed latency.
    async fn record_hit(&self, key: &str, latency_us: u64) -> MemoryCacheResult<()>;

    /// Record a cache miss for the given key with the given reason.
    async fn record_miss(&self, key: &str, reason: &str) -> MemoryCacheResult<()>;

    /// Record an eviction for the given key with the given reason.
    async fn record_eviction(&self, key: &str, reason: &str) -> MemoryCacheResult<()>;

    /// Return the current hit rate (0.0 – 1.0).
    async fn hit_rate(&self) -> MemoryCacheResult<f64>;

    /// Return the total number of operations (hits + misses).
    async fn total_operations(&self) -> MemoryCacheResult<u64>;

    /// Reset all counters to zero.
    async fn reset(&self) -> MemoryCacheResult<()>;
}

/// A no-op statistics implementation that discards all events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultCacheStatistics;

#[async_trait]
impl CacheStatistics for DefaultCacheStatistics {
    async fn record_hit(&self, _key: &str, _latency_us: u64) -> MemoryCacheResult<()> {
        Ok(())
    }

    async fn record_miss(&self, _key: &str, _reason: &str) -> MemoryCacheResult<()> {
        Ok(())
    }

    async fn record_eviction(&self, _key: &str, _reason: &str) -> MemoryCacheResult<()> {
        Ok(())
    }

    async fn hit_rate(&self) -> MemoryCacheResult<f64> {
        Ok(0.0)
    }

    async fn total_operations(&self) -> MemoryCacheResult<u64> {
        Ok(0)
    }

    async fn reset(&self) -> MemoryCacheResult<()> {
        Ok(())
    }
}

/// An atomic statistics implementation backed by `AtomicU64` counters.
///
/// Thread-safe and suitable for concurrent cache access without external locking.
#[derive(Debug)]
pub struct AtomicCacheStatistics {
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl Default for AtomicCacheStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicCacheStatistics {
    /// Creates a new `AtomicCacheStatistics` with all counters at zero.
    pub fn new() -> Self {
        AtomicCacheStatistics {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    /// Returns the raw hit count.
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Returns the raw miss count.
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Returns the raw eviction count.
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl CacheStatistics for AtomicCacheStatistics {
    async fn record_hit(&self, _key: &str, _latency_us: u64) -> MemoryCacheResult<()> {
        self.hits.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn record_miss(&self, _key: &str, _reason: &str) -> MemoryCacheResult<()> {
        self.misses.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn record_eviction(&self, _key: &str, _reason: &str) -> MemoryCacheResult<()> {
        self.evictions.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn hit_rate(&self) -> MemoryCacheResult<f64> {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            Ok(0.0)
        } else {
            Ok(hits as f64 / total as f64)
        }
    }

    async fn total_operations(&self) -> MemoryCacheResult<u64> {
        Ok(self.hits.load(Ordering::Relaxed) + self.misses.load(Ordering::Relaxed))
    }

    async fn reset(&self) -> MemoryCacheResult<()> {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_hit_miss() {
        let stats = AtomicCacheStatistics::new();
        stats.record_hit("a", 10).await.unwrap();
        stats.record_hit("b", 20).await.unwrap();
        stats.record_miss("c", "not_found").await.unwrap();
        assert_eq!(stats.total_operations().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_hit_rate_computation() {
        let stats = AtomicCacheStatistics::new();
        assert_eq!(stats.hit_rate().await.unwrap(), 0.0);
        stats.record_hit("a", 5).await.unwrap();
        stats.record_hit("b", 5).await.unwrap();
        stats.record_miss("c", "expired").await.unwrap();
        let rate = stats.hit_rate().await.unwrap();
        assert!((rate - 2.0 / 3.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_reset() {
        let stats = AtomicCacheStatistics::new();
        stats.record_hit("a", 1).await.unwrap();
        stats.record_miss("b", "x").await.unwrap();
        stats.record_eviction("c", "full").await.unwrap();
        assert_eq!(stats.hits(), 1);
        assert_eq!(stats.misses(), 1);
        assert_eq!(stats.evictions(), 1);
        stats.reset().await.unwrap();
        assert_eq!(stats.hits(), 0);
        assert_eq!(stats.misses(), 0);
        assert_eq!(stats.evictions(), 0);
    }
}
