#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod coordinator;
pub mod error;
pub mod recovery_adapter;

pub use coordinator::DefaultCoordinator;
pub use error::{CoordinatorError, CoordinatorResult};
