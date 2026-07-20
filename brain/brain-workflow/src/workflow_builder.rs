use crate::errors::{WorkflowError, WorkflowResult};
use crate::types::{StepStatus, Workflow, WorkflowStatus, WorkflowStep};
use brain_core::ids::WorkflowId;
use brain_core::tool::ExecutablePlan;
use memory_core::Timestamp;

pub struct WorkflowBuilder;

impl WorkflowBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, name: &str, plan: ExecutablePlan, max_retries: u32) -> Workflow {
        let steps: Vec<WorkflowStep> = plan
            .tool_requirements
            .iter()
            .enumerate()
            .map(|(i, _)| WorkflowStep {
                step_id: format!("step-{}", i),
                name: format!("step-{}", i),
                status: StepStatus::Pending,
                retry_count: 0,
                max_retries,
                result: None,
                error: None,
                started_at: None,
                completed_at: None,
                parallel_group: if i % 2 == 0 {
                    Some("group-a".into())
                } else {
                    Some("group-b".into())
                },
            })
            .collect();

        Workflow {
            workflow_id: WorkflowId::new(),
            name: name.into(),
            plan,
            steps,
            status: WorkflowStatus::Pending,
            current_step: 0,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn build_with_steps(
        &self,
        name: &str,
        plan: ExecutablePlan,
        step_configs: Vec<StepConfig>,
    ) -> WorkflowResult<Workflow> {
        if plan.tool_requirements.len() != step_configs.len() {
            return Err(WorkflowError::Internal(format!(
                "plan has {} requirements but {} configs provided",
                plan.tool_requirements.len(),
                step_configs.len()
            )));
        }

        let steps: Vec<WorkflowStep> = plan
            .tool_requirements
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let config = &step_configs[i];
                WorkflowStep {
                    step_id: format!("step-{}", i),
                    name: format!("step-{}", i),
                    status: StepStatus::Pending,
                    retry_count: 0,
                    max_retries: config.max_retries,
                    result: None,
                    error: None,
                    started_at: None,
                    completed_at: None,
                    parallel_group: config.parallel_group.clone(),
                }
            })
            .collect();

        Ok(Workflow {
            workflow_id: WorkflowId::new(),
            name: name.into(),
            plan,
            steps,
            status: WorkflowStatus::Pending,
            current_step: 0,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            started_at: None,
            completed_at: None,
        })
    }
}

impl Default for WorkflowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct StepConfig {
    pub max_retries: u32,
    pub parallel_group: Option<String>,
}
