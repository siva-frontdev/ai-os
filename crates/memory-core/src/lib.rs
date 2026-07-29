#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # memory-core
//!
//! Foundation crate for the AI-native OS Memory Platform.
//!
//! This crate defines all shared types, traits, errors, and events
//! that other memory crates (`memory-storage`, `memory-index`,
//! `memory-working`, etc.) depend on. No memory crate depends on
//! anything outside of `memory-core` and standard Rust libraries.
//!
//! ## Architecture
//!
//! - **Types**: [`MemoryId`], [`Timestamp`], [`MemorySource`], [`MemoryTier`],
//!   [`MemoryType`], [`MemoryPriority`], [`MemoryImportance`], [`Version`],
//!   [`Checksum`], [`Metadata`], [`RelationType`]
//! - **Records**: [`MemoryObject`], [`Relationship`]
//! - **Query**: [`QueryFilter`], [`SortField`], [`SortOrder`],
//!   [`StorageStats`], [`CapacityInfo`]
//! - **Errors**: [`MemoryError`], [`MemoryResult`]
//! - **Events**: [`MemoryEvent`]
//! - **Traits**: [`Validate`], [`Checksumable`], [`ToJsonBytes`]
//!
//! ## Validation
//!
//! The [`Validate`] trait is implemented for [`MemoryObject`], [`Relationship`],
//! and [`Checksum`] to enforce invariants:
//!
//! - Importance and relationship weight must be in [0.0, 1.0]
//! - Expiration must be after timestamp
//! - Content type must not be empty
//! - Timestamp must not be unreasonably far in the future

mod error;
mod event;
mod types;

/// The universal memory record with builder pattern.
pub mod memory_object;
/// Query filters, sorting, pagination, and storage statistics.
pub mod query;
/// Typed, weighted relationships between memory objects.
pub mod relationship;
/// Validation and serialization traits for memory objects.
pub mod traits;

/// World Model — generic entity/relationship types for the continuous cognitive loop.
pub mod wm;

// Re-export all public types
pub use types::{
    Checksum, MemoryId, MemoryImportance, MemoryPriority, MemorySource, MemoryTier, MemoryType,
    Metadata, RelationType, Timestamp, Version,
};

// Re-export error types
pub use error::{MemoryError, MemoryResult};

// Re-export event types
pub use event::MemoryEvent;

// Re-export query types
pub use query::{CapacityInfo, QueryFilter, SortField, SortOrder, StorageStats};

// Re-export record types
pub use memory_object::{MemoryObject, MemoryObjectBuilder};
pub use relationship::Relationship;

// Re-export traits
pub use traits::{Checksumable, ToJsonBytes, Validate};

// Re-export World Model types
pub use wm::{
    Entity, EntityId, EntityLifecycle, EntityObservation, ExtractionResult,
    Relationship as WmRelationship, RelationshipId, RelationshipObservation, Value,
};
