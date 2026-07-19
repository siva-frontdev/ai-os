#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # memory-working
//!
//! Bounded volatile store for active cognitive state in the AI-native OS Memory Platform.
//!
//! Working memory holds the AI's immediately accessible state — the information
//! it is currently attending to, reasoning about, or acting upon. It is the
//! functional equivalent of human short-term/working memory.
//!
//! ## Responsibilities
//!
//! - **Current Goal**: The active objective the AI is pursuing.
//! - **Active Task**: The current task being executed.
//! - **Current Plan**: The step-by-step plan for the current goal.
//! - **Current Step**: Which plan step is being executed.
//! - **Active Conversation**: Ongoing conversation context.
//! - **Active Applications**: Applications currently in focus.
//! - **Current Selection**: The currently selected/highlighted item.
//! - **Clipboard Snapshot**: Recent clipboard contents.
//! - **Temporary Reasoning State**: Intermediate reasoning data.
//! - **Scratchpad**: Temporary key-value store for reasoning intermediates.
//! - **Attention Queue**: Priority-ordered items the AI should focus on.
//!
//! ## Architecture
//!
//! This crate provides traits and default implementations for four subsystems:
//!
//! - [`WorkingMemory`] — Bounded LRU-evicted memory for active objects.
//! - [`WorkingMemorySession`] — Session-scoped working memory isolation.
//! - [`Scratchpad`] — Temporary key-value store for reasoning intermediates.
//! - [`AttentionManager`] — Priority queue for focus management.
//!
//! All implementations are in-memory, volatile, and bounded in capacity.

mod error;
mod event;

/// Bounded LRU-evicted volatile memory for active cognitive objects.
pub mod working_memory;

/// Session-scoped working memory isolation.
pub mod working_session;

/// Temporary key-value store for reasoning intermediates.
pub mod scratchpad;

/// Priority queue for managing the AI's focus.
pub mod attention;

// Re-export error types
pub use error::{WorkingMemoryError, WorkingMemoryResult};

// Re-export event types
pub use WorkingMemoryEvent::GoalChanged;
pub use WorkingMemoryEvent::ObjectExpired as WorkingMemoryExpired;
pub use WorkingMemoryEvent::ObjectUpdated as WorkingMemoryUpdated;
pub use WorkingMemoryEvent::TaskChanged;
pub use event::WorkingMemoryEvent;

// Re-export all trait types
pub use attention::AttentionManager;
pub use scratchpad::Scratchpad;
pub use working_memory::WorkingMemory;
pub use working_session::WorkingMemorySession;

// Re-export concrete implementations for convenience
pub use attention::PriorityAttentionManager;
pub use scratchpad::InMemoryScratchpad;
pub use working_memory::LruWorkingMemory;
pub use working_session::DefaultWorkingMemorySession;
