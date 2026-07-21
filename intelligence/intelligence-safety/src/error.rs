use thiserror::Error;

#[derive(Debug, Error)]
pub enum SafetyEnforcerError {
    #[error("pii detected")]
    PiiDetected,
    #[error("prompt injection detected")]
    PromptInjection,
    #[error("rule {0} not found")]
    RuleNotFound(String),
    #[error("boolean context timeout")]
    ContextTimeout,
    #[error("regex error: {0}")]
    Regex(String),
    #[error("{0}")]
    Other(String),
}
