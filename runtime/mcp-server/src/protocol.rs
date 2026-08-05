//! MCP wire types and parsing, server side.
//!
//! These types mirror `ai-os-openclaw-runtime`'s `mcp::protocol` module so
//! both ends of the wire agree on the exact execution-focused MCP subset.
//! They are duplicated (not shared) to keep the server crate independent of
//! the client crate.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The MCP protocol version this server speaks.
pub const PROTOCOL_VERSION: &str = "2025-03-26";

/// A tool advertised by this server (`tools/list`).
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
    /// Any content type the peer does not understand.
    #[serde(other)]
    Unknown,
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, Deserialize)]
pub struct McpJsonRpcError {
    /// JSON-RPC error code.
    pub code: i64,
    /// Error message.
    pub message: String,
}

/// A parsed JSON-RPC 2.0 message.
#[derive(Debug, Clone)]
pub enum McpMessage {
    /// A request with an id (expects a response).
    Request {
        /// Request id.
        id: u64,
        /// Method name.
        method: String,
        /// Params.
        params: Option<Value>,
    },
    /// A response to a previous request (a server never receives these).
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
pub fn parse_message(line: &str) -> Result<McpMessage, crate::McpServerError> {
    let value: Value = serde_json::from_str(line)
        .map_err(|e| crate::McpServerError::Protocol(format!("bad JSON: {e}")))?;
    let id = value.get("id");
    let method = value.get("method").and_then(Value::as_str);
    match (id, method) {
        (Some(id), Some(method)) => Ok(McpMessage::Request {
            id: id.as_u64().ok_or_else(|| {
                crate::McpServerError::Protocol("request id is not an unsigned integer".into())
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
                    crate::McpServerError::Protocol("response id is not an unsigned integer".into())
                })?,
                result: value.get("result").cloned(),
                error,
            })
        }
        _ => Err(crate::McpServerError::Protocol(
            "unrecognized JSON-RPC message".into(),
        )),
    }
}

/// Build the `tools/call` result payload for a plugin outcome.
pub(crate) fn call_result(text: &str, is_error: bool) -> Value {
    json!({
        "content": [{"type": "text", "text": text}],
        "isError": is_error,
    })
}

/// The standard `initialize` result.
pub(crate) fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {"tools": {}},
        "serverInfo": {
            "name": "ai-os-mcp-server",
            "version": env!("CARGO_PKG_VERSION"),
        }
    })
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
    fn parses_response() {
        let msg = parse_message(r#"{"jsonrpc":"2.0","id":3,"result":{"ok":true}}"#).unwrap();
        match msg {
            McpMessage::Response { id, result, error } => {
                assert_eq!(id, 3);
                assert!(result.is_some());
                assert!(error.is_none());
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
    fn call_result_serializes_content_and_is_error() {
        let value = call_result("sent", true);
        assert_eq!(value["isError"], true);
        assert_eq!(value["content"][0]["text"], "sent");
    }
}
