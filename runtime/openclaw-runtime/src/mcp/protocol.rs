//! MCP wire types and protocol-level errors.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The MCP protocol version this client speaks.
pub const PROTOCOL_VERSION: &str = "2025-03-26";

/// Protocol error.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// The server returned a JSON-RPC error.
    #[error("MCP JSON-RPC error {code}: {message}")]
    JsonRpc {
        /// JSON-RPC error code.
        code: i64,
        /// Error message.
        message: String,
    },
    /// The transport failed.
    #[error("MCP transport error: {0}")]
    Transport(String),
    /// The peer violated the protocol.
    #[error("MCP protocol error: {0}")]
    Protocol(String),
    /// The connection closed unexpectedly.
    #[error("MCP connection closed")]
    Closed,
}

/// A tool advertised by an MCP server (`tools/list`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name, e.g. `email.send`.
    pub name: String,
    /// Human description.
    #[serde(default)]
    pub description: String,
    /// JSON Schema for the tool's arguments.
    #[serde(default = "default_input_schema", rename = "inputSchema")]
    pub input_schema: Value,
}

fn default_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": true
    })
}

/// One content item of a `tools/call` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum McpContent {
    /// Text content.
    Text {
        /// The text.
        text: String,
    },
    /// Image content.
    Image {
        /// Base64-encoded image data.
        data: String,
        /// MIME type.
        mime_type: String,
    },
    /// Any content type this client does not understand.
    #[serde(other)]
    Unknown,
}

/// The result of a `tools/call` request.
#[derive(Debug, Clone, Deserialize)]
pub struct McpCallResult {
    /// Content items returned by the tool.
    #[serde(default)]
    pub content: Vec<McpContent>,
    /// True when the tool itself reported an error.
    #[serde(default, rename = "isError")]
    pub is_error: bool,
}

impl McpCallResult {
    /// Concatenated text content, useful for error messages.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| match c {
                McpContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct McpJsonRpcError {
    pub(crate) code: i64,
    pub(crate) message: String,
}

/// A parsed JSON-RPC 2.0 message.
#[derive(Debug, Clone)]
pub(crate) enum McpMessage {
    /// A request with an id (expects a response).
    Request {
        /// Request id.
        id: u64,
        /// Method name.
        method: String,
        /// Params.
        params: Option<Value>,
    },
    /// A response to a previous request.
    Response {
        /// Request id.
        id: u64,
        /// Success result.
        result: Option<Value>,
        /// Error object.
        error: Option<McpJsonRpcError>,
    },
    /// A notification (no id, no response).
    Notification {
        /// Method name.
        method: String,
        /// Params.
        params: Option<Value>,
    },
}

/// Parse one newline-delimited JSON-RPC message.
pub(crate) fn parse_message(line: &str) -> Result<McpMessage, McpError> {
    let value: Value =
        serde_json::from_str(line).map_err(|e| McpError::Protocol(format!("bad JSON: {e}")))?;
    let id = value.get("id");
    let method = value.get("method").and_then(Value::as_str);
    match (id, method) {
        (Some(id), Some(method)) => Ok(McpMessage::Request {
            id: id.as_u64().ok_or_else(|| {
                McpError::Protocol("request id is not an unsigned integer".into())
            })?,
            method: method.to_string(),
            params: value.get("params").cloned(),
        }),
        (None, Some(method)) => Ok(McpMessage::Notification {
            method: method.to_string(),
            params: value.get("params").cloned(),
        }),
        (Some(id), None) => {
            let error = value
                .get("error")
                .filter(|e| !e.is_null())
                .and_then(|e| serde_json::from_value::<McpJsonRpcError>(e.clone()).ok());
            Ok(McpMessage::Response {
                id: id.as_u64().ok_or_else(|| {
                    McpError::Protocol("response id is not an unsigned integer".into())
                })?,
                result: value.get("result").cloned(),
                error,
            })
        }
        _ => Err(McpError::Protocol("unrecognized JSON-RPC message".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_request() {
        let msg =
            parse_message(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#).unwrap();
        match msg {
            McpMessage::Request { id, method, params } => {
                assert_eq!(id, 1);
                assert_eq!(method, "tools/list");
                assert!(params.is_some());
            }
            other => panic!("expected request, got {other:?}"),
        }
    }

    #[test]
    fn parses_response() {
        let msg = parse_message(r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#).unwrap();
        match msg {
            McpMessage::Response { id, result, error } => {
                assert_eq!(id, 1);
                assert!(result.is_some());
                assert!(error.is_none());
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    #[test]
    fn parses_notification() {
        let msg = parse_message(
            r#"{"jsonrpc":"2.0","method":"notifications/observation","params":{"kind":"email.received"}}"#,
        )
        .unwrap();
        match msg {
            McpMessage::Notification { method, params } => {
                assert_eq!(method, "notifications/observation");
                assert_eq!(params.unwrap()["kind"], "email.received");
            }
            other => panic!("expected notification, got {other:?}"),
        }
    }

    #[test]
    fn parses_error_response() {
        let msg = parse_message(
            r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"Method not found"}}"#,
        )
        .unwrap();
        match msg {
            McpMessage::Response { error, .. } => {
                let e = error.unwrap();
                assert_eq!(e.code, -32601);
                assert_eq!(e.message, "Method not found");
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_message("not json").is_err());
    }

    #[test]
    fn tool_round_trips() {
        let tool = McpTool {
            name: "email.send".into(),
            description: "Send an email".into(),
            input_schema: json!({"type": "object"}),
        };
        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["name"], "email.send");
        assert_eq!(json["inputSchema"]["type"], "object");
        let back: McpTool = serde_json::from_value(json).unwrap();
        assert_eq!(back.name, "email.send");
    }

    #[test]
    fn call_result_parses_with_defaults() {
        let result: McpCallResult = serde_json::from_value(json!({
            "content": [{"type": "text", "text": "sent"}],
            "isError": false
        }))
        .unwrap();
        assert!(!result.is_error);
        assert_eq!(result.text(), "sent");
    }
}
