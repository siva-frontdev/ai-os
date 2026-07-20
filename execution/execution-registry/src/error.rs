use thiserror::Error;

pub type RegistryResult<T> = Result<T, RegistryError>;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("tool already registered: {0}")]
    AlreadyRegistered(String),

    #[error("tool not found: {0}")]
    NotFound(String),

    #[error("capability mismatch: {0}")]
    CapabilityMismatch(String),

    #[error("no tool satisfies capability {0}")]
    NoToolForCapability(String),

    #[error("capability index inconsistency: {0}")]
    IndexInconsistency(String),

    #[error("lock poisoned: {0}")]
    LockPoisoned(String),

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),
}

impl RegistryError {
    pub fn not_found<T: Into<String>>(msg: T) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn capability_mismatch<T: Into<String>>(msg: T) -> Self {
        Self::CapabilityMismatch(msg.into())
    }
}
