//! Server-side errors and JSON-RPC error objects.

/// A server-side failure.
#[derive(Debug, thiserror::Error)]
pub enum McpServerError {
    /// The transport failed (I/O, EOF handling).
    #[error("transport failure: {0}")]
    Transport(String),
    /// The client violated the protocol.
    #[error("protocol violation: {0}")]
    Protocol(String),
    /// A plugin failed while invoking a tool.
    #[error("plugin error: {0}")]
    Plugin(String),
    /// The requested method is not implemented.
    #[error("method not found: {0}")]
    MethodNotFound(String),
    /// The requested tool is not hosted by any plugin.
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    /// The connection closed.
    #[error("connection closed")]
    Closed,
}

/// A JSON-RPC 2.0 error object sent to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRpcError {
    /// JSON-RPC error code.
    pub code: i64,
    /// Error message.
    pub message: String,
}

impl JsonRpcError {
    /// Standard `-32601` method-not-found error.
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
        }
    }

    /// Standard `-32602` invalid-params error.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }

    /// Standard `-32603` internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_not_found_uses_standard_code() {
        let err = JsonRpcError::method_not_found("bogus");
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found: bogus");
    }

    #[test]
    fn invalid_params_uses_standard_code() {
        let err = JsonRpcError::invalid_params("missing name");
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn internal_uses_standard_code() {
        let err = JsonRpcError::internal("boom");
        assert_eq!(err.code, -32603);
    }
}
