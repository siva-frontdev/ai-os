//! Shared test harness for the runtime integration tests.
//!
//! Contains:
//! - a fake intelligence coordinator (so the real, unmodified `Planner`
//!   produces scripted plans)
//! - a `MockRuntime` (a second runtime implementation, proving swap)
//! - helpers to wire an `OpenClawRuntime` to an in-process mock MCP server
//! - the planner → manager dispatch harness used identically for both
//!   the OpenClaw runtime and the mock runtime

pub mod coordinator;
pub mod mock_runtime;
pub mod openclaw;

pub use coordinator::FakeCoordinator;
pub use mock_runtime::MockRuntime;
pub use openclaw::{email_plugin_handler, email_send_tool, openclaw_runtime_with_handler};

use ai_os_runtime_api::{Action, ActionResult, CapabilityId, RuntimeError};
use ai_os_runtime_manager::RuntimeManager;
use brain_coordinator::planner::{Plan, PlannedAction};

/// Translate a [`PlannedAction`] into a runtime [`Action`].
///
/// This is the execution-layer routing that lives *outside* cognition:
/// the Planner emits capabilities, the wiring layer turns them into
/// runtime actions. The `email.send` capability maps its `topics` list
/// onto structured `{ to, subject, body }` input.
pub fn action_from_planned(planned: &PlannedAction) -> Action {
    let input = match planned.action_type.as_str() {
        "email.send" => serde_json::json!({
            "to": planned.topics.first().cloned().unwrap_or_default(),
            "subject": planned.topics.get(1).cloned().unwrap_or_default(),
            "body": planned.topics.get(2).cloned().unwrap_or_default(),
        }),
        _ => serde_json::json!({
            "topics": planned.topics,
            "reason": planned.reason,
        }),
    };
    Action {
        action_id: uuid::Uuid::new_v4().to_string(),
        capability: CapabilityId::new(planned.action_type.clone()),
        input,
        trace_id: None,
        deadline_ms: None,
        metadata: Default::default(),
    }
}

/// Dispatch every runtime capability in a plan through the manager.
///
/// Brain capabilities (`respond`, `search_memory`, ...) are skipped — they
/// are handled by cognition, not by a runtime.
pub async fn dispatch_plan(
    manager: &RuntimeManager,
    plan: &Plan,
) -> Vec<Result<ActionResult, RuntimeError>> {
    let mut results = Vec::new();
    for planned in &plan.actions {
        let capability = CapabilityId::new(planned.action_type.clone());
        if !manager.has_capability(&capability).await {
            continue;
        }
        results.push(manager.dispatch(action_from_planned(planned)).await);
    }
    results
}
