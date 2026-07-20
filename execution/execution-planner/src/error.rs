use thiserror::Error;

pub type PlannerResult<T> = Result<T, PlannerError>;

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error("no suitable candidate for capability: {0}")]
    NoSuitableCandidate(String),

    #[error("schema validation failed for {field}: {reason}")]
    SchemaValidationFailed { field: String, reason: String },

    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("sandbox resolution failed: {0}")]
    SandboxResolutionFailed(String),

    #[error("circular dependency detected in requirements")]
    DependencyCycleDetected,

    #[error("batch too large: {count} > {max}")]
    BatchTooLarge { count: usize, max: usize },

    #[error("invalid requirement: {0}")]
    InvalidRequirement(String),

    #[error("lock poisoned")]
    LockPoisoned,

    #[error("registry error: {0}")]
    Registry(#[from] execution_registry::RegistryError),

    #[error("sandbox error: {0}")]
    Sandbox(#[from] execution_sandbox::SandboxError),

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}
