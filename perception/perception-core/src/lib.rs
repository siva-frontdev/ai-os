#![forbid(unsafe_code)]

//! # perception-core
//!
//! Foundation crate for the Perception Platform (Phase 7).
//!
//! Defines all shared types, traits, errors, and events
//! that other perception crates depend on.
//!
//! ## Architecture
//!
//! - **Types**: [`ObservationId`], [`ObservationSource`], [`ObservationPayload`],
//!   [`ObservationPriority`], [`Modality`], [`ObserverKind`], [`Confidence`],
//!   [`Provenance`], [`TransformationStep`], [`QualityScore`],
//!   [`CorrelationId`], [`CorrelationKey`], [`ScopeId`],
//!   [`ObservationFilter`], [`Observation`],
//!   [`DesktopState`], [`WindowInfo`]
//! - **Errors**: [`PerceptionError`], [`PerceptionResult`]
//! - **Events**: [`PerceptionEvent`] enum with 19 variants
//! - **Traits**: [`ObservationProvider`], [`ObservationStore`], [`AttentionFilter`],
//!   [`DesktopObservationProvider`]

pub mod error;
pub mod event;
pub mod traits;
pub mod types;

pub use error::{PerceptionError, PerceptionResult};
pub use event::*;
pub use traits::*;
pub use types::*;
