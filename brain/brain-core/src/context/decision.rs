//! `DecisionContext` — context passed to the DecisionMaker.
use crate::budget::CognitiveBudget;
use crate::ids::DecisionId;
use crate::types::Confidence;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionContext {
    pub decision_id: DecisionId,
    pub plan_id: String, // PlanId
    pub options: Vec<OptionRef>,
    pub policies: Vec<PolicyRef>,
    pub conflict: Option<ConflictRef>,
    pub budget: CognitiveBudget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionRef {
    pub option_id: String,
    pub description: String,
    pub estimated_cost: f64,
    pub estimated_benefit: f64,
    pub estimated_risk: f64,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyRef {
    pub policy_id: String,
    pub ruleset_name: String,
    pub applicable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictRef {
    pub conflict_id: String,
    pub description: String,
    pub conflicting_options: Vec<String>,
    pub severity: String, // "minor", "major", "critical"
}
