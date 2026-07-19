//! Error types for the memory-cache crate.

use thiserror::Error;

/// Errors that can occur during cache operations.
#[derive(Error, Debug)]
pub enum MemoryCacheError {
    /// The requested cache key was not found.
    #[error("cache key not found: {0}")]
    EntryNotFound(String),

    /// The cache capacity limit was exceeded.
    #[error("capacity exceeded: current={0}, max={1}")]
    CapacityExceeded(u64, u64),

    /// The provided cache key is invalid.
    #[error("invalid cache key: {0}")]
    InvalidKey(String),

    /// An internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),
}

/// A result type alias for cache operations.
pub type MemoryCacheResult<T> = Result<T, MemoryCacheError>;
