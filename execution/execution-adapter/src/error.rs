use thiserror::Error;

pub type AdapterResult<T> = Result<T, AdapterError>;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("adapter generation failed: {0}")]
    GenerationFailed(String),

    #[error("unsupported adapter type: {0}")]
    UnsupportedType(String),

    #[error("invalid synthesis request: {0}")]
    InvalidRequest(String),

    #[error("required runtime not available: {0}")]
    RuntimeNotAvailable(String),

    #[error("template rendering failed: {0}")]
    TemplateRenderingFailed(String),

    #[error("execution error: {0}")]
    Execution(#[from] execution_core::ExecutionError),

    #[error("core error: {0}")]
    Core(#[from] ai_os_core::CoreError),
}

impl From<AdapterError> for execution_core::ExecutionError {
    fn from(e: AdapterError) -> Self {
        match e {
            AdapterError::GenerationFailed(d) => Self::AdapterGenerationFailed {
                capability: "unknown".into(),
                detail: d,
            },
            AdapterError::UnsupportedType(t) => Self::UnsupportedAdapterType(t),
            AdapterError::InvalidRequest(d) => Self::InvalidSynthesisRequest(d),
            AdapterError::RuntimeNotAvailable(r) => Self::RuntimeNotAvailable(r),
            AdapterError::TemplateRenderingFailed(d) => Self::AdapterGenerationFailed {
                capability: "unknown".into(),
                detail: d,
            },
            AdapterError::Execution(inner) => inner,
            AdapterError::Core(inner) => Self::Core(inner),
        }
    }
}
