use thiserror::Error;
pub type ToolsResult<T> = Result<T, ToolsError>;

#[derive(Debug, Error)]
pub enum ToolsError {
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("registry error: {0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message() {
        assert!(ToolsError::NotFound("x".into()).to_string().contains("x"));
    }
}
