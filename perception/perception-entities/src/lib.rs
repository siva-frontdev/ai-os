#![forbid(unsafe_code)]

//! # perception-entities
//!
//! Entity extraction and resolution for the Perception Platform.
//!
//! This crate implements the [`EntityExtractor`] trait for extracting
//! entity references from observation payloads using pattern matching,
//! and the [`EntityResolver`] trait for resolving raw identifiers
//! against the Memory Platform's knowledge graph.

mod entities;

pub use entities::*;
