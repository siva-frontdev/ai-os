//! # AI-OS Real MCP Server
//!
//! A production Model Context Protocol (MCP) server — the server half of
//! the stdio/JSON-RPC wire protocol spoken by
//! [`ai-os-openclaw-runtime`](https://docs.rs/ai-os-openclaw-runtime).
//!
//! The AI-OS OpenClaw runtime spawns this server as a subprocess (or any
//! MCP-compatible server) and speaks newline-delimited JSON-RPC 2.0 over
//! stdio. This crate hosts **plugins**: named collections of tools that
//! implement real capabilities. In Phase 3 the plugins live in
//! `ai-os-plugins`; this crate only provides the server engine and the
//! plugin contract.
//!
//! ## Wire protocol
//!
//! The server speaks the same execution-focused MCP subset as the client:
//!
//! - `initialize` — version handshake (responds with `protocolVersion`)
//! - `ping`
//! - `tools/list` — advertise the union of all hosted plugins' tools
//! - `tools/call` — invoke one tool, optionally emitting
//!   `notifications/observation` before the response (inbound events)
//!
//! ## Why this crate exists
//!
//! Phase 2 validated the Runtime architecture against an in-process **mock**
//! MCP server. Phase 3 replaces the mock with a real, spawnable server whose
//! tools are implemented by real plugin code. The client never changes: it
//! still just sees an MCP server on the other end of a stdio pipe.
//!
//! ## Layering
//!
//! This crate is a peer of `ai-os-openclaw-runtime`. It deliberately does
//! **not** depend on `ai-os-runtime-api`: it lives on the server side of the
//! wire and never sees AI-OS types. The protocol types in [`protocol`] mirror
//! the client's `mcp::protocol` module byte-for-byte.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod plugin;
pub mod protocol;
pub mod server;
pub mod transport;

pub use error::{JsonRpcError, McpServerError};
pub use plugin::{DefaultMcpPlugin, McpPlugin, ToolOutcome};
pub use protocol::{McpContent, McpTool, PROTOCOL_VERSION};
pub use server::McpServer;
pub use transport::{DuplexServerTransport, McpServerTransport, StdioServerTransport};
