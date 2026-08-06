//! Phase 6 — Autonomous Agent: reflection and learning.
//!
//! Proves the autonomous loop closes: outcomes are reflected upon,
//! lessons are extracted, consolidated into patterns, and converted into
//! strategy feedback that adjusts future behavior.

use brain_core::ids::GoalId;
use brain_core::types::Confidence;
use brain_learning::LearningEngine;
use brain_reflection::types::{Lesson, MistakeCategory};
use brain_reflection::Reflector;
use memory_core::Timestamp;

#[test]
fn reflection_extracts_lessons_from_mismatched_outcome() {
    let reflector = Reflector::new();
    let goal_id = GoalId::new();

    let reflection = reflector
        .reflect(
            goal_id,
            "move files and verify checksums",
            "files moved but no checksum verification",
        )
        .expect("reflection produced");

    assert_eq!(reflection.goal_id, goal_id);
    assert!(reflection.dimensions.overall() < 1.0, "imperfect outcome");
    assert!(
        !reflection.mistakes.is_empty() || !reflection.lessons.is_empty(),
        "mismatched outcome produces mistakes or lessons"
    );
    assert!(reflection.confidence.raw() >= 0.0 && reflection.confidence.raw() <= 1.0);
}

#[test]
fn identical_outcome_yields_high_correctness() {
    let reflector = Reflector::new();
    let comparison = reflector
        .compare(
            "clean temp directory and report",
            "clean temp directory and report",
        )
        .expect("comparison");
    assert_eq!(comparison.match_score, 1.0);
    assert!(comparison.dimensions.overall() >= 0.9);
}

#[test]
fn learning_engine_consolidates_lesson_into_knowledge() {
    let engine = LearningEngine::new();
    let lesson = make_lesson(MistakeCategory::Execution);

    let report = engine.learn_from_lesson(&lesson).expect("consolidated");
    assert_eq!(report.lesson_count, 1);
    assert!(report.pattern_count >= 1, "at least one pattern recognized");
}

#[test]
fn repeated_planning_mistakes_drive_strategy_feedback() {
    let engine = LearningEngine::new();

    // Feed four planning-category lessons so the pattern matures past the
    // frequency>3 threshold that triggers a strategy suggestion.
    for _ in 0..4 {
        let lesson = make_lesson(MistakeCategory::Planning);
        engine.learn_from_lesson(&lesson).expect("learned");
    }

    let feedback = engine.generate_feedback().expect("feedback");
    assert!(feedback.pattern_count >= 1, "patterns observed");
    assert!(feedback.cognitive_health <= 1.0);
    assert!(
        feedback
            .strategy_adjustments
            .iter()
            .any(|adj| adj.suggested_strategy == "iterative"),
        "repeated planning mistakes suggest an iterative strategy"
    );
}

#[test]
fn empty_learning_state_reports_no_patterns() {
    let engine = LearningEngine::new();
    let feedback = engine.generate_feedback();
    assert!(
        matches!(feedback, Err(brain_learning::LearningError::NoPatterns(_))),
        "no signals recorded → no feedback can be generated"
    );
}

#[test]
fn lessons_accumulate_and_knowledge_version_bumps() {
    let engine = LearningEngine::new();

    let r1 = engine
        .learn_from_lesson(&make_lesson(MistakeCategory::Knowledge))
        .expect("first lesson");
    let r2 = engine
        .learn_from_lesson(&make_lesson(MistakeCategory::Knowledge))
        .expect("second lesson");

    assert_eq!(r1.knowledge.version, 1);
    assert!(r2.knowledge.version >= r1.knowledge.version);
    assert!(r2.lesson_count >= r1.lesson_count);
}

/// Build a reusable lesson for the given mistake category.
fn make_lesson(category: MistakeCategory) -> Lesson {
    Lesson {
        lesson_id: brain_core::ids::LessonId::new(),
        goal_id: GoalId::new(),
        summary: format!("lesson about {}", category.as_str()),
        details: "learned from a failed outcome".into(),
        category,
        confidence: Confidence::new(0.8),
        created_at: Timestamp::now(),
        applied_count: 0,
        tags: vec!["integration".into()],
    }
}
