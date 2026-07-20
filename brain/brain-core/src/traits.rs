//! Shared trait bounds and type aliases.

use crate::errors::BrainError;

/// Unified result type alias used throughout all Brain crates.
pub type BrainResult<T> = Result<T, BrainError>;

/// Trait bound shorthand for async Brain traits.
pub trait BrainSendSync: Send + Sync {}

impl<T: Send + Sync> BrainSendSync for T {}
