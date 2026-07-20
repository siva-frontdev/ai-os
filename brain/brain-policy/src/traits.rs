use brain_core::context::DecisionContext;
use brain_core::tool::ExecutablePlan;
use brain_core::budget::CognitiveBudget;
/// Policy engine traits.
use async_trait::async_trait;
use crate::errors::PolicyResult;
use crate::types::*;

// ── PolicyStore ───────────────────────────────────────────────

/// Loads and stores named rule sets.
#[async_trait]
pub trait PolicyStore: Send + Sync {
    async fn load_ruleset(&self, name: &str) -> PolicyResult<RuleSet>;
    async fn store_ruleset(&self, ruleset: RuleSet) -> PolicyResult<()>;
    async fn list_rulesets(&self) -> PolicyResult<Vec<String>>;
    async fn load_guard_rails(&self) -> PolicyResult<Vec<GuardRail>>;
    async fn reload(&self) -> PolicyResult<()>;
}

// ── PolicyEngine ──────────────────────────────────────────────

/// Evaluates a plan against policy rules.
#[async_trait]
pub trait PolicyEngine: Send + Sync {
    async fn evaluate(&self, ctx: &DecisionContext, plan: &ExecutablePlan, budget: &CognitiveBudget) -> PolicyResult<PolicyEvaluation>;
    async fn evaluate_rule(&self, rule: &PolicyRule, ctx: &DecisionContext) -> PolicyResult<bool>;
    async fn check_guard_rails(&self, plan: &ExecutablePlan, budget: &CognitiveBudget) -> PolicyResult<Vec<ViolatedRule>>;
    async fn enforce(&self, evaluation: &PolicyEvaluation, ctx: &mut DecisionContext) -> PolicyResult<()>;
}

// ── PolicyEvaluator ───────────────────────────────────────────

/// Selects applicable rules and evaluates them.
#[async_trait]
pub trait PolicyEvaluator: Send + Sync {
    async fn applicable(&self, policies: &[RuleSet], ctx: &DecisionContext) -> PolicyResult<Vec<PolicyRule>>;
    async fn evaluate_policy(&self, policy: &RuleSet, ctx: &DecisionContext, plan: &ExecutablePlan) -> PolicyResult<PolicyEvaluation>;
    async fn aggregate(&self, evaluations: &[PolicyEvaluation]) -> PolicyResult<PolicyEvaluation>;
}

// ── PolicyCompiler ────────────────────────────────────────────

/// Compiles a RuleSet to a more efficient form.
#[async_trait]
pub trait PolicyCompiler: Send + Sync {
    async fn compile(&self, ruleset: &RuleSet) -> PolicyResult<CompiledRuleSet>;
    async fn validate(&self, compiled: &CompiledRuleSet) -> PolicyResult<CompilationValidation>;
    async fn optimize(&self, compiled: CompiledRuleSet) -> PolicyResult<CompiledRuleSet>;
}

#[derive(Debug, Clone, Default)]
pub struct CompilationValidation {
    pub valid: bool,
    pub errors: Vec<String>,
}

// ── PolicyLoader ──────────────────────────────────────────────

/// Loads policy configuration from files, config, or watchers.
#[async_trait]
pub trait PolicyLoader: Send + Sync {
    async fn load_from_config(&self, config: &PolicyConfig) -> PolicyResult<Vec<RuleSet>>;
    async fn load_from_file(&self, path: &str) -> PolicyResult<RuleSet>;
    async fn watch_for_changes(&self, path: &str) -> PolicyResult<std::sync::mpsc::Receiver<PolicyChange>>;
}

// ── GuardRails ────────────────────────────────────────────────

/// Manages guard rails — pre-compiled high-priority rules always active.
#[async_trait]
pub trait GuardRails: Send + Sync {
    fn active_guard_rails(&self) -> &[GuardRail];
    async fn check(&self, plan: &ExecutablePlan, budget: &CognitiveBudget) -> PolicyResult<Vec<ViolatedRule>>;
    fn reload(&mut self, guard_rails: Vec<GuardRail>);
}
