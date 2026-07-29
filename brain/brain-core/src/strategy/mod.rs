use crate::ids::StrategyId;
use serde::{Deserialize, Serialize};

/// Describes how the Brain should approach achieving a goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyProfile {
    pub strategy_id: StrategyId,
    pub name: String,
    pub description: String,
    pub approach: ExecutionStrategy,
    pub risk_tolerance: f64,
    pub exploration_bias: f64,
    pub max_delegation_depth: u32,
    pub requires_human_approval: bool,
}

/// The execution approach for a strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    /// Direct execution via known tools
    Direct,
    /// Decompose into subtasks and execute sequentially
    Sequential,
    /// Decompose into parallel subtasks
    Parallel,
    /// Research first, then execute
    ResearchThenExecute,
    /// Iterative refinement with feedback loops
    Iterative,
    /// Delegate to a specialized agent
    Delegated,
    /// Escalate to human operator
    HumanAssisted,
}
