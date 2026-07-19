//! # AI-OS Runtime Platform
//!
//! Async-first, event-driven runtime that depends on `ai-os-core`.
//!
//! ## Modules
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`state`]     | Runtime phase state machine |
//! | [`context`]   | Trace context propagation |
//! | [`permission`]| Authorization and permission checks |
//! | [`scheduler`] | Task scheduling and worker pools |
//! | [`session`]   | Session lifecycle management |
//! | [`task`]      | Task creation, lifecycle, tracking |
//! | [`supervisor`]| Fault tolerance and restart policies |
//! | [`resource`]  | Resource usage tracking and limits |
pub mod context;
pub mod error;
pub mod permission;
pub mod resource;
pub mod runtime;
pub mod scheduler;
pub mod session;
pub mod state;
pub mod supervisor;
pub mod task;

pub use error::RuntimeError;
pub use runtime::Runtime;
