#![forbid(unsafe_code)]

pub mod constraints;
pub mod engine;
pub mod errors;
pub mod hypothesis;
pub mod inference;
pub mod risk;
pub mod tradeoff;
pub mod types;

pub use constraints::ConstraintSolver;
pub use engine::ReasoningPipeline;
pub use errors::{ReasonerError, ReasonerResult};
pub use hypothesis::HypothesisGenerator;
pub use inference::InferenceEngine;
pub use risk::RiskAnalyzer;
pub use tradeoff::TradeoffAnalyzer;
pub use types::{
    Constraint, ConstraintSeverity, ConstraintType, Hypothesis, Inference, InferenceType,
    ReasoningResult, RiskAssessment, TradeoffAnalysis, TradeoffOption,
};

#[cfg(test)]
mod tests;
