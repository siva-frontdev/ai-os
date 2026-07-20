//! Workflow events.
use crate::ids::{CheckpointId, WorkflowId};
use memory_core::Timestamp;
use ai_os_core::events::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStarted {
    pub workflow_id: WorkflowId,
    pub name: String,
    pub step_count: usize,
    pub composition: String, // "Sequential", "Parallel", "Conditional", "FanOut", "FanIn"
    pub started_at: Timestamp,
}

impl Event for WorkflowStarted {
    fn event_type(&self) -> &'static str { "brain.workflow.started" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStepStarted {
    pub workflow_id: WorkflowId,
    pub step_id: String, // UUID string
    pub step_name: String,
    pub started_at: Timestamp,
}

impl Event for WorkflowStepStarted {
    fn event_type(&self) -> &'static str { "brain.workflow.step.started" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStepCompleted {
    pub workflow_id: WorkflowId,
    pub step_id: String,
    pub outcome: String, // "Success", "Failed", "Skipped"
    pub duration_ms: u64,
    pub completed_at: Timestamp,
}

impl Event for WorkflowStepCompleted {
    fn event_type(&self) -> &'static str { "brain.workflow.step.completed" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowCheckpointed {
    pub workflow_id: WorkflowId,
    pub checkpoint_id: CheckpointId,
    pub step_index: usize,
    pub total_steps: usize,
    pub size_bytes: u32,
    pub checkpointed_at: Timestamp,
}

impl Event for WorkflowCheckpointed {
    fn event_type(&self) -> &'static str { "brain.workflow.checkpointed" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowCompleted {
    pub workflow_id: WorkflowId,
    pub total_duration_ms: u64,
    pub steps_completed: usize,
    pub steps_failed: usize,
    pub completed_at: Timestamp,
}

impl Event for WorkflowCompleted {
    fn event_type(&self) -> &'static str { "brain.workflow.completed" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowFailed {
    pub workflow_id: WorkflowId,
    pub failed_step_id: Option<String>,
    pub error: String,
    pub recoverable: bool,
    pub failed_at: Timestamp,
}

impl Event for WorkflowFailed {
    fn event_type(&self) -> &'static str { "brain.workflow.failed" }
}
