//! Telegram plugin: `telegram.send` (outbound) and
//! `telegram.inject_inbound` (simulated inbound message, emits an
//! `inbound_message` observation).

use std::sync::Arc;

use ai_os_mcp_server::{McpPlugin, McpServerError, McpTool, ToolOutcome};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::util::str_arg;

/// Internal state: messages sent to chats.
#[derive(Debug)]
struct TelegramInner {
    sent: Mutex<Vec<Value>>,
}

/// A real Telegram plugin. `telegram.send` is a stub (no network); inbound
/// messages are simulated via `telegram.inject_inbound`.
#[derive(Debug)]
pub struct TelegramPlugin {
    inner: Arc<TelegramInner>,
}

impl Default for TelegramPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl TelegramPlugin {
    /// Create a plugin with no sent messages.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TelegramInner {
                sent: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Number of messages sent.
    pub async fn sent_count(&self) -> usize {
        self.inner.sent.lock().await.len()
    }
}

#[async_trait]
impl McpPlugin for TelegramPlugin {
    fn name(&self) -> &'static str {
        "telegram"
    }

    fn tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "telegram.send".into(),
                description: "Post a message to a chat".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "chat_id": {"type": "string"},
                        "text": {"type": "string"}
                    },
                    "required": ["chat_id", "text"]
                }),
            },
            McpTool {
                name: "telegram.inject_inbound".into(),
                description:
                    "Simulate an inbound chat message (test/dev). Emits an inbound_message observation"
                        .into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "chat_id": {"type": "string"},
                        "sender": {"type": "string"},
                        "text": {"type": "string"}
                    },
                    "required": ["chat_id", "sender", "text"]
                }),
            },
        ]
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutcome, McpServerError> {
        match name {
            "telegram.send" => self.send(arguments).await,
            "telegram.inject_inbound" => Ok(self.inject_inbound(arguments)),
            _ => Err(McpServerError::UnknownTool(name.to_string())),
        }
    }
}

impl TelegramPlugin {
    async fn send(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(chat_id) = str_arg(&args, "chat_id") else {
            return Ok(tool_error("telegram.send requires a 'chat_id'"));
        };
        let Some(text) = str_arg(&args, "text") else {
            return Ok(tool_error("telegram.send requires a 'text'"));
        };
        let message_id = uuid::Uuid::new_v4().to_string();

        self.inner.sent.lock().await.push(json!({
            "messageId": message_id,
            "chat_id": chat_id,
            "text": text,
        }));

        Ok(ToolOutcome {
            is_error: false,
            result: json!({
                "messageId": message_id,
                "chat_id": chat_id,
                "text": text,
                "status": "sent"
            }),
            observations: vec![],
        })
    }

    fn inject_inbound(&self, args: Value) -> ToolOutcome {
        let Some(chat_id) = str_arg(&args, "chat_id") else {
            return tool_error("telegram.inject_inbound requires a 'chat_id'");
        };
        let Some(sender) = str_arg(&args, "sender") else {
            return tool_error("telegram.inject_inbound requires a 'sender'");
        };
        let Some(text) = str_arg(&args, "text") else {
            return tool_error("telegram.inject_inbound requires a 'text'");
        };

        ToolOutcome {
            is_error: false,
            result: json!({
                "injected": true,
                "chat_id": chat_id,
                "sender": sender,
                "text": text,
            }),
            observations: vec![json!({
                "source": "telegram.receive",
                "kind": "inbound_message",
                "channel_id": chat_id,
                "sender": sender,
                "text": text,
            })],
        }
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
    async fn send_records_message() {
        let plugin = TelegramPlugin::new();
        let outcome = plugin
            .call_tool("telegram.send", json!({"chat_id": "c1", "text": "hi"}))
            .await
            .unwrap();
        assert!(!outcome.is_error);
        assert_eq!(outcome.result["status"], "sent");
        assert!(outcome.result["messageId"].as_str().is_some());
        assert_eq!(plugin.sent_count().await, 1);
        assert!(outcome.observations.is_empty());
    }

    #[tokio::test]
    async fn inject_inbound_emits_inbound_observation() {
        let plugin = TelegramPlugin::new();
        let outcome = plugin
            .call_tool(
                "telegram.inject_inbound",
                json!({"chat_id": "c9", "sender": "alice", "text": "send email"}),
            )
            .await
            .unwrap();
        assert!(!outcome.is_error);
        let obs = &outcome.observations[0];
        assert_eq!(obs["source"], "telegram.receive");
        assert_eq!(obs["kind"], "inbound_message");
        assert_eq!(obs["channel_id"], "c9");
        assert_eq!(obs["sender"], "alice");
        assert_eq!(obs["text"], "send email");
    }

    #[tokio::test]
    async fn send_requires_chat_id_and_text() {
        let plugin = TelegramPlugin::new();
        assert!(
            plugin
                .call_tool("telegram.send", json!({"text": "x"}))
                .await
                .unwrap()
                .is_error
        );
        assert!(
            plugin
                .call_tool("telegram.send", json!({"chat_id": "c1"}))
                .await
                .unwrap()
                .is_error
        );
    }
}
