//! WhatsApp plugin: `whatsapp.send` (real delivery through the Meta
//! WhatsApp Cloud API) and `whatsapp.receive` (drains inbound messages that
//! arrived on the webhook into observations for the runtime).
//!
//! Outbound messages report success **only** when Meta confirms with a
//! message id. Inbound messages are queued by the webhook receiver and
//! surfaced to the Cognitive Loop when the runtime calls `whatsapp.receive`.
//! When the provider is not configured both tools return a structured
//! `ConfigurationMissing` error.

use std::sync::Arc;

use ai_os_mcp_server::{McpPlugin, McpServerError, McpTool, ToolOutcome};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::provider::{inbound_observation, ProviderError, WebhookServer, WhatsAppProvider};
use crate::util::{provider_tool_error, str_arg, tool_error};

/// A real WhatsApp plugin hosted by the MCP server.
#[derive(Debug)]
pub struct WhatsAppPlugin {
    provider: Option<Arc<WhatsAppProvider>>,
    /// Optional inbound webhook server; kept alive for the plugin lifetime.
    webhook: Option<WebhookServer>,
}

impl WhatsAppPlugin {
    /// Create a plugin backed by the given WhatsApp provider.
    ///
    /// Pass `None` when WhatsApp credentials are absent; the tools then
    /// report `ConfigurationMissing`.
    pub fn new(provider: Option<Arc<WhatsAppProvider>>) -> Self {
        Self {
            provider,
            webhook: None,
        }
    }

    /// Attach a running inbound webhook server to this plugin, if any.
    pub fn with_webhook(mut self, webhook: Option<WebhookServer>) -> Self {
        self.webhook = webhook;
        self
    }

    /// The underlying provider, if configured.
    pub fn provider(&self) -> Option<Arc<WhatsAppProvider>> {
        self.provider.clone()
    }
}

#[async_trait]
impl McpPlugin for WhatsAppPlugin {
    fn name(&self) -> &'static str {
        "whatsapp"
    }

    fn tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "whatsapp.send".into(),
                description: "Send a WhatsApp text message through the WhatsApp Cloud API".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "to": {"type": "string", "description": "Recipient phone number in E.164 format, e.g. +15551234567"},
                        "text": {"type": "string"}
                    },
                    "required": ["to", "text"]
                }),
            },
            McpTool {
                name: "whatsapp.receive".into(),
                description:
                    "Drain inbound WhatsApp messages received by the webhook into observations"
                        .into(),
                input_schema: json!({"type": "object", "properties": {}}),
            },
        ]
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutcome, McpServerError> {
        match name {
            "whatsapp.send" => self.send(arguments).await,
            "whatsapp.receive" => self.receive(arguments).await,
            _ => Err(McpServerError::UnknownTool(name.to_string())),
        }
    }
}

impl WhatsAppPlugin {
    async fn send(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(to) = str_arg(&args, "to") else {
            return Ok(tool_error("whatsapp.send requires a 'to' recipient"));
        };
        let Some(text) = str_arg(&args, "text") else {
            return Ok(tool_error("whatsapp.send requires a 'text' body"));
        };

        let Some(provider) = &self.provider else {
            return Ok(provider_tool_error(&ProviderError::configuration_missing(
                "WhatsApp provider is not configured: set AIOS_WHATSAPP_ACCESS_TOKEN \
                 and AIOS_WHATSAPP_PHONE_NUMBER_ID",
            )));
        };

        match provider.send(&to, &text).await {
            Ok(receipt) => Ok(ToolOutcome {
                is_error: false,
                result: json!({
                    "messageId": receipt.message_id,
                    "to": receipt.to,
                    "status": "sent"
                }),
                observations: vec![json!({
                    "source": "whatsapp.send",
                    "kind": "status_changed",
                    "channel_id": receipt.to,
                    "text": format!("whatsapp message sent to {}", receipt.to),
                })],
            }),
            Err(err) => Ok(provider_tool_error(&err)),
        }
    }

    async fn receive(&self, _args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(provider) = &self.provider else {
            return Ok(provider_tool_error(&ProviderError::configuration_missing(
                "WhatsApp provider is not configured: set AIOS_WHATSAPP_ACCESS_TOKEN \
                 and AIOS_WHATSAPP_PHONE_NUMBER_ID",
            )));
        };

        let messages = provider.drain();
        let observations: Vec<Value> = messages.iter().map(inbound_observation).collect();
        let count = observations.len();
        Ok(ToolOutcome {
            is_error: false,
            result: json!({ "received": count }),
            observations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::WhatsAppConfig;

    fn unconfigured() -> WhatsAppPlugin {
        WhatsAppPlugin::new(None)
    }

    #[tokio::test]
    async fn send_without_config_is_structured_error() {
        let plugin = unconfigured();
        let outcome = plugin
            .call_tool("whatsapp.send", json!({"to": "+15551234567", "text": "hi"}))
            .await
            .unwrap();
        assert!(outcome.is_error);
        assert_eq!(outcome.result["error"]["code"], "ConfigurationMissing");
        assert!(outcome.observations.is_empty());
    }

    #[tokio::test]
    async fn send_requires_arguments() {
        let plugin = unconfigured();
        let outcome = plugin
            .call_tool("whatsapp.send", json!({"text": "hi"}))
            .await
            .unwrap();
        assert!(outcome.is_error);
        assert_eq!(
            outcome.result["error"],
            "whatsapp.send requires a 'to' recipient"
        );
    }

    #[tokio::test]
    async fn receive_without_config_is_structured_error() {
        let plugin = unconfigured();
        let outcome = plugin
            .call_tool("whatsapp.receive", json!({}))
            .await
            .unwrap();
        assert!(outcome.is_error);
        assert_eq!(outcome.result["error"]["code"], "ConfigurationMissing");
    }

    #[tokio::test]
    async fn receive_drains_queue_into_observations() {
        let provider = WhatsAppProvider::new(test_config());
        let plugin = WhatsAppPlugin::new(Some(Arc::new(provider)));

        let outcome = plugin
            .call_tool("whatsapp.receive", json!({}))
            .await
            .unwrap();
        assert!(!outcome.is_error);
        assert_eq!(outcome.result["received"], 0);
        assert!(outcome.observations.is_empty());
    }

    #[tokio::test]
    async fn unknown_tool_rejected() {
        let plugin = unconfigured();
        assert!(matches!(
            plugin.call_tool("whatsapp.fax", json!({})).await,
            Err(McpServerError::UnknownTool(_))
        ));
    }

    fn test_config() -> WhatsAppConfig {
        WhatsAppConfig {
            access_token: "token".into(),
            phone_number_id: "123456".into(),
            api_version: "v21.0".into(),
            graph_base: "http://localhost:1".into(),
            webhook_verify_token: "verify-token".into(),
            app_secret: "app-secret".into(),
            webhook_host: "127.0.0.1".into(),
            webhook_port: 0,
            webhook_path: "/webhook/whatsapp".into(),
        }
    }
}
