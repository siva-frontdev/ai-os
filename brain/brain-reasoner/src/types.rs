use brain_core::ids::GoalId;
use brain_core::types::Confidence;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub description: String,
    pub confidence: Confidence,
    pub evidence_ids: Vec<String>,
    pub source: String,
    pub is_falsified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inference {
    pub id: String,
    pub premise: String,
    pub conclusion: String,
    pub confidence: Confidence,
    pub inference_type: InferenceType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InferenceType {
    Deductive,
    Inductive,
    Abductive,
    Analogical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub id: String,
    pub description: String,
    pub constraint_type: ConstraintType,
    pub severity: ConstraintSeverity,
    pub expression: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstraintType {
    Resource,
    Temporal,
    Dependency,
    Policy,
    Safety,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstraintSeverity {
    Hard,
    Soft,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeoffAnalysis {
    pub options: Vec<TradeoffOption>,
    pub recommended: Option<String>,
    pub reasoning: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeoffOption {
    pub id: String,
    pub label: String,
    pub cost: f64,
    pub benefit: f64,
    pub risk: f64,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub risk_id: String,
    pub description: String,
    pub probability: f64,
    pub impact: f64,
    pub mitigation: String,
    pub residual_risk: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningResult {
    pub goal_id: GoalId,
    pub hypotheses: Vec<Hypothesis>,
    pub inferences: Vec<Inference>,
    pub tradeoffs: Vec<TradeoffAnalysis>,
    pub risks: Vec<RiskAssessment>,
    pub final_confidence: Confidence,
    pub reasoning_chain: Vec<String>,
}
