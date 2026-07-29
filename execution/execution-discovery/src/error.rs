use thiserror::Error;

pub type DiscoveryResult<T> = Result<T, DiscoveryError>;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("local discovery failed: {0}")]
    LocalDiscoveryFailed(String),

    #[error("external discovery failed: {0}")]
    ExternalDiscoveryFailed(String),

    #[error("provider not found: {0}")]
    ProviderNotFound(String),

    #[error("runtime check failed: {0}")]
    RuntimeCheckFailed(String),

    #[error("command execution failed: {cmd}: {detail}")]
    CommandFailed { cmd: String, detail: String },

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}

impl From<DiscoveryError> for execution_core::ExecutionError {
    fn from(e: DiscoveryError) -> Self {
        match e {
            DiscoveryError::LocalDiscoveryFailed(d) => Self::LocalDiscoveryFailed {
                capability: "unknown".into(),
                detail: d,
            },
            DiscoveryError::ExternalDiscoveryFailed(d) => Self::ExternalDiscoveryFailed {
                capability: "unknown".into(),
                detail: d,
            },
            DiscoveryError::ProviderNotFound(p) => Self::ProviderNotFound(p),
            DiscoveryError::RuntimeCheckFailed(d) => Self::RuntimeNotAvailable(d),
            DiscoveryError::CommandFailed { cmd, detail } => {
                Self::ProcessSpawnFailed(format!("{cmd}: {detail}"))
            }
            DiscoveryError::Execution(inner) => inner,
            DiscoveryError::Core(inner) => Self::Core(inner),
        }
    }
}
