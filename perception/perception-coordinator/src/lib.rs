//! perception-coordinator — Pipeline assembly, lifecycle, health, reconfiguration
//!
//! Assembles the perception pipeline from individual stage components,
//! manages lifecycle (start/stop/reload), monitors stage health, handles
//! backpressure, and bridges processed observations to the Brain Platform.

#![forbid(unsafe_code)]

mod coordinator;
pub use coordinator::*;
