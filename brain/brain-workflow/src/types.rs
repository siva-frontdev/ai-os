use brain_core::ids::WorkflowId;
use brain_core::tool::ExecutablePlan;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub step_id: String,
    pub name: String,
    pub status: StepStatus,
    pub retry_count: u32,
    pub max_retries: u32,
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
    pub parallel_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    pub workflow_id: WorkflowId,
    pub name: String,
    pub plan: ExecutablePlan,
    pub steps: Vec<WorkflowStep>,
    pub status: WorkflowStatus,
    pub current_step: usize,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStage {
    pub stage_id: String,
    pub name: String,
    pub step_ids: Vec<String>,
    pub parallel: bool,
    pub status: StepStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowProgress {
    pub workflow_id: WorkflowId,
    pub total_steps: u32,
    pub completed_steps: u32,
    pub failed_steps: u32,
    pub skipped_steps: u32,
    pub progress_pct: f64,
    pub estimated_remaining: Option<std::time::Duration>,
}
