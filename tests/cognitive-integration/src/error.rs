use serde::{Deserialize, Serialize};

/// Errors from the cognitive-runtime integration layer.
#[derive(Debug, Serialize, Deserialize, thiserror::Error)]
pub enum IntegrationError {
    #[error("runtime error: {0}")]
    Runtime(String),

    #[error("capability '{0}' not found on any runtime")]
    CapabilityNotFound(String),

    #[error("plan action failed: {0}")]
    ActionFailed(String),

    #[error("observation source error: {0}")]
    Observation(String),

    #[error("bridge not initialized")]
    NotInitialized,

    #[error("cognitive tick failed: {0}")]
    CognitiveTick(String),
}
