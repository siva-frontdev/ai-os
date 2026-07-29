use crate::types::{Confidence, GoalPriority};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A named value extracted from the user's request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: String,
    pub value: String,
}

/// A constraint inferred from the user's request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InferredConstraint {
    pub description: String,
    pub constraint_type: String,
}

/// Structured output produced by the Intelligence Platform for a user request.
///
/// Returned by [`ReasoningService`](crate::traits::ReasoningService) so that
/// the Brain Platform receives machine-readable semantics instead of raw
/// natural-language text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReasoningResult {
    /// One-sentence summary of what the user wants.
    pub summary: String,
    /// The core intent label (e.g. "file_management", "system_settings").
    pub intent: String,
    /// How confident the model is in its interpretation.
    pub confidence: Confidence,
    /// Relevant observations extracted from the request.
    pub observations: Vec<String>,
    /// Named entities found (files, apps, values, etc.).
    pub entities: Vec<ExtractedEntity>,
    /// Constraints implied by the request.
    pub inferred_constraints: Vec<InferredConstraint>,
    /// Assumptions the model made.
    pub assumptions: Vec<String>,
    /// Possible interpretations considered.
    pub hypotheses: Vec<String>,
    /// Which hypothesis was selected.
    pub selected_hypothesis: Option<String>,
    /// Suggested goal description for the Brain to use.
    pub suggested_goal: String,
    /// Suggested priority level.
    pub suggested_priority: GoalPriority,
    /// High-level capabilities needed (e.g. "browsing", "file_operations").
    pub required_capabilities: Vec<String>,
    /// Why the model interpreted the request this way.
    pub explanation: String,
    /// Arbitrary metadata key-value pairs.
    pub metadata: HashMap<String, String>,
}
