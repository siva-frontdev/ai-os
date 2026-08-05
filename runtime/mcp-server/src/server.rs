//! The [`McpServer`]: hosts plugins and serves the MCP subset.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::error::JsonRpcError;
use crate::plugin::McpPlugin;
use crate::protocol::{call_result, initialize_result, parse_message, McpMessage, McpTool};
use crate::transport::McpServerTransport;

/// The outcome of handling one request: an error, a result, and any
/// observations to emit before the response.
type HandleOutcome = (Option<JsonRpcError>, Option<Value>, Vec<Value>);

/// A real MCP server that hosts tool plugins.
///
/// Construct one with [`McpServer::new`], then serve it over a transport
/// (stdio in production, a duplex pair in tests).
#[derive(Debug)]
pub struct McpServer {
    plugins: Vec<Arc<dyn McpPlugin>>,
    registry: HashMap<String, Arc<dyn McpPlugin>>,
}

impl McpServer {
    /// Build a server hosting the given plugins. Duplicate tool names
    /// across plugins resolve to the first plugin that declared them.
    pub fn new(plugins: Vec<Arc<dyn McpPlugin>>) -> Self {
        let mut registry = HashMap::new();
        for plugin in &plugins {
            for tool in plugin.tools() {
                let name = tool.name.clone();
                if let std::collections::hash_map::Entry::Vacant(entry) = registry.entry(name) {
                    entry.insert(plugin.clone());
                } else {
                    tracing::warn!(
                        tool = %tool.name,
                        plugin = %plugin.name(),
                        "tool already hosted by an earlier plugin; keeping the first"
                    );
                }
            }
        }
        Self { plugins, registry }
    }

    /// The hosted plugins.
    pub fn plugins(&self) -> &[Arc<dyn McpPlugin>] {
        &self.plugins
    }

    /// The union of all hosted plugins' tools, deduplicated by name (first
    /// plugin wins, matching the registry).
    pub fn tools(&self) -> Vec<McpTool> {
        let mut seen = std::collections::HashSet::new();
        let mut tools = Vec::new();
        for plugin in &self.plugins {
            for tool in plugin.tools() {
                if seen.insert(tool.name.clone()) {
                    tools.push(tool);
                }
            }
        }
        tools
    }

    /// Handle one request. Exposed for tests that drive the server without
    /// a transport.
    pub async fn handle(&self, method: &str, params: Value) -> HandleOutcome {
        match method {
            "initialize" => (None, Some(initialize_result()), vec![]),
            "ping" => (None, Some(json!({})), vec![]),
            "tools/list" => (None, Some(json!({ "tools": self.tools() })), vec![]),
            "tools/call" => self.handle_call(params).await,
            _ => (Some(JsonRpcError::method_not_found(method)), None, vec![]),
        }
    }

    /// Serve requests over a transport until EOF.
    pub async fn serve(self, transport: Arc<dyn McpServerTransport>) {
        loop {
            let line = match transport.recv_line().await {
                Ok(Some(line)) => line,
                _ => break,
            };
            let message = match parse_message(&line) {
                Ok(message) => message,
                Err(e) => {
                    tracing::debug!(error = %e, "dropping unparseable message");
                    continue;
                }
            };
            match message {
                McpMessage::Request { id, method, params } => {
                    let params = params.unwrap_or(Value::Null);
                    let (error, result, observations) = self.handle(&method, params).await;

                    if let Some(error) = error {
                        let response = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": error.code, "message": error.message },
                        });
                        let _ = transport.send_line(&response.to_string()).await;
                        continue;
                    }

                    for observation in &observations {
                        let notification = json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/observation",
                            "params": observation,
                        });
                        let _ = transport.send_line(&notification.to_string()).await;
                    }

                    let result = result.unwrap_or(Value::Null);
                    let response = json!({ "jsonrpc": "2.0", "id": id, "result": result });
                    let _ = transport.send_line(&response.to_string()).await;
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

    async fn handle_call(&self, params: Value) -> HandleOutcome {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| JsonRpcError::invalid_params("tools/call requires a 'name' string"))
            .map(String::from);
        let name = match name {
            Ok(name) => name,
            Err(error) => return (Some(error), None, vec![]),
        };
        let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

        let Some(plugin) = self.registry.get(&name).cloned() else {
            return (
                Some(JsonRpcError::invalid_params(format!(
                    "unknown tool: {name}"
                ))),
                None,
                vec![],
            );
        };

        match plugin.call_tool(&name, arguments).await {
            Ok(outcome) => (
                None,
                Some(call_result(&outcome.result.to_string(), outcome.is_error)),
                outcome.observations,
            ),
            Err(e) => (
                Some(JsonRpcError::internal(format!("tool '{name}' failed: {e}"))),
                None,
                vec![],
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{DefaultMcpPlugin, ToolOutcome};
    use crate::protocol::McpTool;
    use crate::PROTOCOL_VERSION;

    #[derive(Debug)]
    struct EchoPlugin;

    #[async_trait::async_trait]
    impl McpPlugin for EchoPlugin {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn tools(&self) -> Vec<McpTool> {
            vec![McpTool {
                name: "echo.echo".into(),
                description: "echo input".into(),
                input_schema: json!({"type": "object"}),
            }]
        }
        async fn call_tool(
            &self,
            name: &str,
            arguments: Value,
        ) -> Result<ToolOutcome, crate::McpServerError> {
            assert_eq!(name, "echo.echo");
            Ok(ToolOutcome {
                is_error: false,
                result: json!({"echoed": arguments}),
                observations: vec![json!({"source": "echo.received", "kind": "inbound_message"})],
            })
        }
    }

    #[tokio::test]
    async fn initialize_ping_and_tools_list() {
        let server = McpServer::new(vec![Arc::new(EchoPlugin)]);
        let (error, result, _) = server.handle("initialize", json!({})).await;
        assert!(error.is_none());
        assert_eq!(result.unwrap()["protocolVersion"], PROTOCOL_VERSION);

        let (error, result, _) = server.handle("ping", json!({})).await;
        assert!(error.is_none());
        assert!(result.is_some());

        let (error, result, _) = server.handle("tools/list", json!({})).await;
        assert!(error.is_none());
        let tools = result.unwrap()["tools"].as_array().unwrap().clone();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "echo.echo");
    }

    #[tokio::test]
    async fn tool_call_returns_result_and_observations() {
        let server = McpServer::new(vec![Arc::new(EchoPlugin)]);
        let (error, result, observations) = server
            .handle(
                "tools/call",
                json!({"name": "echo.echo", "arguments": {"x": 1}}),
            )
            .await;
        assert!(error.is_none());
        let result = result.unwrap();
        assert_eq!(result["isError"], false);
        assert_eq!(result["content"][0]["text"], r#"{"echoed":{"x":1}}"#);
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0]["source"], "echo.received");
    }

    #[tokio::test]
    async fn unknown_tool_is_an_invalid_params_error() {
        let server = McpServer::new(vec![Arc::new(EchoPlugin)]);
        let (error, result, observations) =
            server.handle("tools/call", json!({"name": "nope"})).await;
        assert!(error.is_some());
        assert_eq!(error.unwrap().code, -32602);
        assert!(result.is_none());
        assert!(observations.is_empty());
    }

    #[tokio::test]
    async fn unknown_method_is_method_not_found() {
        let server = McpServer::new(vec![Arc::new(DefaultMcpPlugin)]);
        let (error, ..) = server.handle("bogus", json!({})).await;
        assert_eq!(error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn duplicate_tool_names_resolve_to_first_plugin() {
        #[derive(Debug)]
        struct A;
        #[derive(Debug)]
        struct B;
        #[async_trait::async_trait]
        impl McpPlugin for A {
            fn name(&self) -> &'static str {
                "a"
            }
            fn tools(&self) -> Vec<McpTool> {
                vec![McpTool {
                    name: "dup.tool".into(),
                    description: "a".into(),
                    input_schema: json!({}),
                }]
            }
            async fn call_tool(
                &self,
                _name: &str,
                _args: Value,
            ) -> Result<ToolOutcome, crate::McpServerError> {
                Ok(ToolOutcome {
                    is_error: false,
                    result: json!({"owner": "a"}),
                    observations: vec![],
                })
            }
        }
        #[async_trait::async_trait]
        impl McpPlugin for B {
            fn name(&self) -> &'static str {
                "b"
            }
            fn tools(&self) -> Vec<McpTool> {
                vec![McpTool {
                    name: "dup.tool".into(),
                    description: "b".into(),
                    input_schema: json!({}),
                }]
            }
            async fn call_tool(
                &self,
                _name: &str,
                _args: Value,
            ) -> Result<ToolOutcome, crate::McpServerError> {
                Ok(ToolOutcome {
                    is_error: false,
                    result: json!({"owner": "b"}),
                    observations: vec![],
                })
            }
        }

        let server = McpServer::new(vec![Arc::new(A), Arc::new(B)]);
        assert_eq!(server.tools().len(), 1);
        let (error, result, _) = server
            .handle("tools/call", json!({"name": "dup.tool", "arguments": {}}))
            .await;
        assert!(error.is_none());
        assert_eq!(result.unwrap()["content"][0]["text"], r#"{"owner":"a"}"#);
    }
}
