use async_trait::async_trait;
pub use execution_core::error::ExecutionResult;
use execution_core::traits::{RecoveryAction, RecoveryManager};
use execution_core::types::{ExecutionPlan, ExecutionResult as EResult};
use execution_recovery::{DefaultRetryManager, DefaultRollbackManager};

use std::fmt;

/// Thin adapter implementing [`RecoveryManager`] by delegating to
/// [`DefaultRetryManager`] and [`DefaultRollbackManager`].
#[derive(Default)]
pub struct DefaultCoordinatorRecoveryManager {
    retry: DefaultRetryManager,
    rollback: DefaultRollbackManager,
}

impl fmt::Debug for DefaultCoordinatorRecoveryManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultCoordinatorRecoveryManager").finish()
    }
}

impl DefaultCoordinatorRecoveryManager {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RecoveryManager for DefaultCoordinatorRecoveryManager {
    async fn handle_failure(
        &self,
        result: &EResult,
        plan: &ExecutionPlan,
    ) -> ExecutionResult<RecoveryAction> {
        let policy = &plan.request.retry_policy;
        if self.retry.should_retry(result, policy, 0) {
            let retry_plan = self.retry.make_retry_plan(plan, 1).map_err(|e| {
                execution_core::error::ExecutionError::ConfigurationError(e.to_string())
            })?;
            let delay_ms = self.retry.compute_backoff(policy, 1).as_millis() as u64;
            return Ok(RecoveryAction::Retry {
                retry_plan,
                delay_ms,
            });
        }
        Ok(RecoveryAction::Ignore)
    }

    async fn handle_timeout(
        &self,
        result: &EResult,
        plan: &ExecutionPlan,
    ) -> ExecutionResult<RecoveryAction> {
        self.handle_failure(result, plan).await
    }

    async fn handle_cancellation(
        &self,
        _result: &EResult,
        _plan: &ExecutionPlan,
    ) -> ExecutionResult<RecoveryAction> {
        Ok(RecoveryAction::Ignore)
    }

    async fn execute_rollback(
        &self,
        plan: &ExecutionPlan,
        result: &EResult,
    ) -> ExecutionResult<()> {
        self.rollback
            .execute_rollback(plan, result)
            .map_err(|e| execution_core::error::ExecutionError::ConfigurationError(e.to_string()))
    }

    async fn retry(&self, plan: &ExecutionPlan) -> ExecutionResult<ExecutionPlan> {
        self.retry
            .make_retry_plan(plan, 1)
            .map_err(|e| execution_core::error::ExecutionError::ConfigurationError(e.to_string()))
    }
}
