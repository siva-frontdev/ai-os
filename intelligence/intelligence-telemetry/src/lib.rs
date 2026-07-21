#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Telemetry sink for the Intelligence Platform.
//!
//! Uses `tracing` for structured logging and an `mpsc`-backed batch writer
//! for telemetry records.

pub mod error;
pub mod sink;

pub use error::TelemetrySinkError;
pub use sink::DefaultTelemetrySink;
