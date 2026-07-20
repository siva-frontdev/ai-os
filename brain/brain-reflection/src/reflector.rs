use crate::errors::ReflectionResult;
use crate::types::{
    ComparisonResult, Improvement, ImprovementCategory, Lesson, Mistake,
    MistakeCategory, MistakeSeverity, Reflection,
};
use brain_core::ids::{GoalId, LessonId, ReflectionId};
use brain_core::types::Confidence;
use memory_core::Timestamp;

pub struct Reflector;

impl Reflector {
    pub fn new() -> Self { Self }

    pub fn reflect(
        &self,
        goal_id: GoalId,
        expected_outcome: &str,
        actual_outcome: &str,
    ) -> ReflectionResult<Reflection> {
        let comparison = self.compare(expected_outcome, actual_outcome)?;
        let mistakes = self.detect_mistakes(&comparison);
        let improvements = self.generate_improvements(&mistakes);
        let lessons = self.extract_lessons(goal_id, &mistakes, &improvements);

        Ok(Reflection {
            reflection_id: ReflectionId::new(),
            goal_id,
            outcome: actual_outcome.into(),
            mistakes,
            improvements,
            lessons,
            confidence: Confidence::new(comparison.match_score as f32),
            created_at: Timestamp::now(),
        })
    }

    pub fn compare(&self, expected: &str, actual: &str) -> ReflectionResult<ComparisonResult> {
        let expected_words: Vec<&str> = expected.split_whitespace().collect();
        let actual_words: Vec<&str> = actual.split_whitespace().collect();

        let common = expected_words.iter().filter(|w| actual_words.contains(w)).count();
        let total = expected_words.len().max(actual_words.len());
        let match_score = if total == 0 { 1.0 } else { common as f64 / total as f64 };

        let mut deviations = Vec::new();
        if match_score < 0.5 {
            deviations.push("significant deviation between expected and actual outcome".into());
        }
        if expected_words.len() != actual_words.len() {
            deviations.push(format!("length mismatch: expected {} words, got {}", expected_words.len(), actual_words.len()));
        }

        Ok(ComparisonResult {
            expected: expected.into(),
            actual: actual.into(),
            match_score,
            deviations,
        })
    }

    fn detect_mistakes(&self, comparison: &ComparisonResult) -> Vec<Mistake> {
        let mut mistakes = Vec::new();
        if comparison.match_score < 0.3 {
            mistakes.push(Mistake {
                id: "mistake-planning-1".into(),
                description: "plan did not produce expected outcome".into(),
                category: MistakeCategory::Planning,
                severity: MistakeSeverity::Critical,
                root_cause: "incorrect strategy selection".into(),
            });
        } else if comparison.match_score < 0.7 {
            mistakes.push(Mistake {
                id: "mistake-execution-1".into(),
                description: "partial deviation from expected outcome".into(),
                category: MistakeCategory::Execution,
                severity: MistakeSeverity::Major,
                root_cause: "suboptimal execution".into(),
            });
        }
        mistakes
    }

    fn generate_improvements(&self, mistakes: &[Mistake]) -> Vec<Improvement> {
        mistakes.iter().map(|m| {
            let category = match m.category {
                MistakeCategory::Planning => ImprovementCategory::Strategy,
                MistakeCategory::Execution => ImprovementCategory::Coordination,
                _ => ImprovementCategory::Prompt,
            };
            Improvement {
                id: format!("improve-{}", m.id),
                description: format!("address root cause: {}", m.root_cause),
                expected_impact: match m.severity {
                    MistakeSeverity::Critical => 0.8,
                    MistakeSeverity::Major => 0.5,
                    MistakeSeverity::Minor => 0.2,
                },
                category,
            }
        }).collect()
    }

    fn extract_lessons(&self, goal_id: GoalId, mistakes: &[Mistake], improvements: &[Improvement]) -> Vec<Lesson> {
        mistakes.iter().zip(improvements.iter()).map(|(m, i)| {
            Lesson {
                lesson_id: LessonId::new(),
                goal_id,
                summary: format!("lesson from {}: {}", m.category.as_str(), m.root_cause),
                details: format!("mistake: {}. improvement: {}", m.description, i.description),
                category: m.category,
                confidence: Confidence::new(0.7),
                created_at: Timestamp::now(),
                applied_count: 0,
            }
        }).collect()
    }
}

impl Default for Reflector {
    fn default() -> Self { Self::new() }
}

impl MistakeCategory {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Reasoning => "reasoning",
            Self::Execution => "execution",
            Self::Timing => "timing",
            Self::Resource => "resource",
            Self::Unknown => "unknown",
        }
    }
}
