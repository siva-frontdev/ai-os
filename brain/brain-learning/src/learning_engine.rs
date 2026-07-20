use crate::errors::LearningResult;
use crate::knowledge_base::KnowledgeBase;
use crate::pattern_recognizer::PatternRecognizer;
use crate::types::{ConsolidationReport, LearningSignal};
use brain_reflection::types::Lesson;

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
