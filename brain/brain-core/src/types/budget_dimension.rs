//! `BudgetDimension` — the resource tracked by [`CognitiveBudget`](::budget::CognitiveBudget).
use serde::{Deserialize, Serialize};

/// Which budget dimension has been exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BudgetDimension {
    WallTime,
    Iterations,
    ReasoningDepth,
    Branches,
    ModelCalls,
    Tokens,
    CostCents,
    Deadline,
}

impl BudgetDimension {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::WallTime => "wall_time",
            Self::Iterations => "iterations",
            Self::ReasoningDepth => "reasoning_depth",
            Self::Branches => "branches",
            Self::ModelCalls => "model_calls",
            Self::Tokens => "tokens",
            Self::CostCents => "cost_cents",
            Self::Deadline => "deadline",
        }
    }
}

impl std::fmt::Display for BudgetDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
