//! Learning events.
use memory_core::Timestamp;
use ai_os_core::events::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningCycleStarted {
    pub source: String, // "ReflectionCompleted", "Mistake", "Periodic", "PerformanceDegraded"
    pub trigger_event_id: Option<String>,
    pub scope: String, // "goal", "session", "global"
    pub started_at: Timestamp,
}

impl Event for LearningCycleStarted {
    fn event_type(&self) -> &'static str { "brain.learning.cycle.started" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningCycleCompleted {
    pub source: String,
    pub promoted_count: u32,
    pub planner_adjustments_made: bool,
    pub reasoner_heuristic_updates: u32,
    pub completed_at: Timestamp,
}

impl Event for LearningCycleCompleted {
    fn event_type(&self) -> &'static str { "brain.learning.cycle.completed" }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionApplied {
    pub episodic_ids: Vec<String>,
    pub target_tier: String, // "semantic", "procedural", "generalised"
    pub promotion_reason: String,
    pub applied_at: Timestamp,
}

impl Event for PromotionApplied {
    fn event_type(&self) -> &'static str { "brain.learning.promotion.applied" }
}
