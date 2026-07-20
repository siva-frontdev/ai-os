#![forbid(unsafe_code)]

pub mod errors;
pub mod knowledge_base;
pub mod learning_engine;
pub mod pattern_recognizer;
pub mod types;

pub use errors::{LearningError, LearningResult};
pub use knowledge_base::KnowledgeBase;
pub use learning_engine::LearningEngine;
pub use pattern_recognizer::PatternRecognizer;
pub use types::{
    ConsolidationReport, Knowledge, LearningSignal, Pattern,
};

#[cfg(test)]
mod tests;
