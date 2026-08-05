//! Mock MCP server for deterministic integration testing.
//!
//! Speaks the same newline-delimited JSON-RPC MCP subset as the client.
//! Available when the `testkit` feature is enabled (dev/test builds only —
//! never part of a production dependency graph).

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::mcp::protocol::{parse_message, McpMessage, McpTool};
use crate::mcp::transport::McpTransport;

/// A handler's outcome for one `tools/call`.
#[derive(Debug, Clone, Default)]
pub struct MockCallResponse {
    /// Whether the tool reported an error.
    pub is_error: bool,
    /// Text content to return.
    pub text: String,
    /// Observations to push as `notifications/observation` before the
    /// response (simulates an inbound event caused by the call).
    pub observations: Vec<Value>,
}

impl MockCallResponse {
    /// A successful response with the given text.
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            is_error: false,
            text: text.into(),
            observations: vec![],
        }
    }
}

/// A handler for one tool of the mock server.
pub type ToolHandler = Arc<dyn Fn(Value) -> MockCallResponse + Send + Sync>;

/// An in-process MCP server used by tests in place of a real runtime.
pub struct MockMcpServer {
    tools: Vec<McpTool>,
    handlers: Mutex<HashMap<String, ToolHandler>>,
}

impl MockMcpServer {
    /// Create a server advertising the given tools.
    pub fn new(tools: Vec<McpTool>) -> Self {
        Self {
            tools,
            handlers: Mutex::new(HashMap::new()),
        }
    }

    /// Register a handler for one tool. Calls to unhandled tools fall
    /// back to a generic success response.
    pub fn with_handler(
        mut self,
        tool: &str,
        handler: impl Fn(Value) -> MockCallResponse + Send + Sync + 'static,
    ) -> Self {
        // Exclusive access during construction; no blocking lock needed.
        self.handlers
            .get_mut()
            .insert(tool.to_string(), Arc::new(handler));
        self
    }

    /// Run the server over the given transport in a spawned task.
    pub fn run(self, transport: Arc<dyn McpTransport>) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.serve(transport).await;
        })
    }

    /// Split a duplex pair into a client transport and a server transport.
    pub fn transport_pair() -> (Arc<dyn McpTransport>, Arc<dyn McpTransport>) {
        let (client_end, server_end) = tokio::io::duplex(65536);
        (
            Arc::new(crate::mcp::transport::DuplexTransport::new(client_end)),
            Arc::new(crate::mcp::transport::DuplexTransport::new(server_end)),
        )
    }

    /// Serve requests over a transport until EOF.
    pub async fn serve(self, transport: Arc<dyn McpTransport>) {
        loop {
            let line = match transport.recv_line().await {
                Ok(Some(line)) => line,
                _ => break,
            };
            let message = match parse_message(&line) {
                Ok(message) => message,
                Err(_) => continue,
            };
            match message {
                McpMessage::Request { id, method, params } => {
                    let params = params.unwrap_or(Value::Null);
                    let outcome = self.handle(&method, params).await;
                    if let Some(error) = &outcome.0 {
                        let message = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": error.code,
                                "message": error.message,
                            }
                        });
                        let _ = transport.send_line(&message.to_string()).await;
                        continue;
                    }

                    for observation in &outcome.2 {
                        let notification = json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/observation",
                            "params": observation,
                        });
                        let _ = transport.send_line(&notification.to_string()).await;
                    }

                    let result = outcome.1.unwrap_or(Value::Null);
                    let message = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result,
                    });
                    let _ = transport.send_line(&message.to_string()).await;
                }
                McpMessage::Notification { .. } => {
                    // Notifications require no response.
                }
                McpMessage::Response { .. } => {
                    // A server never receives responses.
                }
            }
        }
    }

    async fn handle(
        &self,
        method: &str,
        params: Value,
    ) -> (Option<MockJsonRpcError>, Option<Value>, Vec<Value>) {
        match method {
            "initialize" => (
                None,
                Some(json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "mock-openclaw", "version": "0.0.0"},
                })),
                vec![],
            ),
            "ping" => (None, Some(json!({})), vec![]),
            "tools/list" => (
                None,
                Some(json!({
                    "tools": self.tools,
                })),
                vec![],
            ),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
                let handler = self
                    .handlers
                    .lock()
                    .await
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| Arc::new(|_: Value| MockCallResponse::ok("ok")));
                let response = handler(arguments);
                let result = json!({
                    "content": [{"type": "text", "text": response.text}],
                    "isError": response.is_error,
                });
                (None, Some(result), response.observations)
            }
            _ => (
                Some(MockJsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {method}"),
                }),
                None,
                vec![],
            ),
        }
    }
}

struct MockJsonRpcError {
    code: i64,
    message: String,
}
