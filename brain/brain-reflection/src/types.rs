use brain_core::ids::{GoalId, LessonId, ReflectionId};
use brain_core::types::Confidence;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reflection {
    pub reflection_id: ReflectionId,
    pub goal_id: GoalId,
    pub outcome: String,
    pub mistakes: Vec<Mistake>,
    pub improvements: Vec<Improvement>,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub expected: String,
    pub actual: String,
    pub match_score: f64,
    pub deviations: Vec<String>,
}
