//! # AI-OS OpenClaw Runtime
//!
//! An implementation of the vendor-neutral [`Runtime`] trait
//! (`ai-os-runtime-api`) that drives **OpenClaw** as an execution
//! runtime.
//!
//! OpenClaw is the *hands and legs*: it exposes capabilities (channels,
//! tools, connectors) and produces observations. It never reasons, plans,
//! prompts, keeps memory, or makes decisions — that is exclusively the
//! job of the AI-OS cognitive core.
//!
//! ## Boundary
//!
//! ```text
//! AI-OS Brain (cognition only)
//!    │  Action { capability: "email.send", input }
//!    ▼
//! OpenClawRuntime (this crate)
//!    │  capability map → tool name
//!    │  action map → tool arguments
//!    │  result map → ActionResult
//!    │  observation map → Observation
//!    ▼
//! MCP client (tools/list, tools/call)
//!    │  newline-delimited JSON-RPC 2.0
//!    ▼
//! OpenClaw Gateway / any MCP server
//! ```
//!
//! AI-OS never imports this crate. The wiring point registers an
//! [`OpenClawRuntime`] behind `Arc<dyn Runtime>`, so replacing OpenClaw
//! requires changes only here — never in `brain/` or `runtime-api`.
//!
//! ## No dependencies on OpenClaw code
//!
//! This crate communicates exclusively through the Model Context Protocol
//! (MCP). It contains no OpenClaw types and requires no OpenClaw
//! libraries.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod action_map;
pub mod capability_map;
pub mod config;
pub mod error;
pub mod mcp;
pub mod observation_map;
pub mod openclaw_runtime;
pub mod result_map;

#[cfg(feature = "testkit")]
pub mod mock;

pub use config::{OpenClawRuntimeConfig, TransportConfig};
pub use error::OpenClawError;
pub use mcp::client::McpClient;
pub use mcp::protocol::{McpCallResult, McpContent, McpError, McpTool};
pub use mcp::transport::{ChildTransport, DuplexTransport, McpTransport};
pub use openclaw_runtime::OpenClawRuntime;
