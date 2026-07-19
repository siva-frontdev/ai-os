#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # memory-context
//!
//! Hierarchical execution context management for the AI-native OS Memory Platform.
//!
//! Maintains the complete execution context: runtime state, current workspace,
//! current directory, open windows, active processes, active user, environment
//! variables, platform information, and session metadata.
//!
//! ## Key Concepts
//!
//! - **Hierarchical contexts**: contexts form a tree. Child contexts inherit
//!   parent values and can override them (read-through inheritance).
//! - **Event-driven updates**: context values update automatically from Runtime
//!   and OSAL events via `ContextProvider`.
//! - **Snapshot/restore**: capture and restore complete context subtrees.
//! - **Merge**: merge values from one context into another.
//!
//! ## Thread Model
//!
//! All shared state is protected by `std::sync::RwLock<HashMap<...>>`.
//! Critical sections are short (single hash map lookups) and never held
//! across `.await` points.

mod error;
mod event;
pub mod context_manager;
pub mod context_provider;
pub mod context_snapshot;

pub use error::{ContextManagerError, ContextManagerResult};
pub use event::ContextEvent;
pub use context_manager::{ContextManager, DefaultContextManager, ContextManagerStats, TreeContextManager};
pub use context_provider::{ContextProvider, DefaultContextProvider, StaticContextProvider};
pub use context_snapshot::{ContextSnapshot, DefaultContextSnapshot, ContextSnapshotId};
