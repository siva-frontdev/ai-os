use thiserror::Error;

pub type ResolutionResult<T> = Result<T, ResolutionError>;

#[derive(Debug, Error)]
pub enum ResolutionError {
    #[error("resolution pipeline exhausted: {0}")]
    PipelineExhausted(String),

    #[error("stage failed: {stage}: {detail}")]
    StageFailed { stage: String, detail: String },

    #[error("human approval required: {0}")]
    HumanApprovalRequired(String),

    #[error("human approval denied: {0}")]
    HumanApprovalDenied(String),

    #[error("unsafe provider requires approval: {0}")]
    UnsafeProvider(String),

    #[error("adapter generation failed: {0}")]
    AdapterGenerationFailed(String),

    #[error("discovery error: {0}")]
    Discovery(#[from] execution_discovery::DiscoveryError),

    #[error("adapter error: {0}")]
    Adapter(#[from] execution_adapter::AdapterError),

    #[error("cache error: {0}")]
    Cache(#[from] execution_cache::CacheError),

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("planner error: {0}")]
    Planner(#[from] execution_planner::PlannerError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}

impl From<ResolutionError> for execution_core::ExecutionError {
    fn from(e: ResolutionError) -> Self {
        match e {
            ResolutionError::PipelineExhausted(c) => Self::ResolutionExhausted { capability: c },
            ResolutionError::StageFailed { stage, detail } => {
                Self::ConfigurationError(format!("stage {stage} failed: {detail}"))
            }
            ResolutionError::HumanApprovalRequired(d) => Self::HumanApprovalRequired(d),
            ResolutionError::HumanApprovalDenied(d) => Self::HumanApprovalDenied(d),
            ResolutionError::UnsafeProvider(p) => Self::UnsafeProvider(p),
            ResolutionError::AdapterGenerationFailed(d) => Self::AdapterGenerationFailed {
                capability: "unknown".into(),
                detail: d,
            },
            ResolutionError::Discovery(inner) => inner.into(),
            ResolutionError::Adapter(inner) => inner.into(),
            ResolutionError::Cache(inner) => inner.into(),
            ResolutionError::Execution(inner) => inner,
            ResolutionError::Planner(inner) => Self::ConfigurationError(inner.to_string()),
            ResolutionError::Core(inner) => Self::Core(inner),
        }
    }
}
