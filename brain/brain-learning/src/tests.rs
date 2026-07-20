#[cfg(test)]
mod tests {
    use crate::knowledge_base::KnowledgeBase;
    use crate::learning_engine::LearningEngine;
    use crate::pattern_recognizer::PatternRecognizer;
    use crate::types::LearningSignal;
    use brain_core::ids::LessonId;
    use brain_core::types::Confidence;
    use brain_reflection::types::{Lesson, MistakeCategory};
    use memory_core::Timestamp;
    use uuid::Uuid;

    fn make_lesson_id(b: u8) -> LessonId {
        LessonId::from(Uuid::from_bytes({
            let mut buf = [0u8; 16];
            buf[0] = b;
            buf[15] = b;
            buf
        }))
    }

    #[test]
    fn test_recognizer_records_signal() {
        let r = PatternRecognizer::new();
        let signal = LearningSignal {
            lesson_id: make_lesson_id(1),
            category: MistakeCategory::Planning,
            description: "bad plan".into(),
            context: "plan failed".into(),
            success: false,
            timestamp: Timestamp::now(),
        };
        r.record_signal(signal).unwrap();
        let patterns = r.extract_patterns().unwrap();
        assert_eq!(patterns.len(), 1);
    }

    #[test]
    fn test_recognizer_categorizes_patterns() {
        let r = PatternRecognizer::new();
        for i in 0..4 {
            r.record_signal(LearningSignal {
                lesson_id: make_lesson_id(i),
                category: MistakeCategory::Planning,
                description: format!("plan error {}", i),
                context: "ctx".into(),
                success: false,
                timestamp: Timestamp::now(),
            })
            .unwrap();
        }
        let patterns = r.extract_patterns().unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].frequency, 4);
    }

    #[test]
    fn test_knowledge_base_consolidation() {
        let kb = KnowledgeBase::new();
        use brain_reflection::types::MistakeCategory;
        let pattern = crate::types::Pattern {
            id: "test-pattern".into(),
            category: MistakeCategory::Execution,
            description: "test pattern".into(),
            frequency: 3,
            confidence: Confidence::new(0.7),
            last_observed: Timestamp::now(),
            lessons: vec![make_lesson_id(1)],
        };
        let report = kb.consolidate(vec![pattern]).unwrap();
        assert_eq!(report.pattern_count, 1);
        assert_eq!(report.lesson_count, 1);
        assert!(!report.knowledge.summary.is_empty());
    }

    #[test]
    fn test_learning_engine_full_cycle() {
        let engine = LearningEngine::new();
        let lesson = Lesson {
            lesson_id: make_lesson_id(1),
            goal_id: brain_core::ids::GoalId::from(Uuid::from_bytes({
                let mut buf = [0u8; 16];
                buf[0] = 1;
                buf[15] = 1;
                buf
            })),
            summary: "test lesson".into(),
            details: "details".into(),
            category: MistakeCategory::Planning,
            confidence: Confidence::new(0.8),
            created_at: Timestamp::now(),
            applied_count: 0,
        };
        let report = engine.learn_from_lesson(&lesson).unwrap();
        assert!(report.pattern_count > 0);
        assert!(report.lesson_count > 0);
    }

    #[test]
    fn test_multiple_patterns() {
        let engine = LearningEngine::new();
        for i in 0..3 {
            let lesson = Lesson {
                lesson_id: make_lesson_id(i),
                goal_id: brain_core::ids::GoalId::from(Uuid::from_bytes({
                    let mut buf = [0u8; 16];
                    buf[0] = i;
                    buf[15] = i;
                    buf
                })),
                summary: format!("lesson {}", i),
                details: "details".into(),
                category: match i % 3 {
                    0 => MistakeCategory::Planning,
                    1 => MistakeCategory::Execution,
                    _ => MistakeCategory::Timing,
                },
                confidence: Confidence::new(0.8),
                created_at: Timestamp::now(),
                applied_count: 0,
            };
            engine.learn_from_lesson(&lesson).unwrap();
        }
        let patterns = engine.patterns().unwrap();
        assert_eq!(patterns.len(), 3);
    }
}
