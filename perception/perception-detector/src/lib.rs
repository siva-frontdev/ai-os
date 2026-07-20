#![forbid(unsafe_code)]

//! # perception-detector
//!
//! State detection, change detection, and temporal pattern
//! detection for the Perception Platform.
//!
//! This crate implements the [`StateDetector`] trait for tracking
//! per-scope state machines and the [`ChangeDetector`] trait for
//! continuous value change detection over time windows.

mod detector;

pub use detector::*;
