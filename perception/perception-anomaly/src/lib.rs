#![forbid(unsafe_code)]

//! # perception-anomaly
//!
//! Statistical anomaly detection for the Perception Platform.
//!
//! This crate implements the [`AnomalyDetector`] trait for scoring
//! observations against configurable statistical models (Z-score,
//! rolling window, Holt-Winters), and the [`AnomalyModel`] trait
//! for pluggable detection algorithms.

mod anomaly;

pub use anomaly::*;
