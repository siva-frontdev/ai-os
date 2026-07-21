#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Core types, errors, events, and traits for the Intelligence Platform.
//!
//! This crate defines the shared abstractions used by all intelligence crates.

pub mod error;
pub mod event;
pub mod traits;
pub mod types;

pub use error::{ModelError, ModelResult};
pub use event::*;
pub use traits::{
    ChatStream, ContextAssembler, CostTracker, EmbeddingGenerator, IntelligenceCoordinator,
    ModelProvider, ModelRegistry, ModelRouter, PromptRenderer, ResponseCache, RoutingDecision,
    RoutingPolicy, SafetyEnforcer, StreamDecoder, TelemetrySink, ToolRegistry,
};
pub use types::*;
