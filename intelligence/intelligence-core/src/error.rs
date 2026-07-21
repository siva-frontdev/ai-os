use crate::types::*;
use thiserror::Error;

pub type ModelResult<T> = Result<T, ModelError>;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("connection failed to provider {provider}: {detail}")]
    ConnectionFailed { provider: String, detail: String },
    #[error("authentication failed for provider {provider}")]
    AuthenticationFailed { provider: String },
    #[error("rate limited by provider {provider}, retry after {retry_after_ms}ms")]
    RateLimited {
        provider: String,
        retry_after_ms: u64,
    },
    #[error("timeout after {timeout_ms}ms for provider {provider}")]
    Timeout { provider: String, timeout_ms: u64 },
    #[error("server error {status} from provider {provider}")]
    ServerError { provider: String, status: u16 },
    #[error("invalid response from provider {provider}: {detail}")]
    InvalidResponse { provider: String, detail: String },

    #[error("model not found: {0}")]
    ModelNotFound(ModelId),
    #[error("capability mismatch: required {0:?}, available {1:?}")]
    CapabilityMismatch(CapabilityKind, Vec<CapabilityKind>),
    #[error("context window exceeded")]
    ContextWindowExceeded,
    #[error("content filtered: {0}")]
    ContentFiltered(String),
    #[error("safety violation")]
    SafetyViolation,

    #[error("no capable model for capability {0:?}")]
    NoCapableModel(CapabilityKind),
    #[error("all providers unavailable for capability {0:?}")]
    AllProvidersUnavailable(CapabilityKind),
    #[error("no fallback available")]
    NoFallbackAvailable,

    #[error("template not found")]
    TemplateNotFound,
    #[error("render error: {0}")]
    RenderError(String),
    #[error("missing variable {0}")]
    VariableMissing(String),
    #[error("context too long")]
    ContextTooLong,

    #[error("safety rule violation")]
    SafetyRuleViolation,
    #[error("safety framework error: {0}")]
    SafetyFrameworkError(String),

    #[error("budget exceeded: limit {limit}, current {current}")]
    BudgetExceeded { limit: f64, current: f64 },
    #[error("quota exceeded for period {0:?}")]
    QuotaExceeded(CostPeriod),

    #[error("cache write failed: {0}")]
    CacheWriteFailed(String),
    #[error("cache read failed: {0}")]
    CacheReadFailed(String),

    #[error("io: {0}")]
    Io(String),
    #[error("serialization: {0}")]
    Serialization(String),
}
