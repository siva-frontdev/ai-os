//! `ReasoningContext` — context passed to the Reasoner.
use crate::budget::CognitiveBudget;
use crate::ids::{GoalId, ThoughtId};
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningContext {
    pub thought_id: ThoughtId,
    pub goal_id: GoalId,
    pub observations: Vec<ObservationRef>,
    pub hypotheses_in_progress: Vec<HypothesisRef>,
    pub constraints: Vec<ConstraintRef>,
    pub budget: CognitiveBudget,
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationRef {
    pub observation_id: String,
    pub source: String, // "perception", "memory", "runtime"
    pub data: String,   // serialised observation payload
    pub observed_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HypothesisRef {
    pub hypothesis_id: String,
    pub description: String,
    pub confidence: f64,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstraintRef {
    pub constraint_id: String,
    pub description: String,
    pub severity: String, // "soft", "hard"
}
