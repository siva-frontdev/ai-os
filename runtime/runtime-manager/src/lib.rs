//! # AI-OS Runtime Manager
//!
//! The Runtime Manager sits between the Planner and individual runtimes.
//! The Planner never communicates directly with a runtime:
//!
//! ```text
//! Planner
//!    │
//!    ▼
//! Capability Registry
//!    │
//!    ▼
//! Runtime Manager ── register / initialize / dispatch / observe
//!    │
//!    ├── OpenClaw Runtime (MCP)
//!    ├── Mock Runtime
//!    └── any future runtime
//! ```
//!
//! Responsibilities:
//!
//! - **discover**: register runtimes
//! - **initialize**: initialize all registered runtimes
//! - **merge**: collect capabilities from all runtimes into one set,
//!   deduplicated by [`CapabilityId`]
//! - **select**: route an action to the runtime that claims its capability
//! - **dispatch**: execute actions without runtime-specific code in the
//!   Planner
//! - **collect**: aggregate observations and health from all runtimes
//!
//! The manager depends only on `ai-os-runtime-api`. It has no knowledge
//! of OpenClaw or any other runtime implementation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod manager;

pub use manager::RuntimeManager;
