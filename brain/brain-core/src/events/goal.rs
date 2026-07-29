//! Goal lifecycle events.
use crate::ids::GoalId;
use crate::types::{GoalPriority, GoalStatus};
use memory_core::Timestamp;

use ai_os_core::events::Event;
use serde::{Deserialize, Serialize};

/// Published when a new goal is registered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalCreated {
    pub goal_id: GoalId,
    pub goal_type: String,
    pub priority: GoalPriority,
    pub dependencies: Vec<GoalId>,
    pub deadline: Option<Timestamp>,
    pub created_at: Timestamp,
}

impl Event for GoalCreated {
    fn event_type(&self) -> &'static str {
        "brain.goal.created"
    }
}

/// Published when a goal moves from Pending to Active.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalActivated {
    pub goal_id: GoalId,
    pub prior_status: GoalStatus,
    pub activated_at: Timestamp,
}

impl Event for GoalActivated {
    fn event_type(&self) -> &'static str {
        "brain.goal.activated"
    }
}

/// Published when a goal reaches Completed status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalCompleted {
    pub goal_id: GoalId,
    pub outcome: String,
    pub final_confidence: crate::types::Confidence,
    pub total_duration_ms: u64,
    pub completed_at: Timestamp,
}

impl Event for GoalCompleted {
    fn event_type(&self) -> &'static str {
        "brain.goal.completed"
    }
}

/// Published when a goal fails (replan or recovery exhausted).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalFailed {
    pub goal_id: GoalId,
    pub reason: String,
    pub recovery_attempts: u32,
    pub last_error: String,
    pub failed_at_phase: BrainPhase,
    pub failed_at: Timestamp,
}

impl Event for GoalFailed {
    fn event_type(&self) -> &'static str {
        "brain.goal.failed"
    }
}

/// Phase in which a goal failure occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrainPhase {
    Planning,
    Reasoning,
    Decision,
    Execution,
    Reflection,
    Learning,
    Unknown,
}

/// Published when a goal is cancelled by operator or policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalCancelled {
    pub goal_id: GoalId,
    pub reason: String,
    pub cancelled_at_phase: Option<BrainPhase>,
    pub cancelled_at: Timestamp,
}

impl Event for GoalCancelled {
    fn event_type(&self) -> &'static str {
        "brain.goal.cancelled"
    }
}

/// Published when a goal is paused (cooperative — phase boundaries only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalPaused {
    pub goal_id: GoalId,
    pub reason: Option<String>,
    pub paused_at: Timestamp,
}

impl Event for GoalPaused {
    fn event_type(&self) -> &'static str {
        "brain.goal.paused"
    }
}

/// Published when a dependency violation is detected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalDependencyViolation {
    pub goal_id: GoalId,
    pub blocking_dependency: GoalId,
    pub violation_type: String, // "unresolved", "cycle", "not_found"
    pub detected_at: Timestamp,
}

impl Event for GoalDependencyViolation {
    fn event_type(&self) -> &'static str {
        "brain.goal.dependency.violation"
    }
}

/// Published when two goals are merged into one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalMerged {
    pub target_goal_id: GoalId,
    pub source_goal_id: GoalId,
    pub merged_at: Timestamp,
}

impl Event for GoalMerged {
    fn event_type(&self) -> &'static str {
        "brain.goal.merged"
    }
}

/// Published when a goal is split into multiple sub-goals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalSplit {
    pub source_goal_id: GoalId,
    pub new_goal_ids: Vec<GoalId>,
    pub split_at: Timestamp,
}

impl Event for GoalSplit {
    fn event_type(&self) -> &'static str {
        "brain.goal.split"
    }
}

/// Published when a goal's priority is changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalReprioritized {
    pub goal_id: GoalId,
    pub old_priority: GoalPriority,
    pub new_priority: GoalPriority,
    pub reason: String,
    pub reprioritized_at: Timestamp,
}

impl Event for GoalReprioritized {
    fn event_type(&self) -> &'static str {
        "brain.goal.reprioritized"
    }
}

/// Published when a goal is archived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalArchived {
    pub goal_id: GoalId,
    pub reason: String,
    pub archived_at: Timestamp,
}

impl Event for GoalArchived {
    fn event_type(&self) -> &'static str {
        "brain.goal.archived"
    }
}

/// Published when a goal becomes blocked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalBlocked {
    pub goal_id: GoalId,
    pub blocker: String,
    pub blocked_at: Timestamp,
}

impl Event for GoalBlocked {
    fn event_type(&self) -> &'static str {
        "brain.goal.blocked"
    }
}

/// Published when a goal is deferred.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalDeferred {
    pub goal_id: GoalId,
    pub defer_until: Option<Timestamp>,
    pub reason: String,
    pub deferred_at: Timestamp,
}

impl Event for GoalDeferred {
    fn event_type(&self) -> &'static str {
        "brain.goal.deferred"
    }
}

/// Published when a goal is retried.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalRetried {
    pub goal_id: GoalId,
    pub attempt: u32,
    pub reason: String,
    pub retried_at: Timestamp,
}

impl Event for GoalRetried {
    fn event_type(&self) -> &'static str {
        "brain.goal.retried"
    }
}

/// Published when a new objective is created as part of goal decomposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectiveCreated {
    pub objective_id: crate::ids::ObjectiveId,
    pub goal_id: GoalId,
    pub description: String,
    pub order: u32,
    pub created_at: Timestamp,
}

impl Event for ObjectiveCreated {
    fn event_type(&self) -> &'static str {
        "brain.goal.objective.created"
    }
}

/// Published when a milestone within a goal is reached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MilestoneReached {
    pub milestone_id: crate::ids::MilestoneId,
    pub objective_id: crate::ids::ObjectiveId,
    pub goal_id: GoalId,
    pub description: String,
    pub reached_at: Timestamp,
}

impl Event for MilestoneReached {
    fn event_type(&self) -> &'static str {
        "brain.goal.milestone.reached"
    }
}
