//! Event types emitted by the cache during its lifecycle.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Emitted when a cache lookup results in a hit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHit {
    /// The key that was looked up.
    pub key: String,
    /// Latency of the lookup in microseconds.
    pub latency_us: u64,
    /// The cache tier that served the hit.
    pub tier: u8,
}

impl CacheHit {
    /// Returns the event type identifier.
    pub fn event_type(&self) -> &'static str {
        "cache_hit"
    }

    /// Returns a map of metadata fields for this event.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("key".into(), self.key.clone());
        m.insert("latency_us".into(), self.latency_us.to_string());
        m.insert("tier".into(), self.tier.to_string());
        m
    }
}

/// Emitted when a cache lookup results in a miss.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMiss {
    /// The key that was looked up.
    pub key: String,
    /// The reason for the miss (e.g. "not_found", "expired").
    pub reason: String,
}

impl CacheMiss {
    /// Returns the event type identifier.
    pub fn event_type(&self) -> &'static str {
        "cache_miss"
    }

    /// Returns a map of metadata fields for this event.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("key".into(), self.key.clone());
        m.insert("reason".into(), self.reason.clone());
        m
    }
}

/// The reason why an entry was evicted from the cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionReason {
    /// The entry's TTL expired.
    TtlExpired,
    /// The cache was full and the entry was evicted to make room.
    CapacityFull,
    /// The entry was manually removed.
    ManualRemoval,
    /// The entry was evicted because a higher-priority entry needed space.
    PriorityEviction,
}

/// Emitted when an entry is evicted from the cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEvicted {
    /// The key of the evicted entry.
    pub key: String,
    /// The reason for eviction.
    pub reason: EvictionReason,
    /// The size of the evicted entry in bytes.
    pub size_bytes: u64,
}

impl CacheEvicted {
    /// Returns the event type identifier.
    pub fn event_type(&self) -> &'static str {
        "cache_evicted"
    }

    /// Returns a map of metadata fields for this event.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("key".into(), self.key.clone());
        m.insert("reason".into(), format!("{:?}", self.reason));
        m.insert("size_bytes".into(), self.size_bytes.to_string());
        m
    }
}
