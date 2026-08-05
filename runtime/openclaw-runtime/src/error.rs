//! Error type for the OpenClaw runtime adapter.

use crate::mcp::protocol::McpError;

/// Crate-level error for configuration, transport, and protocol problems.
///
/// Per-action failures are *not* represented here — they are carried in
/// [`ActionResult`](ai_os_runtime_api::ActionResult) with typed error
/// payloads.
#[derive(Debug, thiserror::Error)]
pub enum OpenClawError {
    /// The runtime has not been initialized.
    #[error("runtime not initialized: {0}")]
    NotInitialized(String),
    /// Configuration is invalid.
    #[error("configuration error: {0}")]
    Config(String),
    /// The MCP transport failed.
    #[error("MCP transport error: {0}")]
    Mcp(#[from] McpError),
    /// The MCP handshake or protocol contract was violated.
    #[error("MCP protocol error: {0}")]
    Protocol(String),
    /// Any other internal failure.
    #[error("internal openclaw runtime error: {0}")]
    Internal(String),
}

impl OpenClawError {
    /// Convert into the vendor-neutral interface error.
    pub fn to_runtime_error(&self) -> ai_os_runtime_api::RuntimeError {
        ai_os_runtime_api::RuntimeError::Transport(self.to_string())
    }
}
