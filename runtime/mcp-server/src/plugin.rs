//! The plugin contract: a named collection of tools hosted by the server.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::McpServerError;
use crate::protocol::McpTool;

/// The outcome of one tool call.
#[derive(Debug, Clone, Default)]
pub struct ToolOutcome {
    /// Whether the tool reported an error (`isError`).
    pub is_error: bool,
    /// Structured result, serialized as the tool's text content.
    pub result: Value,
    /// Observations to emit as `notifications/observation` **before** the
    /// response. This is how a tool surfaces an inbound event (e.g. an
    /// email that just arrived, or a telegram message that triggered the
    /// action).
    pub observations: Vec<Value>,
}

/// A named collection of tools (one capability area) hosted by the MCP
/// server.
///
/// A plugin never reasons, plans, or prompts — it translates a tool call
/// into an effect and reports the outcome. Observations it produces are
/// passed to the client verbatim; AI-OS cognition decides what they mean.
#[async_trait]
pub trait McpPlugin: std::fmt::Debug + Send + Sync {
    /// Stable plugin name, e.g. `email`.
    fn name(&self) -> &'static str;

    /// The tools this plugin advertises in `tools/list`.
    fn tools(&self) -> Vec<McpTool>;

    /// Invoke one of this plugin's tools.
    ///
    /// Returning `Err` is reserved for plugin failures; the server answers
    /// with a JSON-RPC internal error. Tool-level failures (e.g. file not
    /// found) should be reported inside the outcome with `is_error` set.
    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutcome, McpServerError>;
}

/// The default plugin: hosts no tools and answers every call with a typed
/// error. Serves as a sane base for tests and unconfigured servers.
#[derive(Debug, Default)]
pub struct DefaultMcpPlugin;

#[async_trait]
impl McpPlugin for DefaultMcpPlugin {
    fn name(&self) -> &'static str {
        "default"
    }

    fn tools(&self) -> Vec<McpTool> {
        Vec::new()
    }

    async fn call_tool(
        &self,
        name: &str,
        _arguments: Value,
    ) -> Result<ToolOutcome, McpServerError> {
        Err(McpServerError::UnknownTool(name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_plugin_has_no_tools() {
        let plugin = DefaultMcpPlugin;
        assert_eq!(plugin.name(), "default");
        assert!(plugin.tools().is_empty());
    }

    #[tokio::test]
    async fn default_plugin_rejects_tool_calls() {
        let plugin = DefaultMcpPlugin;
        let err = plugin
            .call_tool("email.send", Value::Null)
            .await
            .unwrap_err();
        assert!(matches!(err, McpServerError::UnknownTool(_)));
    }
}
