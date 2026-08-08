//! Result translation: MCP `tools/call` result → AI-OS [`ActionResult`].

use ai_os_runtime_api::{Action, ActionResult, ActionStatus, RuntimeErrorPayload};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::mcp::protocol::{McpCallResult, McpContent};

/// A structured provider error, matching `ProviderError::as_json`:
/// `{"error":{"code":...,"message":...,"retryable":...}}`.
#[derive(Debug, Deserialize)]
struct StructuredError {
    code: String,
    message: String,
    retryable: bool,
}

/// Translate an MCP tool result into a terminal [`ActionResult`].
///
/// - tool reported success → `Succeeded` with structured output
/// - tool reported an error → `Failed`; if the error text is a structured
///   provider error payload, its `code`/`message`/`retryable` are surfaced
///   verbatim (so cognition sees `ConfigurationMissing`,
///   `AuthenticationRequired`, etc. with the provider's retryability);
///   otherwise a generic `tool_error` is reported.
pub fn result_to_action_result(
    action: &Action,
    result: McpCallResult,
    started_ms: u64,
    finished_ms: u64,
) -> ActionResult {
    let output = json!({
        "content": content_to_values(&result.content),
        "is_error": result.is_error,
    });

    if result.is_error {
        return action_result_for_error(action, &result, started_ms, finished_ms);
    }

    ActionResult {
        action_id: action.action_id.clone(),
        status: ActionStatus::Succeeded,
        output: Some(output),
        error: None,
        started_ms,
        finished_ms,
    }
}

/// Build a `Failed` result, preserving a structured provider error when the
/// tool emitted one.
fn action_result_for_error(
    action: &Action,
    result: &McpCallResult,
    started_ms: u64,
    finished_ms: u64,
) -> ActionResult {
    let text = result.text();
    if let Some(structured) = parse_structured_error(&text) {
        ActionResult {
            action_id: action.action_id.clone(),
            status: ActionStatus::Failed,
            output: None,
            error: Some(RuntimeErrorPayload {
                code: structured.code,
                message: structured.message,
                retryable: structured.retryable,
            }),
            started_ms,
            finished_ms,
        }
    } else {
        ActionResult {
            action_id: action.action_id.clone(),
            status: ActionStatus::Failed,
            output: None,
            error: Some(RuntimeErrorPayload {
                code: "tool_error".into(),
                message: if text.is_empty() {
                    "tool returned an error".into()
                } else {
                    text
                },
                retryable: true,
            }),
            started_ms,
            finished_ms,
        }
    }
}

/// If `text` parses as a `ProviderError` JSON payload, return its fields.
fn parse_structured_error(text: &str) -> Option<StructuredError> {
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    let err = value.get("error")?;
    serde_json::from_value::<StructuredError>(err.clone()).ok()
}

fn content_to_values(content: &[McpContent]) -> Vec<Value> {
    content
        .iter()
        .map(|item| match item {
            McpContent::Text { text } => json!(text),
            McpContent::Image { data, mime_type } => json!({
                "type": "image",
                "data": data,
                "mimeType": mime_type,
            }),
            McpContent::Unknown => Value::Null,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::McpContent;

    #[test]
    fn maps_success_result() {
        let action = Action {
            action_id: "act_1".into(),
            capability: "email.send".into(),
            input: json!({}),
            trace_id: None,
            deadline_ms: None,
            metadata: Default::default(),
        };
        let result = McpCallResult {
            content: vec![McpContent::Text {
                text: "{\"messageId\":\"m1\"}".into(),
            }],
            is_error: false,
        };
        let out = result_to_action_result(&action, result, 1, 2);
        assert_eq!(out.status, ai_os_runtime_api::ActionStatus::Succeeded);
        assert!(out.error.is_none());
        assert_eq!(out.output.unwrap()["content"][0], "{\"messageId\":\"m1\"}");
    }

    #[test]
    fn maps_error_result() {
        let action = Action {
            action_id: "act_2".into(),
            capability: "email.send".into(),
            input: json!({}),
            trace_id: None,
            deadline_ms: None,
            metadata: Default::default(),
        };
        let result = McpCallResult {
            content: vec![McpContent::Text {
                text: "SMTP rejected".into(),
            }],
            is_error: true,
        };
        let out = result_to_action_result(&action, result, 1, 2);
        assert_eq!(out.status, ai_os_runtime_api::ActionStatus::Failed);
        assert_eq!(out.error_code(), Some("tool_error"));
        assert!(out.error.unwrap().retryable);
    }
}
