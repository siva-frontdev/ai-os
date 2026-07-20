#![forbid(unsafe_code)]

//! # perception-context
//!
//! Context enrichment and session resolution for the Perception Platform.
//!
//! This crate implements the [`ContextEnricher`] trait for attaching
//! temporal, spatial, and session context to observations, and the
//! [`SessionResolver`] trait for resolving OS-level identifiers to
//! session references.

mod context;

pub use context::*;
