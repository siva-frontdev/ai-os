#![forbid(unsafe_code)]
#![warn(missing_docs)]
mod error;
pub mod tracker;
pub use error::CostError;
pub use intelligence_core::types::{BudgetCheck, CostReport};
pub use tracker::DefaultCostAccountant;
