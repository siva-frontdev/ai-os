//! `CognitiveBudget` and `BudgetUsage` — resource accounting for a reasoning cycle.
use crate::types::BudgetDimension;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

/// Resource budget allocated to a single reasoning cycle.
///
/// Every tick of the cognitive loop receives a `CognitiveBudget` before
/// entering the loop. Budget consumption is tracked in `BudgetUsage` and
/// checked at each phase boundary. Exhaustion of any budget dimension
/// triggers graceful degradation or pause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveBudget {
    /// Maximum wall-clock time for the entire cycle (nanoseconds).
    pub max_thinking_time_ns: u64,
    /// Maximum loop iterations for this goal.
    pub max_iterations: usize,
    /// Maximum depth of reasoning: steps in chain-of-thought.
    pub max_reasoning_depth: usize,
    /// Maximum parallel branches in reasoning.
    pub max_branches: usize,
    /// Maximum model invocations per cycle.
    pub max_model_calls: usize,
    /// Maximum tokens to consume across all model calls.
    pub max_token_budget: u32,
    /// Maximum cost (USD cents) for this cycle.
    pub max_cost_budget_cents: u64,
    /// Absolute deadline. Cycle must complete before this timestamp.
    pub deadline: Timestamp,
    /// Relative priority of this budget (higher = more resources).
    pub priority: u8,
}

impl CognitiveBudget {
    /// Create a new `CognitiveBudget` with all fields specified.
    pub fn new(
        max_thinking_time_ns: u64,
        max_iterations: usize,
        max_reasoning_depth: usize,
        max_branches: usize,
        max_model_calls: usize,
        max_token_budget: u32,
        max_cost_budget_cents: u64,
        deadline: Timestamp,
        priority: u8,
    ) -> Self {
        Self {
            max_thinking_time_ns,
            max_iterations,
            max_reasoning_depth,
            max_branches,
            max_model_calls,
            max_token_budget,
            max_cost_budget_cents,
            deadline,
            priority,
        }
    }

    /// Return whether any resource has been exhausted given current usage.
    pub fn exhausted(&self, usage: &BudgetUsage) -> bool {
        usage.iterations_used >= self.max_iterations
            || usage.reasoning_depth_reached >= self.max_reasoning_depth
            || usage.branches_spawned >= self.max_branches
            || usage.model_calls_used >= self.max_model_calls
            || usage.tokens_consumed >= self.max_token_budget
            || usage.cost_incurred_cents >= self.max_cost_budget_cents
    }

    /// Return the remaining token budget.
    pub fn remaining_tokens(&self, usage: &BudgetUsage) -> u32 {
        self.max_token_budget.saturating_sub(usage.tokens_consumed)
    }

    /// Return the remaining cost budget in cents.
    pub fn remaining_cost_cents(&self, usage: &BudgetUsage) -> u64 {
        self.max_cost_budget_cents
            .saturating_sub(usage.cost_incurred_cents)
    }

    /// Return true if the current time has passed the deadline.
    pub fn past_deadline(&self, now: Timestamp) -> bool {
        now > self.deadline
    }

    /// Produce a degraded copy of this budget (reduced scope for retry).
    ///
    /// Branches, depth, and model calls are halved (minimum 1).
    pub fn degraded(&self) -> Self {
        Self {
            max_reasoning_depth: (self.max_reasoning_depth / 2).max(1),
            max_branches: (self.max_branches / 2).max(1),
            max_model_calls: (self.max_model_calls / 2).max(1),
            max_token_budget: self.max_token_budget / 2,
            max_cost_budget_cents: self.max_cost_budget_cents / 2,
            ..*self
        }
    }

    /// Check whether a specific dimension is exhausted.
    pub fn dimension_exhausted(&self, usage: &BudgetUsage, dim: BudgetDimension) -> bool {
        match dim {
            BudgetDimension::WallTime => {
                let now = Timestamp::now();
                now.as_nanos().saturating_sub(usage.started_at.as_nanos())
                    >= self.max_thinking_time_ns as i64
            }
            BudgetDimension::Iterations => usage.iterations_used >= self.max_iterations,
            BudgetDimension::ReasoningDepth => {
                usage.reasoning_depth_reached >= self.max_reasoning_depth
            }
            BudgetDimension::Branches => usage.branches_spawned >= self.max_branches,
            BudgetDimension::ModelCalls => usage.model_calls_used >= self.max_model_calls,
            BudgetDimension::Tokens => usage.tokens_consumed >= self.max_token_budget,
            BudgetDimension::CostCents => usage.cost_incurred_cents >= self.max_cost_budget_cents,
            BudgetDimension::Deadline => Timestamp::now() > self.deadline,
        }
    }
}

impl Default for CognitiveBudget {
    /// Conservative default budget (suitable for unit tests).
    fn default() -> Self {
        let deadline = Timestamp::now();
        Self {
            max_thinking_time_ns: 30_000_000_000, // 30 s
            max_iterations: 10,
            max_reasoning_depth: 5,
            max_branches: 3,
            max_model_calls: 5,
            max_token_budget: 8192,
            max_cost_budget_cents: 50,
            deadline,
            priority: 1,
        }
    }
}

/// Tracks actual consumption of a `CognitiveBudget` during a reasoning cycle.
///
/// Updated synchronously within a tick (no `await` while holding the lock).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BudgetUsage {
    /// Number of loop iterations consumed.
    pub iterations_used: usize,
    /// Maximum reasoning depth reached this cycle.
    pub reasoning_depth_reached: usize,
    /// Number of parallel branches spawned.
    pub branches_spawned: usize,
    /// Number of model calls made this cycle.
    pub model_calls_used: usize,
    /// Total tokens consumed across all model calls.
    pub tokens_consumed: u32,
    /// Total cost incurred (USD cents).
    pub cost_incurred_cents: u64,
    /// When this cycle started.
    pub started_at: Timestamp,
    /// Last checkpoint / reset time.
    pub last_checkpoint: Timestamp,
}

impl BudgetUsage {
    /// Create a fresh `BudgetUsage` snapshot starting now.
    pub fn start(now: Timestamp) -> Self {
        Self {
            started_at: now,
            last_checkpoint: now,
            ..Default::default()
        }
    }

    /// Record a model call.
    pub fn record_model_call(&mut self, tokens: u32, cost_cents: u64) {
        self.model_calls_used += 1;
        self.tokens_consumed = self.tokens_consumed.saturating_add(tokens);
        self.cost_incurred_cents = self.cost_incurred_cents.saturating_add(cost_cents);
    }

    /// Record that a branch was spawned.
    pub fn record_branch(&mut self) {
        self.branches_spawned = self.branches_spawned.saturating_add(1);
    }

    /// Record progression to a new reasoning depth.
    pub fn record_depth(&mut self, depth: usize) {
        self.reasoning_depth_reached = self.reasoning_depth_reached.max(depth);
    }
}

/// Trait for checking budget consumption at phase boundaries.
///
/// Implementations are typically thin wrappers around `CognitiveBudget` + `BudgetUsage`.
#[async_trait::async_trait]
pub trait BudgetChecker: Send + Sync {
    /// Return whether any budget dimension is exhausted.
    fn exhausted(&self, usage: &BudgetUsage) -> bool;

    /// Return remaining token budget.
    fn remaining_tokens(&self, usage: &BudgetUsage) -> u32;

    /// Return remaining cost budget in cents.
    fn remaining_cost_cents(&self, usage: &BudgetUsage) -> u64;

    /// Return true if `now` has passed the deadline.
    fn past_deadline(&self, now: Timestamp) -> bool;

    /// Produce a degraded budget for retry.
    fn degraded(&self) -> CognitiveBudget;
}

#[async_trait::async_trait]
impl BudgetChecker for CognitiveBudget {
    fn exhausted(&self, usage: &BudgetUsage) -> bool {
        CognitiveBudget::exhausted(self, usage)
    }
    fn remaining_tokens(&self, usage: &BudgetUsage) -> u32 {
        CognitiveBudget::remaining_tokens(self, usage)
    }
    fn remaining_cost_cents(&self, usage: &BudgetUsage) -> u64 {
        CognitiveBudget::remaining_cost_cents(self, usage)
    }
    fn past_deadline(&self, now: Timestamp) -> bool {
        CognitiveBudget::past_deadline(self, now)
    }
    fn degraded(&self) -> CognitiveBudget {
        CognitiveBudget::degraded(self)
    }
}
