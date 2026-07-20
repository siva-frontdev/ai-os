//! # Brain Policy
//!
//! Isolated policy engine for the Brain Platform.
//!
//! All policy logic lives here: rule loading, compilation, evaluation,
//! hot-reload, and guard-rail enforcement. This crate depends **only**
//! on `brain-core` — no dependency on any other Brain crate.
//!
//! ## Modules
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`types`] | Policy data types: `PolicyRule`, `RuleSet`, `GuardRail`, `PolicyEvaluation`, etc. |
//! | [`errors`] | `PolicyError` and `PolicyResult` |
//! | [`traits`] | `PolicyStore`, `PolicyEngine`, `PolicyEvaluator`, `PolicyCompiler`, `PolicyLoader`, `GuardRails` |
//! | [`engine`] | `InMemoryPolicyEngine` — default implementation |
//!
//! ## Design decisions
//!
//! * **Trait-first**: all behaviour is defined by traits; implementations are pluggable.
//! * **Data-driven**: policy rules are JSON-serializable; operators can author them without recompiling.
//! * **Hot-reload**: `PolicyLoader::watch_for_changes` returns a channel that emits `PolicyChange` events.
//! * **No filesystem watcher in Stage 1**: the watcher channel is implemented but filesystem polling
//!   is deferred to Stage 2. In Stage 1, reload is triggered programmatically.

pub mod errors;
pub mod traits;
pub mod types;

pub use errors::{PolicyError, PolicyResult};
pub use traits::{
    GuardRails, PolicyCompiler, PolicyEngine, PolicyEvaluator, PolicyLoader,
    PolicyStore,
};
pub use types::{CompiledRuleSet, PolicyChange};
pub use types::{
    Comparison, CompiledRule, EscalationChannel, GuardRail, LogLevel, PolicyConfig,
    PolicyEvaluation, PolicyRule, RuleAction, RuleCondition, RuleSet,
    ViolatedRule, ViolationSeverity,
};
