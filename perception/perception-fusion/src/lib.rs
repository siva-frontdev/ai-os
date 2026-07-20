//! perception-fusion — Observation correlation and synthesis
//!
//! Correlates related observations from multiple observers or modalities within
//! configurable time windows. When all expected observations for a correlation
//! key arrive, synthesizes a single fused observation with consolidated
//! confidence and merged provenance.

#![forbid(unsafe_code)]

mod fusion;
pub use fusion::*;
