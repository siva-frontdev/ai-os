//! `PlanningContext` — context passed to the Planner.
use crate::ids::GoalId;
use crate::budget::CognitiveBudget;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningContext {
    pub plan_id: String, // PlanId serialised
    pub goal_id: GoalId,
    pub task_graph: TaskGraphRef,
    pub resources: ResourceConstraints,
    pub alternatives: Vec<AlternativePlanRef>,
    pub budget: CognitiveBudget,
}

/// Reference to a task graph (lazy — owned by Planner).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskGraphRef {
    pub node_count: usize,
    pub edge_count: usize,
    pub entry_points: Vec<String>,
    pub exit_points: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceConstraints {
    pub max_parallel_steps: usize,
    pub max_retries_per_step: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlternativePlanRef {
    pub plan_id: String,
    pub estimated_cost: f64,
    pub estimated_duration_ms: u64,
    pub confidence: f64,
}
