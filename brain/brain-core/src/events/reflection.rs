//! Reflection events.
use crate::ids::{DecisionId, ReflectionId, LessonId};
use memory_core::Timestamp;
use ai_os_core::events::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionCompleted {
    pub reflection_id: ReflectionId,
    pub decision_id: DecisionId,
    pub match_score: f64,
    pub mistakes_detected: Vec<String>,
    pub improvements_generated: Vec<String>,
    pub lesson_stored: Option<LessonId>,
    pub completed_at: Timestamp,
}

impl Event for ReflectionCompleted {
    fn event_type(&self) -> &'static str { "brain.reflection.completed" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutcomeObserved {
    pub decision_id: DecisionId,
    pub expected_outcome: String,
    pub actual_outcome: String,
    pub match_score: f64,
    pub observed_at: Timestamp,
}

impl Event for OutcomeObserved {
    fn event_type(&self) -> &'static str { "brain.reflection.outcome.observed" }
}
