//! Cognitive Layer Integration — bridges the runtime manager to the brain's
//! cognitive loop so the full pipeline (observe → plan → execute → reflect)
//! runs against the **real MCP subprocess**.
//!
//! This crate implements the Phase 5 integration layer. It does NOT duplicate
//! or replace the brain's existing `CognitiveLoopService`, `Planner`, or
//! `ActionExecutor`. The runtime glue now lives in the brain itself
//! (`brain_coordinator::planner::runtime_executor`); this crate re-exports it
//! and adds the remaining integration pieces:
//!
//! - `RuntimeObservationSource` — wraps the `RuntimeManager` as an
//!   `ObservationSource` so the observation loop ingests runtime events.
//! - `CognitiveRuntimeBridge` — assembles the full pipeline with the real
//!   runtime wiring and exposes a single `initialize()` entry point.

#![forbid(unsafe_code)]

pub mod bridge;
pub mod error;
pub mod source;

pub use ai_os_runtime_manager::RuntimeManager;
pub use brain_coordinator::planner::runtime_executor::{
    is_in_memory_action, planned_to_runtime_action, RuntimeAwareExecutor, IN_MEMORY_ACTIONS,
};

pub use bridge::CognitiveRuntimeBridge;
pub use error::IntegrationError;
pub use source::RuntimeObservationSource;

use ai_os_runtime_api::Observation;

/// Convert a runtime [`Observation`] into a text string for the cognitive loop.
pub fn observation_to_text(obs: &Observation) -> String {
    if let Some(ref sender) = obs.sender {
        format!("{sender}: {}", obs.payload["text"].as_str().unwrap_or(""))
    } else {
        serde_json::to_string_pretty(&obs.payload).unwrap_or_default()
    }
}
