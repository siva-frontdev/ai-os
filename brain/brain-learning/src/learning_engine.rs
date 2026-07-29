use crate::errors::LearningResult;
use crate::knowledge_base::KnowledgeBase;
use crate::pattern_recognizer::PatternRecognizer;
use crate::types::{
    ConsolidationReport, DecompositionPattern, LearningFeedback, LearningSignal, PlannerHint,
    StrategyAdjustment,
};
use brain_reflection::types::{Lesson, MistakeCategory};

pub struct LearningEngine {
    recognizer: PatternRecognizer,
    knowledge: KnowledgeBase,
}

impl LearningEngine {
    pub fn new() -> Self {
        Self {
            recognizer: PatternRecognizer::new(),
            knowledge: KnowledgeBase::new(),
        }
    }

    pub fn learn_from_lesson(&self, lesson: &Lesson) -> LearningResult<ConsolidationReport> {
        let signal = LearningSignal {
            lesson_id: lesson.lesson_id,
            category: lesson.category,
            description: lesson.summary.clone(),
            context: lesson.details.clone(),
            success: lesson.applied_count > 0,
            timestamp: lesson.created_at,
        };
        self.recognizer.record_signal(signal)?;
        let patterns = self.recognizer.extract_patterns()?;
        self.knowledge.consolidate(patterns)
    }

    /// Generate closed-loop feedback for strategy and planning subsystems.
    ///
    /// This is the core of Phase 6: it takes accumulated patterns and
    /// translates them into actionable adjustments for the cognitive engine.
    pub fn generate_feedback(&self) -> LearningResult<LearningFeedback> {
        let patterns = self.recognizer.extract_patterns()?;
        let _knowledge_list = self.knowledge.list_knowledge()?;

        let mut strategy_adj = Vec::new();
        let mut planner_hints = Vec::new();
        let mut decomp_patterns = Vec::new();

        // Analyze patterns to produce strategy adjustments
        for pattern in &patterns {
            if pattern.frequency >= 3 && pattern.confidence.raw() >= 0.7 {
                let (goal_type, suggested) = self.suggest_strategy_change(pattern);
                if let Some((gt, sug)) = goal_type.zip(suggested) {
                    strategy_adj.push(StrategyAdjustment {
                        goal_type: gt,
                        current_strategy: pattern.category.as_str().into(),
                        suggested_strategy: sug,
                        confidence: pattern.confidence.raw() as f64,
                        reason: format!(
                            "pattern observed {} times: {}",
                            pattern.frequency, pattern.description
                        ),
                    });
                }
            }
        }

        // Generate planner hints from high-confidence patterns
        for pattern in &patterns {
            if pattern.confidence.raw() >= 0.6 {
                planner_hints.push(PlannerHint {
                    description: format!(
                        "avoid '{}' category mistakes by adding validation",
                        pattern.category.as_str()
                    ),
                    applies_to: vec![pattern.category.as_str().into()],
                    expected_benefit: format!(
                        "reduce {} errors by ~{}%",
                        pattern.category.as_str(),
                        (pattern.confidence.raw() * 50.0) as u32
                    ),
                    lesson_source: format!("{} lessons in pattern", pattern.lessons.len()),
                });
            }
        }

        // Generate decomposition patterns from related patterns
        if !patterns.is_empty() {
            let avg_freq =
                patterns.iter().map(|p| p.frequency).sum::<u32>() as f64 / patterns.len() as f64;
            decomp_patterns.push(DecompositionPattern {
                goal_type_pattern: patterns
                    .iter()
                    .map(|p| p.category.as_str())
                    .collect::<Vec<_>>()
                    .join("_"),
                suggested_objectives: vec![
                    "analysis".into(),
                    "planning".into(),
                    "execution".into(),
                    "validation".into(),
                ],
                suggested_order: vec![0, 1, 2, 3],
                confidence: (avg_freq / 10.0).min(1.0),
                based_on_lessons: patterns.iter().map(|p| p.lessons.len() as u32).sum(),
            });
        }

        let cognitive_health = if patterns.is_empty() {
            1.0
        } else {
            let critical = patterns
                .iter()
                .filter(|p| {
                    matches!(
                        p.category,
                        MistakeCategory::Safety | MistakeCategory::Planning
                    ) && p.confidence.raw() >= 0.7
                })
                .count();
            (1.0 - (critical as f64 * 0.2)).clamp(0.0, 1.0)
        };

        Ok(LearningFeedback {
            strategy_adjustments: strategy_adj,
            planner_hints,
            decomposition_patterns: decomp_patterns,
            cognitive_health,
            pattern_count: patterns.len() as u32,
        })
    }

    fn suggest_strategy_change(
        &self,
        pattern: &crate::types::Pattern,
    ) -> (Option<String>, Option<String>) {
        match pattern.category {
            MistakeCategory::Planning if pattern.frequency > 3 => {
                (Some("planning".into()), Some("iterative".into()))
            }
            MistakeCategory::Execution if pattern.frequency > 2 => {
                (Some("execution".into()), Some("sequential".into()))
            }
            MistakeCategory::Resource if pattern.frequency > 2 => {
                (Some("resource_intensive".into()), Some("delegated".into()))
            }
            MistakeCategory::Knowledge if pattern.confidence.raw() >= 0.7 => (
                Some("research".into()),
                Some("research_then_execute".into()),
            ),
            _ => (None, None),
        }
    }

    pub fn patterns(&self) -> LearningResult<Vec<crate::types::Pattern>> {
        self.recognizer.extract_patterns()
    }

    pub fn knowledge(&self) -> &KnowledgeBase {
        &self.knowledge
    }
}

impl Default for LearningEngine {
    fn default() -> Self {
        Self::new()
    }
}
