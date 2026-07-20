use async_trait::async_trait;
use execution_core::{
    ExecutionError, ExecutionId, ExecutionPlan, ExecutionResult, Runner, RunnerOutput,
};

#[derive(Debug, Clone)]
pub struct DefaultContainerRunner;

#[async_trait]
impl Runner for DefaultContainerRunner {
    async fn execute(&self, _plan: &ExecutionPlan) -> ExecutionResult<RunnerOutput> {
        Err(ExecutionError::BackendUnavailable(
            "Container execution not yet implemented".into(),
        ))
    }

    async fn cancel(&self, _execution_id: &ExecutionId) -> ExecutionResult<()> {
        Err(ExecutionError::BackendUnavailable(
            "Container execution not yet implemented".into(),
        ))
    }

    async fn health(&self) -> ExecutionResult<()> {
        Err(ExecutionError::BackendUnavailable(
            "Container execution not yet implemented".into(),
        ))
    }

    fn backend_type(&self) -> &'static str {
        "container"
    }
}
