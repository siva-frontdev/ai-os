//! Result translation: MCP `tools/call` result → AI-OS [`ActionResult`].

use ai_os_runtime_api::{Action, ActionResult, RuntimeErrorPayload};
use serde_json::{json, Value};

use crate::mcp::protocol::{McpCallResult, McpContent};

/// Translate an MCP tool result into a terminal [`ActionResult`].
///
/// - tool reported success → `Succeeded` with structured output
/// - tool reported an error → `Failed` with code `tool_error`
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
        return ActionResult {
            action_id: action.action_id.clone(),
            status: ai_os_runtime_api::ActionStatus::Failed,
            output: None,
            error: Some(RuntimeErrorPayload {
                code: "tool_error".into(),
                message: if result.text().is_empty() {
                    "tool returned an error".into()
                } else {
                    result.text()
                },
                retryable: true,
            }),
            started_ms,
            finished_ms,
        };
    }

    ActionResult {
        action_id: action.action_id.clone(),
        status: ai_os_runtime_api::ActionStatus::Succeeded,
        output: Some(output),
        error: None,
        started_ms,
        finished_ms,
    }
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
