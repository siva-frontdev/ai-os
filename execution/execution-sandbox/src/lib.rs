//! # execution-sandbox
//!
//! Sandbox profile management, permission validation, capability
//! validation, and execution constraints for the Execution Platform.

#![forbid(unsafe_code)]

pub mod enforcer;
pub mod error;

pub use enforcer::{DefaultSandboxEnforcer, InMemoryProfileStore};
pub use error::{SandboxError, SandboxResult};
