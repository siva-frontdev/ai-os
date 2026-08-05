//! A minimal asynchronous MCP client.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use super::protocol::{
    parse_message, McpCallResult, McpError, McpMessage, McpTool, PROTOCOL_VERSION,
};
use super::transport::McpTransport;

/// A client for a small, execution-focused subset of the MCP protocol.
///
/// The client owns a background reader task that dispatches responses to
/// pending requests and forwards `notifications/observation` payloads to
/// a channel consumed by the owning runtime.
#[derive(Debug)]
pub struct McpClient {
    transport: Arc<dyn McpTransport>,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<McpMessage>>>>,
    observation_rx: Mutex<mpsc::UnboundedReceiver<Value>>,
    reader: Mutex<Option<JoinHandle<()>>>,
    closed: Arc<AtomicBool>,
}

impl McpClient {
    /// Connect to an MCP server and perform the `initialize` handshake.
    pub async fn connect(transport: Arc<dyn McpTransport>) -> Result<Arc<Self>, McpError> {
        let (observation_tx, observation_rx) = mpsc::unbounded_channel::<Value>();
        let client = Arc::new(McpClient {
            transport,
            next_id: AtomicU64::new(1),
            pending: Arc::new(Mutex::new(HashMap::new())),
            observation_rx: Mutex::new(observation_rx),
            reader: Mutex::new(None),
            closed: Arc::new(AtomicBool::new(false)),
        });

        let reader = {
            let transport = client.transport.clone();
            let pending = client.pending.clone();
            let observation_tx = observation_tx.clone();
            let closed = client.closed.clone();
            tokio::spawn(async move {
                Self::reader_loop(transport, pending, observation_tx, closed).await;
            })
        };
        *client.reader.lock().await = Some(reader);

        let result = client
            .request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "ai-os-openclaw-runtime",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }),
            )
            .await?;

        let version = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                McpError::Protocol("initialize response missing protocolVersion".into())
            })?;
        tracing::debug!(server_protocol_version = version, "MCP initialize complete");

        client
            .notify("notifications/initialized", json!({}))
            .await?;
        Ok(client)
    }

    async fn reader_loop(
        transport: Arc<dyn McpTransport>,
        pending: Arc<Mutex<HashMap<u64, oneshot::Sender<McpMessage>>>>,
        observation_tx: mpsc::UnboundedSender<Value>,
        closed: Arc<AtomicBool>,
    ) {
        while let Ok(Some(line)) = transport.recv_line().await {
            let Ok(message) = parse_message(&line) else {
                continue;
            };
            match message {
                McpMessage::Response { id, result, error } => {
                    let sender = pending.lock().await.remove(&id);
                    if let Some(sender) = sender {
                        let _ = sender.send(McpMessage::Response { id, result, error });
                    }
                }
                McpMessage::Notification { method, params } => {
                    if method == "notifications/observation" {
                        let _ = observation_tx.send(params.unwrap_or(Value::Null));
                    }
                    // Other notifications (e.g. notifications/message)
                    // are intentionally ignored.
                }
                McpMessage::Request { .. } => {
                    // Unsolicited server requests are not supported by
                    // this minimal client.
                }
            }
        }
        closed.store(true, Ordering::SeqCst);
    }

    /// Send a request and await its response.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, McpError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(McpError::Closed);
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        self.transport.send_line(&message.to_string()).await?;

        let reply = rx.await.map_err(|_| McpError::Closed)?;
        match reply {
            McpMessage::Response { result, error, .. } => {
                if let Some(error) = error {
                    return Err(McpError::JsonRpc {
                        code: error.code,
                        message: error.message,
                    });
                }
                Ok(result.unwrap_or(Value::Null))
            }
            _ => Err(McpError::Protocol("unexpected message type".into())),
        }
    }

    /// Send a notification (no response expected).
    pub async fn notify(&self, method: &str, params: Value) -> Result<(), McpError> {
        let message = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.transport.send_line(&message.to_string()).await
    }

    /// List tools advertised by the server.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let result = self.request("tools/list", json!({})).await?;
        let tools = result
            .get("tools")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![]));
        serde_json::from_value(tools)
            .map_err(|e| McpError::Protocol(format!("tools/list returned invalid tools: {e}")))
    }

    /// Invoke a tool and return its structured result.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpCallResult, McpError> {
        let result = self
            .request(
                "tools/call",
                json!({
                    "name": name,
                    "arguments": arguments
                }),
            )
            .await?;
        serde_json::from_value(result)
            .map_err(|e| McpError::Protocol(format!("tools/call returned invalid result: {e}")))
    }

    /// Ping the server.
    pub async fn ping(&self) -> Result<(), McpError> {
        self.request("ping", json!({})).await.map(|_| ())
    }

    /// Take ownership of the observation notification channel.
    ///
    /// Called once by the owning runtime so it can forward observations
    /// into the runtime's event stream. Messages already queued are kept.
    pub async fn take_observation_receiver(&self) -> mpsc::UnboundedReceiver<Value> {
        let mut slot = self.observation_rx.lock().await;
        let (_, fresh) = mpsc::unbounded_channel();
        std::mem::replace(&mut *slot, fresh)
    }

    /// True once the connection has closed.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::McpTool;

    #[tokio::test]
    async fn list_tools_and_call_tool_round_trip() {
        let (a, b) = tokio::io::duplex(65536);
        let server = crate::mock::MockMcpServer::new(vec![McpTool {
            name: "echo".into(),
            description: "echo".into(),
            input_schema: json!({"type": "object"}),
        }]);
        server.run(Arc::new(crate::mcp::DuplexTransport::new(b)));

        let client = McpClient::connect(Arc::new(crate::mcp::DuplexTransport::new(a)))
            .await
            .unwrap();
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");

        let call = client.call_tool("echo", json!({"x": 1})).await.unwrap();
        assert!(!call.is_error);
        assert!(client.ping().await.is_ok());
    }

    #[tokio::test]
    async fn unknown_method_yields_json_rpc_error() {
        let (a, b) = tokio::io::duplex(65536);
        let server = crate::mock::MockMcpServer::new(vec![]);
        server.run(Arc::new(crate::mcp::DuplexTransport::new(b)));
        let client = McpClient::connect(Arc::new(crate::mcp::DuplexTransport::new(a)))
            .await
            .unwrap();
        let err = client.request("bogus/method", json!({})).await.unwrap_err();
        assert!(matches!(err, McpError::JsonRpc { code: -32601, .. }));
    }
}
