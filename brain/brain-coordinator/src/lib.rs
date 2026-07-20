#![forbid(unsafe_code)]

pub mod errors;
pub mod orchestrator;
pub mod types;

pub use errors::{CoordinatorError, CoordinatorResult};
pub use orchestrator::BrainOrchestrator;
pub use types::{BrainSession, BrainState, CognitiveLoad, CoordinationReport};

#[cfg(test)]
mod tests;
