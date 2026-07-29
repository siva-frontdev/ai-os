use brain_core::ids::LessonId;
use brain_core::types::Confidence;
use brain_reflection::types::MistakeCategory;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub id: String,
    pub category: MistakeCategory,
    pub description: String,
    pub frequency: u32,
    pub confidence: Confidence,
    pub last_observed: Timestamp,
    pub lessons: Vec<LessonId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Knowledge {
    pub id: String,
    pub summary: String,
    pub patterns: Vec<Pattern>,
    pub confidence: Confidence,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningSignal {
    pub lesson_id: LessonId,
    pub category: MistakeCategory,
    pub description: String,
    pub context: String,
    pub success: bool,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsolidationReport {
    pub knowledge: Knowledge,
    pub patterns_found: Vec<Pattern>,
    pub pattern_count: u32,
    pub lesson_count: u32,
    pub merged_patterns: Vec<String>,
}

/// Feedback from the learning subsystem to strategy and planning.
///
/// This is the closed-loop signal that allows the Brain to improve
/// its own cognitive processes based on past outcomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningFeedback {
    /// Adjustments to strategy selection weights by goal type
    pub strategy_adjustments: Vec<StrategyAdjustment>,
    /// Hints for the planner about optimal decomposition
    pub planner_hints: Vec<PlannerHint>,
    /// Patterns that suggest how to decompose similar goals
    pub decomposition_patterns: Vec<DecompositionPattern>,
    /// Overall cognitive health score (0.0-1.0)
    pub cognitive_health: f64,
    /// Number of patterns contributing to this feedback
    pub pattern_count: u32,
}

/// Suggests adjusting the strategy for a given goal type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyAdjustment {
    pub goal_type: String,
    pub current_strategy: String,
    pub suggested_strategy: String,
    pub confidence: f64,
    pub reason: String,
}

/// Provides hints to the planner about task decomposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannerHint {
    pub description: String,
    pub applies_to: Vec<String>,
    pub expected_benefit: String,
    pub lesson_source: String,
}

/// Describes how similar goals should be decomposed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecompositionPattern {
    pub goal_type_pattern: String,
    pub suggested_objectives: Vec<String>,
    pub suggested_order: Vec<u32>,
    pub confidence: f64,
    pub based_on_lessons: u32,
}
