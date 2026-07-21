#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Safety enforcement for the Intelligence Platform.
//!
//! Uses `regex` to scan inputs and outputs for PII and prompt-injection
//! patterns, returning a [`SafetyResult`] and emitting [`SafetyEvent`]s.

pub mod error;
pub mod safety;

pub use error::SafetyEnforcerError;
pub use safety::DefaultSafetyEnforcer;
