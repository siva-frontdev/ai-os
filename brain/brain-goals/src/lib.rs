#![forbid(unsafe_code)]

pub mod dag;
pub mod errors;
pub mod manager;
pub mod memory_store;
pub mod store;
pub mod types;
pub mod validator;

pub use dag::GoalDag;
pub use errors::{GoalsError, GoalsResult};
pub use manager::GoalManager;
pub use memory_store::InMemoryGoalStore;
pub use store::GoalStore;
pub use types::GoalRecord;
pub use validator::GoalValidator;

#[cfg(test)]
mod tests;
