use thiserror::Error;

pub type ProviderResult<T> = Result<T, ProviderError>;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider {0} already registered")]
    AlreadyRegistered(String),
    #[error("provider {0} not found")]
    NotFound(String),
    #[error("provider {0}: health check failed: {1}")]
    HealthCheckFailed(String, String),
    #[error("provider {0}: request failed: {1}")]
    RequestFailed(String, String),
    #[error("provider {0}: authentication failed")]
    Unauthorized(String),
    #[error("rate limited by provider {0}, retry after {retry_after_ms}ms")]
    RateLimited {
        provider: String,
        retry_after_ms: u64,
    },
    #[error("timeout waiting for provider {0} after {timeout_ms}ms")]
    Timeout { provider: String, timeout_ms: u64 },
    #[error("provider {0} returned server error {status}")]
    ServerError { provider: String, status: u16 },
    #[error("provider {0}: invalid response: {1}")]
    InvalidResponse(String, String),
    #[error("provider HTTP client error: {0}")]
    Http(#[from] reqwest::Error),
}

impl From<ProviderError> for intelligence_core::error::ModelError {
    fn from(e: ProviderError) -> Self {
        intelligence_core::error::ModelError::Io(e.to_string())
    }
}
