use brain_core::ids::{GoalId, LessonId, ReflectionId};
use brain_core::types::Confidence;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

/// Multi-dimensional outcome assessment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutcomeDimensions {
    pub correctness: f64,
    pub completeness: f64,
    pub efficiency: f64,
    pub safety: f64,
    pub user_satisfaction: f64,
}

impl OutcomeDimensions {
    pub fn overall(&self) -> f64 {
        (self.correctness * 0.3
            + self.completeness * 0.25
            + self.efficiency * 0.15
            + self.safety * 0.2
            + self.user_satisfaction * 0.1)
            .clamp(0.0, 1.0)
    }
}

/// Root cause analysis with causal chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RootCause {
    pub primary: String,
    pub contributing_factors: Vec<String>,
    pub category: MistakeCategory,
    pub severity: MistakeSeverity,
    pub fix_suggestion: String,
}

/// Actionable insight with priority and expected impact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionableInsight {
    pub id: String,
    pub summary: String,
    pub detail: String,
    pub category: ImprovementCategory,
    pub priority: u8,
    pub estimated_impact: f64,
    pub effort_estimate: String,
    pub applies_to_future_goals: Vec<String>,
}

/// Enhanced reflection with structured analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reflection {
    pub reflection_id: ReflectionId,
    pub goal_id: GoalId,
    pub outcome: String,
    pub dimensions: OutcomeDimensions,
    pub mistakes: Vec<Mistake>,
    pub improvements: Vec<Improvement>,
    pub root_causes: Vec<RootCause>,
    pub actionable_insights: Vec<ActionableInsight>,
    pub lessons: Vec<Lesson>,
    pub confidence: Confidence,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mistake {
    pub id: String,
    pub description: String,
    pub category: MistakeCategory,
    pub severity: MistakeSeverity,
    pub root_cause: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MistakeCategory {
    Planning,
    Reasoning,
    Execution,
    Timing,
    Resource,
    Knowledge,
    Communication,
    Safety,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MistakeSeverity {
    Minor,
    Major,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Improvement {
    pub id: String,
    pub description: String,
    pub expected_impact: f64,
    pub category: ImprovementCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImprovementCategory {
    Prompt,
    Strategy,
    Resource,
    Timing,
    Coordination,
    Knowledge,
    Safety,
    Automation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lesson {
    pub lesson_id: LessonId,
    pub goal_id: GoalId,
    pub summary: String,
    pub details: String,
    pub category: MistakeCategory,
    pub confidence: Confidence,
    pub created_at: Timestamp,
    pub applied_count: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub expected: String,
    pub actual: String,
    pub match_score: f64,
    pub deviations: Vec<String>,
    pub dimensions: OutcomeDimensions,
}

impl MistakeCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Reasoning => "reasoning",
            Self::Execution => "execution",
            Self::Timing => "timing",
            Self::Resource => "resource",
            Self::Knowledge => "knowledge",
            Self::Communication => "communication",
            Self::Safety => "safety",
            Self::Unknown => "unknown",
        }
    }
}
