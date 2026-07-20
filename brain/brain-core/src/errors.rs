//! `BrainError` — unified error type for the Brain Platform.
use thiserror::Error;

/// Unified error type for the Brain Platform.
///
/// Every Brain crate converts its specific failures into this enum,
/// giving callers a single error type to handle.
#[derive(Debug, Error)]
pub enum BrainError {
    // ── Goal errors ─────────────────────────────────────────
    #[error("goal {0} not found")]
    GoalNotFound(crate::ids::GoalId),

    #[error("plan {0} not found")]
    PlanNotFound(crate::ids::PlanId),

    #[error("decision {0} not found")]
    DecisionNotFound(crate::ids::DecisionId),

    #[error("workflow {0} not found")]
    WorkflowNotFound(crate::ids::WorkflowId),

    // ── State errors ────────────────────────────────────────
    #[error("invalid brain state transition: {from:?} -> {to:?}")]
    InvalidStateTransition {
        from: crate::types::BrainState,
        to: crate::types::BrainState,
    },

    // ── Dependency errors ───────────────────────────────────
    #[error("cycle detected in goal dependencies: {0:?}")]
    CycleDetected(Vec<crate::ids::GoalId>),

    #[error("dependency violation: {blocker} blocks {blocked}")]
    DependencyViolation {
        blocker: crate::ids::GoalId,
        blocked: crate::ids::GoalId,
    },

    // ── Policy errors ───────────────────────────────────────
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("policy violation: {0}")]
    PolicyViolation(String),

    // ── Decision errors ─────────────────────────────────────
    #[error("conflict: {0}")]
    Conflict(String),

    // ── Budget errors ───────────────────────────────────────
    #[error("budget exhausted: {0}")]
    BudgetExhausted(crate::types::BudgetDimension),

    // ── Model errors ────────────────────────────────────────
    #[error("model provider error: {0}")]
    ModelProviderError(String),

    #[error("no model available for capability: {0:?}")]
    NoModelAvailable(String),

    // ── Tool errors ─────────────────────────────────────────
    #[error("tool not found: capability {0}")]
    ToolNotFound(crate::ids::ToolCapabilityId),

    // ── Memory errors ───────────────────────────────────────
    #[error("memory platform error: {0}")]
    MemoryError(String),

    // ── Validation ──────────────────────────────────────────
    #[error("invalid input: {0}")]
    ValidationError(String),

    // ── Internal ────────────────────────────────────────────
    #[error("internal error: {0}")]
    Internal(String),

    #[error("lock poisoned: {0}")]
    LockPoisoned(String),
}

impl BrainError {
    /// Convenience constructor for internal/shared errors.
    pub fn internal<T: Into<String>>(msg: T) -> Self {
        Self::Internal(msg.into())
    }
}

/// Result type alias used throughout the Brain Platform.
pub type BrainResult<T> = Result<T, BrainError>;
