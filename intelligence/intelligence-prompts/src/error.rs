use intelligence_core::error::ModelError;
use thiserror::Error;

pub type PromptsResult<T> = Result<T, PromptsError>;

#[derive(Debug, Error)]
pub enum PromptsError {
    #[error("template not found: {0}")]
    TemplateNotFound(String),
    #[error("missing variable: {0}")]
    MissingVariable(String),
    #[error("render error: {0}")]
    RenderError(String),
    #[error("other: {0}")]
    Other(String),
}

impl From<PromptsError> for ModelError {
    fn from(e: PromptsError) -> Self {
        match e {
            PromptsError::TemplateNotFound(id) => {
                ModelError::RenderError(format!("template not found: {}", id))
            }
            PromptsError::MissingVariable(v) => ModelError::VariableMissing(v),
            PromptsError::RenderError(d) => ModelError::RenderError(d),
            PromptsError::Other(d) => ModelError::RenderError(d),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_messages() {
        let e = PromptsError::TemplateNotFound("t1".into());
        assert!(e.to_string().contains("t1"));
    }
}
