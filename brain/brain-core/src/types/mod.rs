//! Shared type definitions for the Brain Platform.
pub use self::brain_state::BrainState;
pub use self::budget_dimension::BudgetDimension;
pub use self::confidence::Confidence;
pub use self::goal_priority::GoalPriority;
pub use self::goal_status::GoalStatus;

mod brain_state;
mod budget_dimension;
mod confidence;
mod goal_priority;
mod goal_status;
mod reasoning_result;

mod decision;
pub use self::decision::Decision; // Added in Phase 6 cognitive loop

pub use self::reasoning_result::{ExtractedEntity, InferredConstraint, ReasoningResult};
