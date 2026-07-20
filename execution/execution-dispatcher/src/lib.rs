#![forbid(unsafe_code)]

pub mod dispatch;
pub mod error;

pub use dispatch::{DefaultDispatcher, DispatchPolicy, QueuedPlan};
pub use error::{DispatchError, DispatchResult};
