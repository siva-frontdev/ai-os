#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # memory-index
//!
//! Index trait definitions for the AI-native OS Memory Platform.
//!
//! This crate defines the four index traits that complement the
//! [`MemoryIndex`](https://docs.rs/memory-index/) trait (embedding-based vector search,
//! defined in a future stage) to form a complete search infrastructure:
//!
//! | Trait | Purpose |
//! |-------|---------|
//! | [`MetadataIndex`] | Search objects by metadata key-value pairs |
//! | [`TagIndex`] | Search objects by tags (AND/OR semantics) |
//! | [`RelationshipIndex`] | Graph adjacency queries (outgoing/incoming) |
//! | [`TimeIndex`] | Temporal range queries (oldest, newest, range) |
//!
//! Each trait has a `Default*` no-op implementation (`DefaultMetadataIndex`,
//! `DefaultTagIndex`, etc.) for use as placeholders. Production
//! implementations are provided by specialized crates.

mod metadata_index;
mod tag_index;
mod relationship_index;
mod time_index;

pub use metadata_index::{MetadataIndex, DefaultMetadataIndex};
pub use tag_index::{TagIndex, DefaultTagIndex};
pub use relationship_index::{RelationshipIndex, DefaultRelationshipIndex};
pub use time_index::{TimeIndex, DefaultTimeIndex};
