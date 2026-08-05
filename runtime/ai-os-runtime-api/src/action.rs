//! Actions and results: the outbound half of the Runtime API.

use serde::{Deserialize, Serialize};

use crate::capability::CapabilityId;
use crate::time::now_ms;

/// A request to perform one capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Caller-generated correlation id, echoed in the result.
    pub action_id: String,
    /// The capability to invoke.
    pub capability: CapabilityId,
    /// Structured parameters, validated against the capability's
    /// `input_schema` before execution.
    pub input: serde_json::Value,
    /// Optional trace id propagated from the Cognitive Loop.
    pub trace_id: Option<String>,
    /// Optional hard deadline in milliseconds from the epoch.
    pub deadline_ms: Option<u64>,
    /// Free-form metadata.
    pub metadata: std::collections::HashMap<String, String>,
}

/// The outcome of an [`Action`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// The id of the action this result corresponds to.
    pub action_id: String,
    /// Terminal state of the action.
    pub status: ActionStatus,
    /// Structured output on success.
    pub output: Option<serde_json::Value>,
    /// Typed failure details, present when `status != Succeeded`.
    pub error: Option<RuntimeErrorPayload>,
    /// When execution started (ms since epoch).
    pub started_ms: u64,
    /// When execution reached a terminal state (ms since epoch).
    pub finished_ms: u64,
}

impl ActionResult {
    /// Build a successful result.
    pub fn succeeded(action_id: impl Into<String>, output: serde_json::Value) -> Self {
        let started = now_ms();
        Self {
            action_id: action_id.into(),
            status: ActionStatus::Succeeded,
            output: Some(output),
            error: None,
            started_ms: started,
            finished_ms: started,
        }
    }

    /// Build a failed result with a machine-readable error code.
    pub fn failed(
        action_id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        let started = now_ms();
        Self {
            action_id: action_id.into(),
            status: ActionStatus::Failed,
            output: None,
            error: Some(RuntimeErrorPayload {
                code: code.into(),
                message: message.into(),
                retryable,
            }),
            started_ms: started,
            finished_ms: started,
        }
    }

    /// Convenience: whether the action succeeded.
    pub fn is_success(&self) -> bool {
        self.status == ActionStatus::Succeeded
    }

    /// Convenience: the machine error code, if the action failed.
    pub fn error_code(&self) -> Option<&str> {
        self.error.as_ref().map(|e| e.code.as_str())
    }
}

/// Terminal state of an action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionStatus {
    /// Completed successfully.
    Succeeded,
    /// Completed with a failure.
    Failed,
    /// Exceeded its deadline.
    TimedOut,
    /// Cancelled before completion.
    Cancelled,
}

/// Machine-readable error carried inside an [`ActionResult`].
///
/// Codes are stable (e.g. `capability_not_found`, `invalid_input`,
/// `tool_error`) so callers can match without parsing prose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeErrorPayload {
    /// Stable machine code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Whether retrying is likely to succeed.
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_round_trips_through_serde() {
        let action = Action {
            action_id: "act_1".into(),
            capability: CapabilityId::new("email.send"),
            input: serde_json::json!({"to": "a@b.c", "subject": "hi"}),
            trace_id: Some("trace_1".into()),
            deadline_ms: Some(5000),
            metadata: Default::default(),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["capability"], "email.send");
        let back: Action = serde_json::from_value(json).unwrap();
        assert_eq!(back.action_id, "act_1");
        assert_eq!(back.capability, CapabilityId::new("email.send"));
    }

    #[test]
    fn test_succeeded_result() {
        let r = ActionResult::succeeded("act_1", serde_json::json!({"ok": true}));
        assert!(r.is_success());
        assert!(r.error.is_none());
        assert_eq!(r.status, ActionStatus::Succeeded);
        assert!(r.finished_ms >= r.started_ms);
    }

    #[test]
    fn test_failed_result_carries_code() {
        let r = ActionResult::failed("act_2", "capability_not_found", "nope", false);
        assert!(!r.is_success());
        assert_eq!(r.error_code(), Some("capability_not_found"));
        assert!(!r.error.unwrap().retryable);
    }

    #[test]
    fn test_action_status_serde() {
        assert_eq!(
            serde_json::from_str::<ActionStatus>("\"Succeeded\"").unwrap(),
            ActionStatus::Succeeded
        );
    }
}
