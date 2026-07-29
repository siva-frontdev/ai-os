//! Brain-internal entity identifier types.
//!
//! All IDs are newtype wrappers around [`uuid::Uuid`].
//! `GoalId` uses UUID v7 (time-sortable) for chronological ordering.
//! All others use UUID v4 (random).

pub use self::agent_id::AgentId;
pub use self::checkpoint_id::CheckpointId;
pub use self::decision_id::DecisionId;
pub use self::goal_id::GoalId;
pub use self::lesson_id::LessonId;
pub use self::milestone_id::MilestoneId;
pub use self::objective_id::ObjectiveId;
pub use self::plan_id::PlanId;
pub use self::reflection_id::ReflectionId;
pub use self::strategy_id::StrategyId;
pub use self::task_id::TaskId;
pub use self::thought_id::ThoughtId;
pub use self::tool_capability_id::ToolCapabilityId;
pub use self::tool_id::ToolId;
pub use self::workflow_id::WorkflowId;

mod agent_id;
mod checkpoint_id;
mod decision_id;
mod goal_id;
mod lesson_id;
mod milestone_id;
mod objective_id;
mod plan_id;
mod reflection_id;
mod strategy_id;
mod task_id;
mod thought_id;
mod tool_capability_id;
mod tool_id;
mod workflow_id;
