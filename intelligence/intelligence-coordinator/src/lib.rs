#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Top-level orchestration wiring for the Intelligence Platform.
///
/// Composes all 13 intelligence crates via `Arc<dyn Trait>` and exposes a
/// single entry point that runs the documented 9-stage pipeline.
pub mod coordinator;
pub mod error;
pub mod reasoning;

/// AI-driven World Understanding — replaces heuristic extraction.
pub mod world_understanding;

pub use coordinator::DefaultCoordinator;
pub use reasoning::IntelligenceReasoningService;
pub use world_understanding::{StructuredWorldUpdate, WorldUnderstandingService};
