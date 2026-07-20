use crate::errors::{WorkflowError, WorkflowResult};
use crate::types::{StepStatus, Workflow, WorkflowProgress, WorkflowStatus};
use crate::workflow_builder::WorkflowBuilder;
use brain_core::ids::WorkflowId;
use brain_core::tool::ExecutablePlan;
use memory_core::Timestamp;
use std::collections::HashMap;
use std::sync::RwLock;

pub struct WorkflowExecutor {
    workflows: RwLock<HashMap<WorkflowId, Workflow>>,
    builder: WorkflowBuilder,
}

impl WorkflowExecutor {
    pub fn new() -> Self {
        Self {
            workflows: RwLock::new(HashMap::new()),
            builder: WorkflowBuilder::new(),
        }
    }

    pub fn create_workflow(&self, name: &str, plan: ExecutablePlan, max_retries: u32) -> WorkflowResult<WorkflowId> {
        let workflow = self.builder.build(name, plan, max_retries);
        let id = workflow.workflow_id;
        let mut store = self.workflows.write().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        store.insert(id, workflow);
        Ok(id)
    }

    pub fn start(&self, workflow_id: &WorkflowId) -> WorkflowResult<()> {
        let mut store = self.workflows.write().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        let wf = store.get_mut(workflow_id).ok_or_else(|| WorkflowError::StepNotFound(format!("workflow {:?}", workflow_id)))?;
        if wf.status == WorkflowStatus::Completed {
            return Err(WorkflowError::AlreadyCompleted(wf.name.clone()));
        }
        wf.status = WorkflowStatus::Running;
        wf.started_at = Some(Timestamp::now());
        wf.updated_at = Timestamp::now();
        if !wf.steps.is_empty() {
            wf.steps[0].status = StepStatus::Running;
            wf.steps[0].started_at = Some(Timestamp::now());
        }
        Ok(())
    }

    pub fn complete_step(&self, workflow_id: &WorkflowId, step_index: usize, result: String) -> WorkflowResult<()> {
        let mut store = self.workflows.write().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        let wf = store.get_mut(workflow_id).ok_or_else(|| WorkflowError::StepNotFound(format!("workflow {:?}", workflow_id)))?;

        if step_index >= wf.steps.len() {
            return Err(WorkflowError::StepNotFound(format!("step {} out of range", step_index)));
        }

        let step = &mut wf.steps[step_index];
        step.status = StepStatus::Completed;
        step.result = Some(result);
        step.completed_at = Some(Timestamp::now());

        wf.current_step = step_index + 1;
        wf.updated_at = Timestamp::now();

        if wf.current_step >= wf.steps.len() {
            wf.status = WorkflowStatus::Completed;
            wf.completed_at = Some(Timestamp::now());
        } else {
            wf.steps[wf.current_step].status = StepStatus::Running;
            wf.steps[wf.current_step].started_at = Some(Timestamp::now());
        }

        Ok(())
    }

    pub fn fail_step(&self, workflow_id: &WorkflowId, step_index: usize, error: String) -> WorkflowResult<()> {
        let mut store = self.workflows.write().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        let wf = store.get_mut(workflow_id).ok_or_else(|| WorkflowError::StepNotFound(format!("workflow {:?}", workflow_id)))?;

        if step_index >= wf.steps.len() {
            return Err(WorkflowError::StepNotFound(format!("step {} out of range", step_index)));
        }

        let step = &mut wf.steps[step_index];
        step.retry_count += 1;

        if step.retry_count >= step.max_retries {
            step.status = StepStatus::Failed;
            step.error = Some(error.clone());
            wf.status = WorkflowStatus::Failed;
            wf.updated_at = Timestamp::now();
        } else {
            step.status = StepStatus::Running;
            step.started_at = Some(Timestamp::now());
        }

        Ok(())
    }

    pub fn pause(&self, workflow_id: &WorkflowId) -> WorkflowResult<()> {
        let mut store = self.workflows.write().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        let wf = store.get_mut(workflow_id).ok_or_else(|| WorkflowError::StepNotFound(format!("workflow {:?}", workflow_id)))?;
        wf.status = WorkflowStatus::Paused;
        wf.updated_at = Timestamp::now();
        Ok(())
    }

    pub fn resume(&self, workflow_id: &WorkflowId) -> WorkflowResult<()> {
        let mut store = self.workflows.write().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        let wf = store.get_mut(workflow_id).ok_or_else(|| WorkflowError::StepNotFound(format!("workflow {:?}", workflow_id)))?;
        wf.status = WorkflowStatus::Running;
        wf.updated_at = Timestamp::now();
        Ok(())
    }

    pub fn cancel(&self, workflow_id: &WorkflowId) -> WorkflowResult<()> {
        let mut store = self.workflows.write().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        let wf = store.get_mut(workflow_id).ok_or_else(|| WorkflowError::StepNotFound(format!("workflow {:?}", workflow_id)))?;
        wf.status = WorkflowStatus::Cancelled;
        wf.updated_at = Timestamp::now();
        Ok(())
    }

    pub fn get_workflow(&self, workflow_id: &WorkflowId) -> WorkflowResult<Workflow> {
        let store = self.workflows.read().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        store.get(workflow_id).cloned().ok_or_else(|| WorkflowError::StepNotFound(format!("workflow {:?}", workflow_id)))
    }

    pub fn get_progress(&self, workflow_id: &WorkflowId) -> WorkflowResult<WorkflowProgress> {
        let store = self.workflows.read().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        let wf = store.get(workflow_id).ok_or_else(|| WorkflowError::StepNotFound(format!("workflow {:?}", workflow_id)))?;

        let total = wf.steps.len() as u32;
        let completed = wf.steps.iter().filter(|s| s.status == StepStatus::Completed).count() as u32;
        let failed = wf.steps.iter().filter(|s| s.status == StepStatus::Failed).count() as u32;
        let skipped = wf.steps.iter().filter(|s| s.status == StepStatus::Skipped).count() as u32;
        let progress = if total == 0 { 1.0 } else { completed as f64 / total as f64 };

        Ok(WorkflowProgress {
            workflow_id: *workflow_id,
            total_steps: total,
            completed_steps: completed,
            failed_steps: failed,
            skipped_steps: skipped,
            progress_pct: progress,
            estimated_remaining: None,
        })
    }

    pub fn list_workflows(&self) -> WorkflowResult<Vec<Workflow>> {
        let store = self.workflows.read().map_err(|e| WorkflowError::Internal(e.to_string()))?;
        let mut list: Vec<Workflow> = store.values().cloned().collect();
        list.sort_by_key(|w| w.created_at);
        list.reverse();
        Ok(list)
    }
}

impl Default for WorkflowExecutor {
    fn default() -> Self { Self::new() }
}
