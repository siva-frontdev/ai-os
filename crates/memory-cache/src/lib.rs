#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! High-speed temporary memory with LRU eviction, TTL, priority eviction,
//! size limits, hit ratio tracking, and prefetch hooks. Sits in front of the
//! retrieval path to accelerate repeated access.

mod error;
mod event;
pub mod cache;
pub mod policy;
pub mod statistics;

pub use cache::{CacheStats, CachedEntry, DefaultMemoryCache, LruMemoryCache, MemoryCache};
pub use error::{MemoryCacheError, MemoryCacheResult};
pub use event::{CacheEvicted, CacheHit, CacheMiss};
pub use policy::{CachePolicy, DefaultCachePolicy};
pub use statistics::{CacheStatistics, DefaultCacheStatistics};
