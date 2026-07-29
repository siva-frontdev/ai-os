use brain_core::ids::{GoalId, PlanId, WorkflowId};
use brain_core::tool::ExecutablePlan;
use brain_core::types::Confidence;
use brain_goals::types::GoalRecord;
use brain_workflow::Workflow;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CognitiveLoad(f64);

impl CognitiveLoad {
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }
    pub fn raw(&self) -> f64 {
        self.0
    }
}

impl Default for CognitiveLoad {
    fn default() -> Self {
        Self(0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BrainState {
    Idle,
    GoalSetting,
    Planning,
    Reasoning,
    DecisionMaking,
    Executing,
    Reflecting,
    Learning,
    Paused,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainSession {
    pub session_id: String,
    pub state: BrainState,
    pub current_goal: Option<GoalRecord>,
    pub current_plan: Option<ExecutablePlan>,
    pub current_workflow: Option<Workflow>,
    pub cognitive_load: CognitiveLoad,
    pub started_at: Timestamp,
    pub updated_at: Timestamp,
    pub step_count: u64,
    pub error_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinationReport {
    pub session_id: String,
    pub goal_id: Option<GoalId>,
    pub plan_id: Option<PlanId>,
    pub workflow_id: Option<WorkflowId>,
    pub state: BrainState,
    pub confidence: Confidence,
    pub summary: String,
    pub duration_ms: u64,
}

/// Outcome returned by [`BrainOrchestrator::process_user_input`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessResult {
    pub summary: String,
    pub reasoning_result: brain_core::types::ReasoningResult,
    pub goal_id: GoalId,
    pub plan_id: PlanId,
    pub workflow_id: Option<WorkflowId>,
    pub state: BrainState,
}
