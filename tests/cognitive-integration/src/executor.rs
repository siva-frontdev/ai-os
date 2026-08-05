//! Executor that dispatches runtime-capability actions through the
//! [`RuntimeManager`] to the real MCP subprocess.

use std::sync::Arc;

use ai_os_runtime_api::{ActionResult, CapabilityId};
use ai_os_runtime_manager::RuntimeManager;
use brain_coordinator::planner::{Plan, PlannedAction};

use crate::{is_in_memory_action, planned_to_runtime_action};

/// Executes plan actions by dispatching runtime-capability actions through
/// the `RuntimeManager`.
///
/// In-memory actions (respond, search_memory, etc.) are handled by the brain's
/// `ActionExecutor` — this executor only handles capabilities that route to
/// a real runtime (email.send, telegram.inject_inbound, filesystem.read, etc.).
pub struct RuntimeAwareExecutor {
    manager: Arc<RuntimeManager>,
}

impl RuntimeAwareExecutor {
    pub fn new(manager: Arc<RuntimeManager>) -> Self {
        Self { manager }
    }

    /// Dispatch every plan action that targets a runtime capability.
    ///
    /// Returns a vector of `(capability, result)` pairs for each dispatched
    /// action. Actions that are handled in-memory are skipped (returned by
    /// the brain's own executor instead).
    pub async fn execute_plan(&self, plan: &Plan) -> Vec<(String, ActionResult)> {
        let mut results = Vec::new();

        for action in &plan.actions {
            if is_in_memory_action(&action.action_type) {
                continue;
            }

            if let Some(runtime_action) = planned_to_runtime_action(action) {
                if self
                    .manager
                    .has_capability(&runtime_action.capability)
                    .await
                {
                    match self.manager.dispatch(runtime_action).await {
                        Ok(result) => {
                            results.push((action.action_type.clone(), result));
                        }
                        Err(e) => {
                            tracing::warn!("dispatch failed for '{}': {}", action.action_type, e);
                            results.push((
                                action.action_type.clone(),
                                ActionResult::failed(
                                    "runtime-exec".to_string(),
                                    "dispatch_error",
                                    e.to_string(),
                                    false,
                                ),
                            ));
                        }
                    }
                } else {
                    tracing::warn!(
                        "capability '{}' not available on any runtime",
                        action.action_type
                    );
                    results.push((
                        action.action_type.clone(),
                        ActionResult::failed(
                            "runtime-exec".to_string(),
                            "capability_not_found",
                            format!("capability '{}' not registered", action.action_type),
                            false,
                        ),
                    ));
                }
            }
        }

        results
    }

    /// Dispatch a single action.
    pub async fn execute_action(&self, action: &PlannedAction) -> Result<ActionResult, String> {
        if is_in_memory_action(&action.action_type) {
            return Err(format!("action '{}' is in-memory", action.action_type));
        }

        let runtime_action = planned_to_runtime_action(action)
            .ok_or_else(|| format!("action '{}' produces no runtime action", action.action_type))?;

        if !self
            .manager
            .has_capability(&runtime_action.capability)
            .await
        {
            return Err(format!(
                "capability '{}' not available on any runtime",
                action.action_type
            ));
        }

        self.manager
            .dispatch(runtime_action)
            .await
            .map_err(|e| e.to_string())
    }

    /// Check if a capability is available on any registered runtime.
    pub async fn has_capability(&self, capability: &str) -> bool {
        self.manager
            .has_capability(&ai_os_runtime_api::CapabilityId::new(capability))
            .await
    }
}

impl std::fmt::Debug for RuntimeAwareExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeAwareExecutor")
            .field("manager", &"<RuntimeManager>")
            .finish()
    }
}
