use thiserror::Error;
pub type StreamingResult<T> = Result<T, StreamingError>;

#[derive(Debug, Error)]
pub enum StreamingError {
    #[error("stream ended unexpectedly")]
    UnexpectedEnd,
    #[error("chunk decode error: {0}")]
    DecodeError(String),
    #[error("other: {0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn messages() {
        assert!(StreamingError::UnexpectedEnd
            .to_string()
            .contains("unexpectedly"));
    }
}
