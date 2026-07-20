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

pub mod budget;
pub mod context;
pub mod errors;
pub mod events;
pub mod ids;
pub mod state;
pub mod tool;
pub mod traits;
pub mod types;

// Re-export the most commonly used types at the crate root.
pub use budget::{BudgetChecker, BudgetUsage, CognitiveBudget};
pub use context::{DecisionContext, GoalContext, PlanningContext, ReasoningContext};
pub use errors::{BrainError, BrainResult};
pub use ids::{
    CheckpointId, DecisionId, GoalId, LessonId, PlanId, ReflectionId, ThoughtId, ToolCapabilityId,
    ToolId, WorkflowId,
};
pub use tool::{ExecutablePlan, ToolCandidate, ToolRequirement};
pub use types::{BrainState, BudgetDimension, Confidence, GoalPriority, GoalStatus};
