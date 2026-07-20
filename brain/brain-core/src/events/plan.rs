//! Planning events.
use crate::ids::{GoalId, PlanId};
use ai_os_core::events::Event;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

/// Published when a new plan is generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanCreated {
    pub plan_id: PlanId,
    pub goal_id: GoalId,
    pub step_count: usize,
    pub tool_requirements: Vec<String>, // ToolRequirement IDs as strings
    pub estimated_duration_ms: u64,
    pub estimated_cost_cents: u64,
    pub strategy_used: String,
    pub created_at: Timestamp,
}

impl Event for PlanCreated {
    fn event_type(&self) -> &'static str {
        "brain.plan.created"
    }
}

/// Published when a plan is modified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanUpdated {
    pub plan_id: PlanId,
    pub changes: String,
    pub reason: String,
    pub updated_at: Timestamp,
}

impl Event for PlanUpdated {
    fn event_type(&self) -> &'static str {
        "brain.plan.updated"
    }
}

/// Published when the PolicyEngine rejects a plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanRejected {
    pub plan_id: PlanId,
    pub goal_id: GoalId,
    pub violated_rules: Vec<String>, // Rule names
    pub rejected_at: Timestamp,
}

impl Event for PlanRejected {
    fn event_type(&self) -> &'static str {
        "brain.plan.rejected"
    }
}
