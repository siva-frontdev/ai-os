//! Email plugin: `email.send` (outbound) and `email.inject` (inbound
//! simulation). Maintains an outbox and an inbox; `email.send` emits a
//! `status_changed` observation (the runtime observes its own action).

use std::sync::Arc;

use ai_os_mcp_server::{McpPlugin, McpServerError, McpTool, ToolOutcome};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::util::{now_rfc3339, str_arg};

/// Internal shared state of the email plugin.
#[derive(Debug)]
struct EmailInner {
    outbox: Mutex<Vec<Value>>,
    inbox: Mutex<Vec<Value>>,
}

/// A real email plugin hosted by the MCP server.
#[derive(Debug)]
pub struct EmailPlugin {
    inner: Arc<EmailInner>,
}

impl Default for EmailPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl EmailPlugin {
    /// Create a plugin with an empty outbox and inbox.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(EmailInner {
                outbox: Mutex::new(Vec::new()),
                inbox: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Number of emails in the outbox.
    pub async fn sent_count(&self) -> usize {
        self.inner.outbox.lock().await.len()
    }

    /// Number of emails waiting in the inbox.
    pub async fn inbox_count(&self) -> usize {
        self.inner.inbox.lock().await.len()
    }
}

#[async_trait]
impl McpPlugin for EmailPlugin {
    fn name(&self) -> &'static str {
        "email"
    }

    fn tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "email.send".into(),
                description: "Send an email message to a recipient".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "to": {"type": "string"},
                        "subject": {"type": "string"},
                        "body": {"type": "string"}
                    },
                    "required": ["to"]
                }),
            },
            McpTool {
                name: "email.inject".into(),
                description:
                    "Simulate an inbound email (test/dev). Emits an inbound_message observation"
                        .into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "from": {"type": "string"},
                        "to": {"type": "string"},
                        "subject": {"type": "string"},
                        "body": {"type": "string"}
                    },
                    "required": ["from", "to"]
                }),
            },
        ]
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutcome, McpServerError> {
        match name {
            "email.send" => self.send(arguments).await,
            "email.inject" => self.inject(arguments).await,
            _ => Err(McpServerError::UnknownTool(name.to_string())),
        }
    }
}

impl EmailPlugin {
    async fn send(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(to) = str_arg(&args, "to") else {
            return Ok(tool_error("email.send requires a 'to' recipient"));
        };
        let subject = str_arg(&args, "subject").unwrap_or_default();
        let body = str_arg(&args, "body").unwrap_or_default();
        let message_id = uuid::Uuid::new_v4().to_string();

        self.inner.outbox.lock().await.push(json!({
            "messageId": message_id,
            "to": to,
            "subject": subject,
            "body": body,
            "sentAt": now_rfc3339(),
        }));

        Ok(ToolOutcome {
            is_error: false,
            result: json!({
                "messageId": message_id,
                "to": to,
                "subject": subject,
                "status": "sent"
            }),
            observations: vec![json!({
                "source": "email.send",
                "kind": "status_changed",
                "channel_id": to,
                "text": format!("email sent to {to}: {subject}"),
            })],
        })
    }

    async fn inject(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(from) = str_arg(&args, "from") else {
            return Ok(tool_error("email.inject requires a 'from' address"));
        };
        let Some(to) = str_arg(&args, "to") else {
            return Ok(tool_error("email.inject requires a 'to' address"));
        };
        let subject = str_arg(&args, "subject").unwrap_or_default();
        let body = str_arg(&args, "body").unwrap_or_default();
        let message_id = uuid::Uuid::new_v4().to_string();

        self.inner.inbox.lock().await.push(json!({
            "messageId": message_id,
            "from": from,
            "to": to,
            "subject": subject,
            "body": body,
            "receivedAt": now_rfc3339(),
        }));

        Ok(ToolOutcome {
            is_error: false,
            result: json!({
                "messageId": message_id,
                "injected": true,
                "subject": subject
            }),
            observations: vec![json!({
                "source": "email.receive",
                "kind": "inbound_message",
                "channel_id": to,
                "sender": from,
                "subject": subject,
                "body": body,
                "text": subject,
            })],
        })
    }
}

fn tool_error(message: &str) -> ToolOutcome {
    ToolOutcome {
        is_error: true,
        result: json!({"error": message}),
        observations: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn email_send_records_outbox_and_emits_observation() {
        let plugin = EmailPlugin::new();
        let outcome = plugin
            .call_tool(
                "email.send",
                json!({"to": "a@b.c", "subject": "hi", "body": "hello"}),
            )
            .await
            .unwrap();
        assert!(!outcome.is_error);
        assert_eq!(outcome.result["status"], "sent");
        assert_eq!(outcome.result["to"], "a@b.c");
        assert!(outcome.result["messageId"].as_str().is_some());
        assert_eq!(plugin.sent_count().await, 1);
        assert_eq!(outcome.observations.len(), 1);
        assert_eq!(outcome.observations[0]["source"], "email.send");
        assert_eq!(outcome.observations[0]["kind"], "status_changed");
    }

    #[tokio::test]
    async fn email_inject_emits_inbound_observation() {
        let plugin = EmailPlugin::new();
        let outcome = plugin
            .call_tool(
                "email.inject",
                json!({"from": "friend@x.dev", "to": "aios@x.dev", "subject": "news"}),
            )
            .await
            .unwrap();
        assert!(!outcome.is_error);
        assert_eq!(plugin.inbox_count().await, 1);
        let obs = &outcome.observations[0];
        assert_eq!(obs["source"], "email.receive");
        assert_eq!(obs["kind"], "inbound_message");
        assert_eq!(obs["sender"], "friend@x.dev");
        assert_eq!(obs["channel_id"], "aios@x.dev");
    }

    #[tokio::test]
    async fn email_send_requires_to() {
        let plugin = EmailPlugin::new();
        let outcome = plugin
            .call_tool("email.send", json!({"subject": "no recipient"}))
            .await
            .unwrap();
        assert!(outcome.is_error);
        assert_eq!(plugin.sent_count().await, 0);
    }

    #[tokio::test]
    async fn unknown_tool_rejected() {
        let plugin = EmailPlugin::new();
        assert!(matches!(
            plugin.call_tool("email.fax", json!({})).await,
            Err(McpServerError::UnknownTool(_))
        ));
    }
}
