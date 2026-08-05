//! # AI-OS Runtime Bootstrap
//!
//! The platform's runtime wiring point (Phase 1 of Runtime Validation).
//!
//! Cognition must never construct or address a runtime directly — it plans
//! against capabilities. This crate owns the only place a concrete runtime
//! is created: it turns configuration into a [`RuntimeManager`] with all
//! runtime instances registered and initialized, then hands the manager to
//! the rest of the platform. Replacing the execution substrate (OpenClaw,
//! a native runtime, ...) requires changes only in this crate.
//!
//! ## Startup flow
//!
//! ```text
//! RuntimeBootstrapConfig  (env or code)
//!   → RuntimeBootstrap::new(config)      register runtimes
//!   → RuntimeBootstrap::initialize()     handshake + capability discovery
//!   → StartupSummary                     runtimes, capabilities, health
//!   → bootstrap.manager()                Arc-free &RuntimeManager for the app
//! ```
//!
//! ## Environment configuration
//!
//! [`RuntimeBootstrapConfig::from_env`] reads:
//!
//! | Variable | Default |
//! |---|---|
//! | `AI_OS_RUNTIME_ENABLED` | `0` (empty bootstrap) |
//! | `AI_OS_MCP_SERVER_BIN` | `ai-os-mcp-server` |
//! | `AI_OS_MCP_PLUGINS` | `email,filesystem,github,calendar,telegram` |
//! | `AI_OS_FILESYSTEM_ROOT` | `.` |
//!
//! ## Out of scope
//!
//! The bootstrap never touches the brain. Feeding runtime observations
//! into the Cognitive Loop is done by the application (or tests) through an
//! `ObservationSource` bridge that holds the manager — the same pattern the
//! desktop companion uses for its other sources.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod bootstrap;
pub mod config;
pub mod error;

pub use bootstrap::{RuntimeBootstrap, StartupSummary};
pub use config::{McpRuntimeConfig, RuntimeBootstrapConfig, RuntimeInstanceConfig};
pub use error::BootstrapError;
