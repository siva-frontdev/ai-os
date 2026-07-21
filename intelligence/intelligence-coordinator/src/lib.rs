#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Top-level orchestration wiring for the Intelligence Platform.
///
/// Composes all 13 intelligence crates via `Arc<dyn Trait>` and exposes a
/// single entry point that runs the documented 9-stage pipeline.
pub mod coordinator;
pub mod error;

pub use coordinator::DefaultCoordinator;
