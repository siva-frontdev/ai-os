//! Context structs passed through the cognitive loop.
//!
//! Each context carries the data a subsystem needs for its phase
//! of the cognitive loop, plus a `trace_id` for distributed tracing.

pub use self::decision::{ConflictRef, DecisionContext, OptionRef, PolicyRef};
pub use self::goal::GoalContext;
pub use self::planning::PlanningContext;
pub use self::reasoning::ReasoningContext;

mod decision;
mod goal;
mod planning;
mod reasoning;
