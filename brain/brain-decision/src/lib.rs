#![forbid(unsafe_code)]

pub mod decision;
pub mod errors;
pub mod types;

pub use decision::DecisionMaker;
pub use errors::{DecisionError, DecisionResult};
pub use types::{
    ConfidenceScore, ConflictDescription, ConflictSeverity, Decision, Explanation,
    ResolutionStrategy,
};

#[cfg(test)]
mod tests;
