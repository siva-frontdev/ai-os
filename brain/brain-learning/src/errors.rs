use thiserror::Error;

#[derive(Debug, Error)]
pub enum LearningError {
    #[error("pattern extraction failed: {0}")]
    PatternExtractionFailed(String),
    #[error("consolidation failed: {0}")]
    ConsolidationFailed(String),
    #[error("no patterns found: {0}")]
    NoPatterns(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type LearningResult<T> = Result<T, LearningError>;
