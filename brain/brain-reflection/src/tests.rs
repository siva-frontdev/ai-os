#[cfg(test)]
mod tests {
    use crate::lesson_store::LessonStore;
    use crate::reflector::Reflector;
    use crate::types::{Lesson, MistakeCategory};
    use brain_core::ids::GoalId;
    use uuid::Uuid;

    fn make_goal_id(n: u8) -> GoalId {
        GoalId::from(Uuid::from_bytes({
            let mut buf = [0u8; 16];
            buf[0] = n;
            buf[15] = n;
            buf
        }))
    }

    #[test]
    fn test_reflector_compare_exact_match() {
        let r = Reflector::new();
        let result = r.compare("complete the task", "complete the task").unwrap();
        assert!((result.match_score - 1.0).abs() < 0.01);
        assert!(result.deviations.is_empty());
    }

    #[test]
    fn test_reflector_compare_no_match() {
        let r = Reflector::new();
        let result = r.compare("do this", "do that").unwrap();
        assert!(result.match_score < 1.0);
    }

    #[test]
    fn test_reflector_detects_mistakes() {
        let r = Reflector::new();
        let goal = make_goal_id(1);
        let reflection = r
            .reflect(goal, "expected great outcome", "poor outcome")
            .unwrap();
        assert!(!reflection.mistakes.is_empty());
        assert!(!reflection.improvements.is_empty());
    }

    #[test]
    fn test_reflection_creates_lessons() {
        let r = Reflector::new();
        let goal = make_goal_id(1);
        let reflection = r.reflect(goal, "plan A works", "plan A failed").unwrap();
        assert!(!reflection.lessons.is_empty());
        assert!(reflection.lessons[0].applied_count == 0);
    }

    #[test]
    fn test_lesson_store_crud() {
        let store = LessonStore::new();
        let goal = make_goal_id(1);
        let lesson = Lesson {
            lesson_id: brain_core::ids::LessonId::new(),
            goal_id: goal,
            summary: "test lesson".into(),
            details: "details".into(),
            category: MistakeCategory::Planning,
            confidence: brain_core::types::Confidence::new(0.8),
            created_at: memory_core::Timestamp::now(),
            applied_count: 0,
        };
        store.store(lesson.clone()).unwrap();
        let fetched = store.get(&lesson.lesson_id).unwrap();
        assert_eq!(fetched.summary, "test lesson");
        let list = store.list_for_goal(&goal).unwrap();
        assert_eq!(list.len(), 1);
        store.increment_applied(&lesson.lesson_id).unwrap();
        let updated = store.get(&lesson.lesson_id).unwrap();
        assert_eq!(updated.applied_count, 1);
    }
}
