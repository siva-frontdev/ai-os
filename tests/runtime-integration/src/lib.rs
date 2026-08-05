//! Integration test crate for the Runtime layer.
//!
//! Cross-crate tests proving:
//!
//! - **runtime registration** (Runtime Manager → OpenClaw Runtime →
//!   capabilities discovered via MCP)
//! - **capability dispatch** (Action → Runtime Manager → OpenClaw Runtime
//!   → Mock MCP → success)
//! - **runtime swap** (Planner → Runtime Manager → Mock Runtime, with zero
//!   planner changes)
//! - **email flow** (Telegram → Planner → email.send → Runtime Manager →
//!   OpenClaw Runtime → Mock Email Plugin → success)
//!
//! The AI-OS cognitive core is never modified by any of these tests; the
//! brain only ever sees the vendor-neutral [`Runtime`](ai_os_runtime_api::Runtime).

/// Shared test harness (fake coordinator, mock runtime, OpenClaw wiring).
pub mod common;
