//! Helpers that wire an [`OpenClawRuntime`] to an in-process mock MCP
//! server acting as the "OpenClaw Gateway + Email Plugin".

use std::sync::Arc;

use ai_os_openclaw_runtime::mock::{MockCallResponse, MockMcpServer};
use ai_os_openclaw_runtime::{McpTool, OpenClawRuntime, OpenClawRuntimeConfig};

/// The `email.send` tool as advertised by the (mock) OpenClaw email
/// plugin through MCP.
pub fn email_send_tool() -> McpTool {
    McpTool {
        name: "email.send".into(),
        description: "Send an email message via the configured SMTP relay.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "to": {"type": "string"},
                "subject": {"type": "string"},
                "body": {"type": "string"}
            },
            "required": ["to", "subject", "body"]
        }),
    }
}

/// A mock email plugin handler: returns a success message id and pushes a
/// delivery-confirmation observation back to AI-OS.
pub fn email_plugin_handler(
) -> impl Fn(serde_json::Value) -> MockCallResponse + Send + Sync + 'static {
    |args: serde_json::Value| {
        let to = args
            .get("to")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let mut response = MockCallResponse::ok(
            serde_json::json!({
                "messageId": format!("mock-msg-{}", to.len()),
                "status": "sent",
            })
            .to_string(),
        );
        response.observations.push(serde_json::json!({
            "source": "email.receive",
            "kind": "inbound_message",
            "channel_id": format!("mailbox:{to}"),
            "sender": to,
            "text": "Delivery confirmation: email sent.",
        }));
        response
    }
}

/// A no-op handler for tools the test does not care about.
pub fn default_handler() -> impl Fn(serde_json::Value) -> MockCallResponse + Send + Sync + 'static {
    |_| MockCallResponse::ok("ok")
}

/// Build an [`OpenClawRuntime`] over a mock MCP server advertising the
/// given tools, with a handler for one tool.
pub async fn openclaw_runtime_with_handler(
    tools: Vec<McpTool>,
    handled_tool: &str,
    handler: impl Fn(serde_json::Value) -> MockCallResponse + Send + Sync + 'static,
) -> Arc<OpenClawRuntime> {
    let (client_transport, server_transport) = MockMcpServer::transport_pair();
    let server = MockMcpServer::new(tools).with_handler(handled_tool, handler);
    server.run(server_transport);

    let config = OpenClawRuntimeConfig::default();
    Arc::new(OpenClawRuntime::from_transport(config, client_transport))
}
