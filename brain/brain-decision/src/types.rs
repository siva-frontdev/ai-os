use brain_core::ids::{DecisionId, GoalId};
use brain_core::types::Confidence;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub decision_id: DecisionId,
    pub goal_id: GoalId,
    pub chosen_option: String,
    pub confidence: Confidence,
    pub alternatives: Vec<String>,
    pub reasoning: String,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceScore {
    pub overall: f64,
    pub evidence_strength: f64,
    pub model_confidence: f64,
    pub consistency_score: f64,
    pub uncertainty: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictDescription {
    pub conflict_id: String,
    pub options: Vec<String>,
    pub description: String,
    pub severity: ConflictSeverity,
    pub resolution_strategy: ResolutionStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictSeverity {
    Minor,
    Major,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResolutionStrategy {
    PrioritizeByConfidence,
    PrioritizeByBenefit,
    DeferToHuman,
    Vote,
    Arbitrate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Explanation {
    pub decision_id: DecisionId,
    pub summary: String,
    pub key_factors: Vec<String>,
    pub alternatives_considered: Vec<String>,
    pub confidence_rationale: String,
    pub policy_compliance: String,
}
