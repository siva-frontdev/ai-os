//! Minimal Model Context Protocol (MCP) client and transports.
//!
//! This is a focused subset of the MCP specification sufficient to drive
//! an execution runtime:
//!
//! - `initialize` / `notifications/initialized`
//! - `tools/list`
//! - `tools/call`
//! - `ping`
//! - `notifications/observation` (custom inbound channel used by runtimes
//!   to push observations into AI-OS)
//!
//! Wire format: newline-delimited JSON-RPC 2.0 (the MCP stdio framing).
//! `serde_json` always escapes embedded newlines, so a single message is
//! always one physical line.

pub mod client;
pub mod protocol;
pub mod transport;

pub use client::McpClient;
pub use protocol::{McpCallResult, McpContent, McpError, McpTool};
pub use transport::{ChildTransport, DuplexTransport, McpTransport};
