//! Runtime-capability execution for the cognitive loop.
//!
//! The Planner produces a [`Plan`] of [`PlannedAction`]s. Actions that map
//! to an in-memory capability are executed by the brain's `ActionExecutor`.
//! Actions that target a runtime capability (`email.send`,
//! `telegram.inject_inbound`, ...) are dispatched through the
//! [`RuntimeManager`] here, and their results are fed back as ground truth
//! into the final `respond` step so the model can never claim success for an
//! action that was not actually performed.

use std::sync::Arc;

use ai_os_runtime_api::{ActionResult, CapabilityId};
use ai_os_runtime_manager::RuntimeManager;

use super::{Plan, PlannedAction};

/// Planner action types handled in-memory by the cognitive loop itself.
/// These are never dispatched to the runtime.
pub const IN_MEMORY_ACTIONS: &[&str] = &[
    "respond",
    "search_memory",
    "search_recent_conversation",
    "ask_clarification",
    "ignore",
    "observe",
    "schedule",
];

/// Whether a planner action type is handled in-memory.
pub fn is_in_memory_action(action_type: &str) -> bool {
    IN_MEMORY_ACTIONS.contains(&action_type)
}

/// Convert a planner action into a runtime [`ai_os_runtime_api::Action`], if
/// the action targets a runtime capability.
///
/// The planner produces flat `topics: Vec<String>` arrays, but runtime
/// capabilities have structured input schemas. This maps known capability
/// types to their proper input format, falling back to a generic
/// `topics` + `reason` envelope for unrecognized capabilities.
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
        "whatsapp.send" => {
            let get = |i: usize| action.topics.get(i).cloned().unwrap_or_default();
            serde_json::json!({
                "to": get(0),
                "text": get(1),
            })
        }
        "whatsapp.receive" => serde_json::json!({}),
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
            let owner = action.topics.first().cloned().unwrap_or_default();
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
        capability: CapabilityId::new(action.action_type.clone()),
        input,
        trace_id: None,
        deadline_ms: None,
        metadata: std::collections::HashMap::new(),
    })
}

/// Executes plan actions by dispatching runtime-capability actions through
/// the [`RuntimeManager`].
///
/// In-memory actions (`respond`, `search_memory`, ...) are handled by the
/// brain's `ActionExecutor` — this executor only handles capabilities that
/// route to a real runtime (`email.send`, `telegram.inject_inbound`, ...).
///
/// Dispatch results are both returned to the caller (for grounding the
/// response) and, when a completion sink is attached, pushed back as
/// observations so the cognitive loop can proactively inform the user
/// ("Done. Email sent.") without the user asking.
pub struct RuntimeAwareExecutor {
    manager: Arc<RuntimeManager>,
    completion: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<String>>>>,
}

impl RuntimeAwareExecutor {
    pub fn new(manager: Arc<RuntimeManager>) -> Self {
        Self {
            manager,
            completion: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Attach a completion sink. Every dispatched `ActionResult` is
    /// formatted as a short observation and sent through this channel,
    /// which the companion's cognitive worker drains into `cycle()`.
    pub fn with_completion_sink(self, tx: tokio::sync::mpsc::UnboundedSender<String>) -> Self {
        *self.completion.blocking_lock() = Some(tx);
        self
    }

    /// Dispatch every plan action that targets a runtime capability.
    ///
    /// Returns a vector of `(capability, result)` pairs for each dispatched
    /// action. Actions that are handled in-memory are skipped (handled by the
    /// brain's own executor instead).
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

        // Runtime completion events become observations: push each result
        // through the completion sink (if attached) so the cognitive loop
        // learns the action finished and can proactively inform the user.
        if !results.is_empty() {
            if let Ok(completion) = self.completion.try_lock() {
                if let Some(tx) = completion.as_ref() {
                    for (action_type, result) in &results {
                        let observation = format_runtime_completion(action_type, result);
                        if tx.send(observation).is_err() {
                            tracing::debug!(
                                "completion channel closed while reporting runtime results"
                            );
                        }
                    }
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
            .has_capability(&CapabilityId::new(capability))
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

/// Build a natural-language grounding block from runtime dispatch results.
///
/// This is fed into the `respond` prompt so the model only ever claims an
/// external action happened if the runtime confirmed it. An empty slice
/// produces an empty block (no external actions were attempted).
pub fn format_runtime_results(results: &[(String, ActionResult)]) -> String {
    if results.is_empty() {
        return String::new();
    }

    let mut lines: Vec<String> = Vec::new();
    for (action_type, result) in results {
        let status = if result.is_success() {
            "SUCCEEDED"
        } else {
            "FAILED"
        };
        let detail = result
            .error
            .as_ref()
            .map(|e| format!(" (code: {}, {})", e.code, e.message))
            .unwrap_or_default();
        lines.push(format!("- {action_type}: {status}{detail}"));
    }
    lines.join("\n")
}

/// Format a single runtime dispatch result as a short observation for the
/// cognitive loop. The loop sees these as "runtime completed" events and can
/// proactively inform the user without a new request.
fn format_runtime_completion(action_type: &str, result: &ActionResult) -> String {
    let status = if result.is_success() {
        "succeeded"
    } else {
        "failed"
    };
    let detail = result
        .error
        .as_ref()
        .map(|e| format!(" ({})", e.message))
        .unwrap_or_default();
    format!("runtime '{action_type}' {status}{detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_with(actions: Vec<(&str, Vec<String>)>) -> Plan {
        Plan {
            actions: actions
                .into_iter()
                .map(|(ty, topics)| PlannedAction {
                    action_type: ty.into(),
                    topics,
                    reason: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn test_in_memory_actions_are_recognized() {
        for ty in IN_MEMORY_ACTIONS {
            assert!(is_in_memory_action(ty), "{ty} should be in-memory");
        }
        assert!(!is_in_memory_action("email.send"));
        assert!(!is_in_memory_action("telegram.inject_inbound"));
    }

    #[test]
    fn test_planned_to_runtime_action_maps_email_topics() {
        let action = PlannedAction {
            action_type: "email.send".into(),
            topics: vec!["a@b.c".into(), "subject".into(), "body".into()],
            reason: "user requested".into(),
        };
        let runtime_action = planned_to_runtime_action(&action).unwrap();
        assert_eq!(runtime_action.capability.as_str(), "email.send");
        assert_eq!(runtime_action.input["to"], "a@b.c");
        assert_eq!(runtime_action.input["subject"], "subject");
        assert_eq!(runtime_action.input["body"], "body");
    }

    #[test]
    fn test_in_memory_actions_do_not_map_to_runtime() {
        let plan = plan_with(vec![("respond", vec![])]);
        assert!(planned_to_runtime_action(&plan.actions[0]).is_none());
    }

    #[test]
    fn test_format_runtime_results_empty_is_empty() {
        assert_eq!(format_runtime_results(&[]), "");
    }

    #[test]
    fn test_format_runtime_results_shows_status() {
        let results = vec![
            (
                "email.send".to_string(),
                ActionResult::succeeded("a", serde_json::json!({})),
            ),
            (
                "telegram.send".to_string(),
                ActionResult::failed("b", "capability_not_found", "not registered", false),
            ),
        ];
        let block = format_runtime_results(&results);
        assert!(block.contains("email.send: SUCCEEDED"));
        assert!(block.contains("telegram.send: FAILED"));
        assert!(block.contains("capability_not_found"));
    }

    #[test]
    fn test_format_runtime_completion_reports_status() {
        let ok = format_runtime_completion(
            "email.send",
            &ActionResult::succeeded("a", serde_json::json!({})),
        );
        assert!(ok.contains("runtime 'email.send' succeeded"));

        let err = format_runtime_completion(
            "telegram.send",
            &ActionResult::failed("b", "capability_not_found", "not registered", false),
        );
        assert!(err.contains("runtime 'telegram.send' failed"));
        assert!(err.contains("not registered"));
    }
}
