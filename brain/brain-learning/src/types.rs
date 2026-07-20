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
