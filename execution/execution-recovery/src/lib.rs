#![forbid(unsafe_code)]

//! # execution-recovery
//!
//! Handles retry management and rollback planning for failed executions.

pub mod error;
pub mod retry;
pub mod rollback;

pub use error::{RecoveryError, RecoveryResult};
pub use retry::{DefaultRetryManager, RetryDecision, RetryManager};
pub use rollback::{DefaultRollbackManager, RollbackDecision, RollbackManager};
