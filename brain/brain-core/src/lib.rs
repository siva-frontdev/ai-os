//! # Brain Core
//!
//! Shared foundation for the Brain Platform (Layer 6).
//!
//! Defines all cross-crate types, events, contexts, budget primitives,
//! the `BrainState` FSM, tool contracts, and the `BrainResult` alias.
//!
//! ## Modules
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`ids`] | Newtype ID wrappers for all Brain entities |
//! | [`types`] | Shared enums and the `Confidence` newtype |
//! | [`events`] | Canonical event structs (implement [`ai_os_core::events::Event`]) |
//! | [`errors`] | [`BrainError`] and the [`BrainResult`] alias |
//! | [`context`] | Context structs carried through the cognitive loop |
//! | [`budget`] | [`CognitiveBudget`], [`BudgetUsage`], [`BudgetChecker`] trait |
//! | [`state`] | `BrainStateMachine` — validates platform-level FSM transitions |
//! | [`tool`] | Abstract tool contracts: [`ToolRequirement`], [`ToolCandidate`] |
//! | [`traits`] | Shared trait bounds and the [`BrainResult`] type alias |
//!
//! ## Crate Dependencies
//!
//! - `ai-os-core` — EventBus, Service, CoreError
//! - `memory-core` — `Timestamp`, `MemoryId`
//!
//! This crate must compile with zero dependencies on other Brain crates.

pub mod ids;
pub mod types;
pub mod events;
pub mod errors;
pub mod context;
pub mod budget;
pub mod state;
pub mod tool;
pub mod traits;

// Re-export the most commonly used types at the crate root.
pub use ids::{
    CheckpointId, GoalId, LessonId, PlanId, DecisionId, ReflectionId, ThoughtId,
    ToolCapabilityId, ToolId, WorkflowId,
};
pub use types::{
    BudgetDimension, BrainState, Confidence, GoalPriority, GoalStatus,
};
pub use errors::{BrainError, BrainResult};
pub use budget::{CognitiveBudget, BudgetUsage, BudgetChecker};
pub use context::{
    DecisionContext, GoalContext, PlanningContext, ReasoningContext,
};
pub use tool::{ExecutablePlan, ToolCandidate, ToolRequirement};
