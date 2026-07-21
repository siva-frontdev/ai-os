use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelemetrySinkError {
    #[error("send error: {0}")]
    SendError(String),
    #[error("batch error: {0}")]
    BatchError(String),
    #[error("export error: {0}")]
    ExportError(String),
    #[error("{0}")]
    Other(String),
}
