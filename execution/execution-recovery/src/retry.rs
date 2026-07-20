use execution_core::types::{
    ExecutionId, ExecutionPlan, ExecutionResult as EResult, ExecutionState, RetryPolicy,
};
use rand::Rng;
use std::time::Duration;

use crate::error::RecoveryResult;

pub trait RetryManager: std::fmt::Debug + Send + Sync {
    fn should_retry(
        &self,
        result: &EResult,
        policy: &RetryPolicy,
        attempt: u32,
    ) -> RecoveryResult<bool>;

    fn compute_backoff(&self, policy: &RetryPolicy, attempt: u32) -> Duration;

    fn make_retry_plan(
        &self,
        original_plan: &ExecutionPlan,
        attempt: u32,
    ) -> RecoveryResult<ExecutionPlan>;
}

pub enum RetryDecision {
    Retry { delay_ms: u64 },
    Rollback,
    Escalate { reason: String },
    GiveUp,
}

#[derive(Debug, Default)]
pub struct DefaultRetryManager;

impl DefaultRetryManager {
    pub fn new() -> Self {
        Self
    }

    pub fn should_retry(
        &self,
        result: &execution_core::types::ExecutionResult,
        policy: &RetryPolicy,
        attempt: u32,
    ) -> bool {
        if attempt >= policy.max_retries {
            return false;
        }
        match result.state {
            ExecutionState::Failed | ExecutionState::TimedOut => true,
            ExecutionState::Cancelled => false,
            ExecutionState::Completed | ExecutionState::RolledBack | ExecutionState::Escalated => {
                false
            }
            _ => false,
        }
    }

    pub fn compute_backoff(&self, policy: &RetryPolicy, attempt: u32) -> Duration {
        let base = policy.base_delay_ms;
        let multiplier = policy.multiplier;
        let max_delay = policy.max_delay_ms;

        if attempt == 0 || base == 0 {
            return Duration::from_millis(0);
        }

        let exponent = (attempt - 1) as f64;
        let delay_ms = (base as f64 * multiplier.powf(exponent)).min(max_delay as f64) as u64;

        let mut rng = rand::thread_rng();
        let jitter_factor: f64 = rng.gen_range(0.5..=1.5);
        let jittered_ms = (delay_ms as f64 * jitter_factor) as u64;

        Duration::from_millis(jittered_ms)
    }

    pub fn make_retry_plan(
        &self,
        original_plan: &ExecutionPlan,
        _attempt: u32,
    ) -> RecoveryResult<ExecutionPlan> {
        let mut plan = original_plan.clone();
        plan.id = ExecutionId::new();
        plan.state = ExecutionState::Planned;
        plan.created_at = memory_core::Timestamp::now();
        Ok(plan)
    }
}
