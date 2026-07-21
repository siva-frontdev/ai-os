use intelligence_core::error::ModelError;
use thiserror::Error;

pub type CacheResult<T> = Result<T, CacheError>;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cache miss")]
    Miss,
    #[error("cache write error: {0}")]
    WriteError(String),
    #[error("cache eviction error: {0}")]
    EvictionError(String),
}

impl From<CacheError> for ModelError {
    fn from(e: CacheError) -> Self {
        match e {
            CacheError::Miss => ModelError::CacheReadFailed("cache miss".into()),
            CacheError::WriteError(d) => ModelError::CacheWriteFailed(d),
            CacheError::EvictionError(d) => ModelError::CacheWriteFailed(d),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn messages() {
        assert!(CacheError::Miss.to_string().contains("miss"));
    }
    #[test]
    fn from_cache_miss() {
        let err: ModelError = CacheError::Miss.into();
        assert!(matches!(err, ModelError::CacheReadFailed(_)));
    }
}
