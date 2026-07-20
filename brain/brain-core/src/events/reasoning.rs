//! Reasoning events.
use crate::ids::{GoalId, ThoughtId};
use crate::types::BudgetDimension;
use crate::types::Confidence;
use ai_os_core::events::Event;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

/// Published when a reasoning cycle starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningStarted {
    pub thought_id: ThoughtId,
    pub goal_id: GoalId,
    pub reasoning_depth: usize,
    pub hypotheses_requested: usize,
    pub started_at: Timestamp,
}

impl Event for ReasoningStarted {
    fn event_type(&self) -> &'static str {
        "brain.reasoning.started"
    }
}

/// Published when a reasoning cycle finishes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningFinished {
    pub thought_id: ThoughtId,
    pub selected_hypothesis_id: Option<String>,
    pub confidence: Confidence,
    pub depth_reached: usize,
    pub model_calls_used: usize,
    pub tokens_consumed: u32,
    pub finished_at: Timestamp,
}

impl Event for ReasoningFinished {
    fn event_type(&self) -> &'static str {
        "brain.reasoning.finished"
    }
}

/// Published when the budget is exhausted mid-cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetExhausted {
    pub budget_dimension: BudgetDimension,
    pub phase: String, // which BrainState was active
    pub usage_at_exhaustion: BudgetUsageSummary,
    pub exhausted_at: Timestamp,
}

impl Event for BudgetExhausted {
    fn event_type(&self) -> &'static str {
        "brain.budget.exhausted"
    }
}

/// Snapshot of budget usage at exhaustion time (for the event payload).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetUsageSummary {
    pub iterations_used: usize,
    pub reasoning_depth_reached: usize,
    pub branches_spawned: usize,
    pub model_calls_used: usize,
    pub tokens_consumed: u32,
    pub cost_incurred_cents: u64,
}

/// Published when a single hypothesis is generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningHypothesisGenerated {
    pub thought_id: ThoughtId,
    pub hypothesis_id: String,
    pub confidence: Confidence,
    pub generated_at: Timestamp,
}

impl Event for ReasoningHypothesisGenerated {
    fn event_type(&self) -> &'static str {
        "brain.reasoning.hypothesis.generated"
    }
}
