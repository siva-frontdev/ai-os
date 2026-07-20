use brain_core::budget::CognitiveBudget;
use brain_core::ids::GoalId;
use brain_core::types::{GoalPriority, GoalStatus};
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalRecord {
    pub goal_id: GoalId,
    pub goal_type: String,
    pub description: String,
    pub priority: GoalPriority,
    pub status: GoalStatus,
    pub parent: Option<GoalId>,
    pub children: Vec<GoalId>,
    pub dependencies: Vec<GoalId>,
    pub dependents: Vec<GoalId>,
    pub budget: CognitiveBudget,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub completed_at: Option<Timestamp>,
    pub failure_count: u32,
    pub recovery_attempts: u32,
    pub metadata: std::collections::HashMap<String, String>,
}

impl GoalRecord {
    pub fn new(
        goal_id: GoalId,
        goal_type: impl Into<String>,
        description: impl Into<String>,
        priority: GoalPriority,
    ) -> Self {
        let now = Timestamp::now();
        Self {
            goal_id,
            goal_type: goal_type.into(),
            description: description.into(),
            priority,
            status: GoalStatus::Pending,
            parent: None,
            children: Vec::new(),
            dependencies: Vec::new(),
            dependents: Vec::new(),
            budget: CognitiveBudget::default(),
            created_at: now,
            updated_at: now,
            completed_at: None,
            failure_count: 0,
            recovery_attempts: 0,
            metadata: std::collections::HashMap::new(),
        }
    }
}
