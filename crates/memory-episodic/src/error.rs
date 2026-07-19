use std::fmt;
use uuid::Uuid;

/// Unified error type for episodic memory operations.
#[derive(Debug, thiserror::Error)]
pub enum EpisodicError {
    #[error("episode not found: {0}")]
    EpisodeNotFound(Uuid),

    #[error("session not found: {0}")]
    SessionNotFound(Uuid),

    #[error("storage backend error: {0}")]
    StorageBackendError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("invalid time range: {0}")]
    InvalidTimeRange(String),

    #[error("episode already exists: {0}")]
    EpisodeAlreadyExists(Uuid),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type EpisodicResult<T> = Result<T, EpisodicError>;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_variants_display() {
        let cases: Vec<EpisodicError> = vec![
            EpisodicError::EpisodeNotFound(Uuid::new_v4()),
            EpisodicError::SessionNotFound(Uuid::new_v4()),
            EpisodicError::StorageBackendError("x".into()),
            EpisodicError::SerializationError("y".into()),
            EpisodicError::InvalidTimeRange("z".into()),
            EpisodicError::EpisodeAlreadyExists(Uuid::new_v4()),
            EpisodicError::Internal("i".into()),
        ];
        for e in &cases {
            let _ = e.to_string();
        }
        assert_eq!(cases.len(), 7);
    }
}
