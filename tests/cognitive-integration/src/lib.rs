//! Cognitive Layer Integration — bridges the runtime manager to the brain's
//! cognitive loop so the full pipeline (observe → plan → execute → reflect)
//! runs against the **real MCP subprocess**.
//!
//! This crate implements the Phase 5 integration layer. It does NOT duplicate
//! or replace the brain's existing `CognitiveLoopService`, `Planner`, or
//! `ActionExecutor`. Instead it provides the missing glue:
//!
//! - `RuntimeAwareExecutor` — executes plan actions that target runtime
//!   capabilities (e.g. `email.send`, `telegram.inject_inbound`) by
//!   dispatching them through the `RuntimeManager` to the real MCP server.
//! - `RuntimeObservationSource` — wraps the `RuntimeManager` as an
//!   `ObservationSource` so the observation loop ingests runtime events.
//! - `CognitiveRuntimeBridge` — assembles the full pipeline with the real
//!   runtime wiring and exposes a single `initialize()` entry point.

#![forbid(unsafe_code)]

pub mod bridge;
pub mod error;
pub mod executor;
pub mod source;

pub use bridge::CognitiveRuntimeBridge;
pub use error::IntegrationError;
pub use executor::RuntimeAwareExecutor;
pub use source::RuntimeObservationSource;

use ai_os_runtime_api::Observation;
use brain_coordinator::planner::{Plan, PlannedAction};

/// Convert a runtime [`Observation`] into a text string for the cognitive loop.
pub fn observation_to_text(obs: &Observation) -> String {
    if let Some(ref sender) = obs.sender {
        format!("{sender}: {}", obs.payload["text"].as_str().unwrap_or(""))
    } else {
        serde_json::to_string_pretty(&obs.payload).unwrap_or_default()
    }
}

/// The set of planner action types that are handled in-memory by the
/// cognitive loop itself and therefore must NOT be dispatched to the runtime.
pub const IN_MEMORY_ACTIONS: &[&str] = &[
    "respond",
    "search_memory",
    "search_recent_conversation",
    "ask_clarification",
    "ignore",
    "observe",
    "schedule",
];

/// Returns true if the planner action is handled in-memory (not a runtime
/// capability).
pub fn is_in_memory_action(action_type: &str) -> bool {
    IN_MEMORY_ACTIONS.contains(&action_type)
}

/// Convert a planner [`PlannedAction`] into a runtime [`Action`], if the
/// action targets a runtime capability.
///
/// The planner produces flat `topics: Vec<String>` arrays, but runtime
/// capabilities have structured input schemas (e.g. `email.send` expects
/// `to`, `subject`, `body`). This function maps known capability types to
/// their proper input format, falling back to a generic `topics`+`reason`
/// envelope for unrecognized capabilities.
pub fn planned_to_runtime_action(action: &PlannedAction) -> Option<ai_os_runtime_api::Action> {
    if is_in_memory_action(&action.action_type) {
        return None;
    }

    let input = match action.action_type.as_str() {
        "email.send" => {
            let get = |i: usize| action.topics.get(i).cloned().unwrap_or_default();
            serde_json::json!({
                "to": get(0),
                "subject": get(1),
                "body": get(2),
            })
        }
        "telegram.send" => {
            let get = |i: usize| action.topics.get(i).cloned().unwrap_or_default();
            serde_json::json!({
                "chat_id": get(0),
                "text": get(1),
            })
        }
        "telegram.inject_inbound" => {
            let get = |i: usize| action.topics.get(i).cloned().unwrap_or_default();
            serde_json::json!({
                "chat_id": get(0),
                "sender": get(1),
                "text": get(2),
            })
        }
        "filesystem.write" => {
            let get = |i: usize| action.topics.get(i).cloned().unwrap_or_default();
            serde_json::json!({
                "path": get(0),
                "content": get(1),
            })
        }
        "filesystem.read" | "filesystem.search" => {
            let path = action.topics.first().cloned().unwrap_or_default();
            serde_json::json!({"path": path})
        }
        "calendar.create_event" => {
            let get = |i: usize| action.topics.get(i).cloned().unwrap_or_default();
            serde_json::json!({
                "date": get(0),
                "title": get(1),
                "description": get(2),
            })
        }
        "calendar.read" => {
            let date = action.topics.first().cloned().unwrap_or_default();
            serde_json::json!({"date": date})
        }
        "github.create_issue" => {
            let get = |i: usize| action.topics.get(i).cloned().unwrap_or_default();
            serde_json::json!({
                "title": get(0),
                "body": get(1),
            })
        }
        "github.read_repository" => {
            let owner = action.topics.get(0).cloned().unwrap_or_default();
            let repo = action.topics.get(1).cloned().unwrap_or_default();
            serde_json::json!({
                "owner": owner,
                "repo": repo,
            })
        }
        _ => {
            if action.topics.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::json!({
                    "topics": action.topics,
                    "reason": action.reason,
                })
            }
        }
    };

    Some(ai_os_runtime_api::Action {
        action_id: uuid::Uuid::new_v4().to_string(),
        capability: ai_os_runtime_api::CapabilityId::new(action.action_type.clone()),
        input,
        trace_id: None,
        deadline_ms: None,
        metadata: std::collections::HashMap::new(),
    })
}
