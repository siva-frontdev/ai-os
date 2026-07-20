//! Policy data types.
use serde::{Deserialize, Serialize};

// ── Rule primitives ──────────────────────────────────────────

/// Severity of a policy violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Warning,
    Blocking,
    Critical,
}

impl std::fmt::Display for ViolationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warning => f.write_str("Warning"),
            Self::Blocking => f.write_str("Blocking"),
            Self::Critical => f.write_str("Critical"),
        }
    }
}

/// Condition that a `PolicyRule` checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuleCondition {
    /// Always true or always false.
    Always,
    /// Context key matches a regex pattern.
    ContextMatches { key: String, pattern: String },
    /// A numeric metric crosses a threshold.
    Threshold { metric: String, operator: Comparison, value: f64 },
    /// A budget dimension is exhausted.
    BudgetExceeded { dimension: String },
    /// A specific guard rail triggered.
    GuardRailTriggered { guard_rail: String },
}

/// Action to take when a rule fires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuleAction {
    Allow,
    Block { reason: String },
    AdjustWeight { dimension: String, multiplier: f64 },
    Escalate { channel: EscalationChannel },
    Log { level: LogLevel },
}

/// Comparison operator for threshold rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Comparison {
    Eq, Ne, Lt, Le, Gt, Ge,
}

/// Channel for escalated policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EscalationChannel {
    Operator,
    Human,
    Admin,
    ExternalWebhook,
}

/// Log level for `RuleAction::Log`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    Debug, Info, Warn, Error,
}

// ── Rule / ruleset types ─────────────────────────────────────

/// A single policy rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub rule_id: String, // UUID string
    pub name: String,
    pub description: String,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub severity: ViolationSeverity,
    pub active: bool,
    pub priority: u32,
}

impl PolicyRule {
    pub fn new<N1, N2, D>(name: N1, description: D, condition: RuleCondition, action: RuleAction) -> Self
    where
        N1: Into<String>, N2: Into<String>, D: Into<String>,
    {
        Self {
            rule_id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            condition,
            action,
            severity: ViolationSeverity::Blocking,
            active: true,
            priority: 0,
        }
    }
}

/// A named set of policy rules loaded together.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RuleSet {
    pub name: String,
    pub version: u32,
    pub active: bool,
    pub rules: Vec<PolicyRule>,
}

impl RuleSet {
    pub fn new<N: Into<String>>(name: N) -> Self {
        Self { name: name.into(), version: 1, active: true, rules: Vec::new() }
    }
}

/// A guard rail — a high-priority policy always enforced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardRail {
    pub id: String,
    pub name: String,
    pub description: String,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub severity: ViolationSeverity,
    pub active: bool,
}

impl GuardRail {
    pub fn new<N1, N2, D>(name: N1, description: D, condition: RuleCondition, action: RuleAction) -> Self
    where
        N1: Into<String>, N2: Into<String>, D: Into<String>,
    {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            condition,
            action,
            severity: ViolationSeverity::Critical,
            active: true,
        }
    }
}

/// Top-level policy configuration (loaded from `configs/policy/`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub rulesets: Vec<RuleSet>,
    pub guard_rails: Vec<GuardRail>,
    pub evaluation_order: String, // "sequential", "prioritized", "parallel"
}

// ── Evaluation result types ───────────────────────────────────

/// A single violated rule from a policy evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViolatedRule {
    pub rule_id: String,
    pub rule_name: String,
    pub violation_detail: String,
    pub severity: ViolationSeverity,
    pub action: RuleAction,
}

/// Result of evaluating a policy against a plan.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PolicyEvaluation {
    /// Whether the plan passed all relevant rules.
    pub passed: bool,
    /// Rules that were violated.
    pub violated_rules: Vec<ViolatedRule>,
    /// Non-blocking warnings.
    pub warnings: Vec<String>,
    /// 0.0 = fully blocked, 1.0 = fully allowed.
    pub allowance: f64,
}

impl PolicyEvaluation {
    /// Whether any blocking violation was detected.
    pub fn has_blocking_violation(&self) -> bool {
        self.violated_rules.iter().any(|v| v.severity == ViolationSeverity::Blocking || v.severity == ViolationSeverity::Critical)
    }

    /// Whether any critical violation was detected.
    pub fn has_critical_violation(&self) -> bool {
        self.violated_rules.iter().any(|v| v.severity == ViolationSeverity::Critical)
    }
}

/// A pre-compiled rule set (result of PolicyCompiler::compile).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CompiledRuleSet {
    pub name: String,
    pub version: u32,
    pub rules: Vec<CompiledRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledRule {
    pub rule_id: String,
    pub name: String,
    pub compiled_at: memory_core::Timestamp,
    pub bytes: Vec<u8>, // serialised compiled form (opaque to consumers)
}

/// A change notification from the policy loader.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyChange {
    pub change_type: String, // "ruleset_loaded", "guard_rail_reloaded", "rule_added", "rule_removed"
    pub source: String,
    pub ruleset_name: Option<String>,
    pub occurred_at: memory_core::Timestamp,
}
