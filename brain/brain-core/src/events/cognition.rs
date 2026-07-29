use crate::strategy::ExecutionStrategy;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

use ai_os_core::events::Event;

/// Published when the cognitive loop completes one full iteration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveLoopTick {
    pub iteration: u64,
    pub goals_active: usize,
    pub goals_completed: usize,
    pub goals_failed: usize,
    pub ticked_at: Timestamp,
}

impl Event for CognitiveLoopTick {
    fn event_type(&self) -> &'static str {
        "brain.cognition.loop.tick"
    }
}

/// Published when the Brain selects a new strategy for a goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategySelected {
    pub goal_id: crate::ids::GoalId,
    pub strategy_id: crate::ids::StrategyId,
    pub approach: ExecutionStrategy,
    pub selected_at: Timestamp,
}

impl Event for StrategySelected {
    fn event_type(&self) -> &'static str {
        "brain.cognition.strategy.selected"
    }
}

/// Published when execution is delegated to an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalDelegated {
    pub goal_id: crate::ids::GoalId,
    pub agent_id: crate::ids::AgentId,
    pub agent_name: String,
    pub delegated_at: Timestamp,
}

impl Event for GoalDelegated {
    fn event_type(&self) -> &'static str {
        "brain.cognition.delegation.sent"
    }
}

/// Published when a delegated task returns a result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegationCompleted {
    pub goal_id: crate::ids::GoalId,
    pub agent_id: crate::ids::AgentId,
    pub success: bool,
    pub outcome: String,
    pub completed_at: Timestamp,
}

impl Event for DelegationCompleted {
    fn event_type(&self) -> &'static str {
        "brain.cognition.delegation.completed"
    }
}

/// Published when a delegation fails or times out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DelegationFailed {
    pub goal_id: crate::ids::GoalId,
    pub agent_id: crate::ids::AgentId,
    pub error: String,
    pub failed_at: Timestamp,
}

impl Event for DelegationFailed {
    fn event_type(&self) -> &'static str {
        "brain.cognition.delegation.failed"
    }
}
