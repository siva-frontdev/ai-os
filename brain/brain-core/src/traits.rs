//! Shared trait bounds and type aliases.

use crate::errors::BrainError;

/// Unified result type alias used throughout all Brain crates.
pub type BrainResult<T> = Result<T, BrainError>;

/// Trait bound shorthand for async Brain traits.
pub trait BrainSendSync: Send + Sync {}

impl<T: Send + Sync> BrainSendSync for T {}

/// Service that provides LLM-based reasoning from a user request.
///
/// Implemented by the Intelligence Platform (Layer 8) and consumed by
/// the Brain Platform (Layer 6) during the cognitive loop.
#[async_trait::async_trait]
pub trait ReasoningService: Send + Sync + std::fmt::Debug {
    /// Accept raw user input and return structured semantic reasoning
    /// about what the user wants to accomplish.
    async fn reason(&self, input: &str) -> BrainResult<crate::types::ReasoningResult>;
}
