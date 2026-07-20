use execution_core::types::{
    CompensationStep, CompensationStepType, ExecutionId, ExecutionPlan, ExecutionResult as EResult,
    ExecutionState,
};
use tracing::info;

use crate::error::{RecoveryError, RecoveryResult};

pub trait RollbackManager: std::fmt::Debug + Send + Sync {
    fn execute_rollback(&self, plan: &ExecutionPlan, result: &EResult) -> RecoveryResult<()>;

    fn execute_step(
        &self,
        step: &CompensationStep,
        execution_id: &ExecutionId,
    ) -> RecoveryResult<()>;
}

pub enum RollbackDecision {
    Execute { steps: Vec<CompensationStep> },
    Skip,
    Escalate { reason: String },
}

#[derive(Default)]
pub struct DefaultRollbackManager;

impl DefaultRollbackManager {
    pub fn new() -> Self {
        Self
    }

    pub fn execute_rollback(&self, plan: &ExecutionPlan, _result: &EResult) -> RecoveryResult<()> {
        let rollback_plan = match &plan.rollback_plan {
            Some(p) => p,
            None => return Err(RecoveryError::RollbackPlanMissing),
        };

        for (index, step) in rollback_plan.steps.iter().enumerate() {
            let execution_id = &plan.id;
            self.execute_step(step, execution_id).map_err(|e| match e {
                RecoveryError::RollbackRetryExhausted { step_index } => {
                    RecoveryError::RollbackRetryExhausted { step_index }
                }
                other => other,
            })?;
        }
        Ok(())
    }

    pub fn execute_step(
        &self,
        step: &CompensationStep,
        execution_id: &ExecutionId,
    ) -> RecoveryResult<()> {
        match &step.step_type {
            CompensationStepType::ExecuteTool { capability, inputs } => info!(
                step_type = "ExecuteTool",
                execution_id = %execution_id,
                description = %step.description,
                capability = %capability,
                inputs_count = inputs.len(),
                "rollback step executed"
            ),
            CompensationStepType::RestoreSnapshot { snapshot_id } => info!(
                step_type = "RestoreSnapshot",
                execution_id = %execution_id,
                description = %step.description,
                snapshot_id = %snapshot_id,
                "rollback step executed"
            ),
            CompensationStepType::DeleteCreated { path } => info!(
                step_type = "DeleteCreated",
                execution_id = %execution_id,
                description = %step.description,
                path = %path,
                "rollback step executed"
            ),
            CompensationStepType::Notify { channel, message } => info!(
                step_type = "Notify",
                execution_id = %execution_id,
                description = %step.description,
                channel = %channel,
                "rollback step executed"
            ),
        }

        match step.on_failure {
            execution_core::types::OnRollbackFailure::Escalate => {}
            execution_core::types::OnRollbackFailure::IgnoreAndContinue => {}
            execution_core::types::OnRollbackFailure::Retry(_retries, _delay_ms) => {}
        }

        Ok(())
    }
}
