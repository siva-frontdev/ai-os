use thiserror::Error;

pub type SandboxResult<T> = Result<T, SandboxError>;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("sandbox profile not found: {0}")]
    ProfileNotFound(String),

    #[error("isolation level unsupported: {0}")]
    IsolationUnsupported(String),

    #[error("permission denied for tool {tool}: {permission} — {detail}")]
    PermissionDenied {
        tool: String,
        permission: String,
        detail: String,
    },

    #[error("capability denied for tool {tool}: {capability}")]
    CapabilityDenied { tool: String, capability: String },

    #[error("filesystem restriction for path {path}: {reason}")]
    FilesystemRestriction { path: String, reason: String },

    #[error("environment variable filtered: {key}")]
    EnvironmentFiltered { key: String },

    #[error("constraint violation: {constraint} = {value}")]
    ConstraintViolation { constraint: String, value: String },

    #[error("lock poisoned")]
    LockPoisoned,

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}

impl From<SandboxError> for execution_core::ExecutionError {
    fn from(e: SandboxError) -> Self {
        match e {
            SandboxError::ProfileNotFound(p) => Self::ProfileNotFound(p),
            SandboxError::IsolationUnsupported(m) => Self::IsolationUnsupported(m),
            SandboxError::PermissionDenied { .. } => Self::PermissionDenied(e.to_string()),
            SandboxError::CapabilityDenied { .. } => Self::PermissionDenied(e.to_string()),
            SandboxError::FilesystemRestriction { .. } => Self::PermissionDenied(e.to_string()),
            SandboxError::EnvironmentFiltered { .. } => Self::PermissionDenied(e.to_string()),
            SandboxError::ConstraintViolation { .. } => Self::BudgetExceeded(e.to_string()),
            SandboxError::LockPoisoned => Self::SandboxCreationFailed("lock poisoned".into()),
            SandboxError::Execution(inner) => inner,
            SandboxError::Core(inner) => Self::Core(inner),
        }
    }
}
