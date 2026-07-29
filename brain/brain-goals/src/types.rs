use brain_core::budget::CognitiveBudget;
use brain_core::ids::{GoalId, MilestoneId, ObjectiveId, StrategyId};
use brain_core::strategy::ExecutionStrategy;
use brain_core::types::{GoalPriority, GoalStatus};
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

/// A single objective within a goal decomposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectiveRecord {
    pub objective_id: ObjectiveId,
    pub goal_id: GoalId,
    pub description: String,
    pub order: u32,
    pub status: GoalStatus,
    pub milestones: Vec<MilestoneRecord>,
    pub created_at: Timestamp,
    pub completed_at: Option<Timestamp>,
}

/// A milestone within an objective, marking a significant checkpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MilestoneRecord {
    pub milestone_id: MilestoneId,
    pub objective_id: ObjectiveId,
    pub description: String,
    pub order: u32,
    pub status: GoalStatus,
    pub created_at: Timestamp,
    pub reached_at: Option<Timestamp>,
}

/// Result summary of a completed or failed goal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalOutcome {
    pub summary: String,
    pub success: bool,
    pub confidence: f32,
    pub artifacts: Vec<String>,
    pub errors: Vec<String>,
}

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
    pub version: u32,
    pub retry_count: u32,

    // Hierarchical decomposition
    pub objectives: Vec<ObjectiveRecord>,
    pub current_objective: Option<ObjectiveId>,

    // Strategy
    pub strategy_id: Option<StrategyId>,
    pub execution_approach: Option<ExecutionStrategy>,

    // Progress tracking
    pub progress_pct: f64,
    pub tags: Vec<String>,
    pub source: String,
    pub outcome: Option<GoalOutcome>,

    // Blocking
    pub blocked_reason: Option<String>,
    pub deferred_until: Option<Timestamp>,

    // Timing
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
    pub failure_count: u32,
    pub recovery_attempts: u32,
    pub last_error: Option<String>,
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
            version: 1,
            retry_count: 0,
            objectives: Vec::new(),
            current_objective: None,
            strategy_id: None,
            execution_approach: None,
            progress_pct: 0.0,
            tags: Vec::new(),
            source: "brain".into(),
            outcome: None,
            blocked_reason: None,
            deferred_until: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            failure_count: 0,
            recovery_attempts: 0,
            last_error: None,
            metadata: std::collections::HashMap::new(),
        }
    }
}
