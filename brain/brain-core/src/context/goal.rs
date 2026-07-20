//! `GoalContext` — context for a single goal.
use crate::budget::CognitiveBudget;
use crate::ids::GoalId;
use crate::types::GoalStatus;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalContext {
    pub goal_id: GoalId,
    pub parent: Option<GoalId>,
    pub children: Vec<GoalId>,
    pub dependencies: Vec<GoalId>,
    pub state: GoalStatus,
    pub budget: CognitiveBudget,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
