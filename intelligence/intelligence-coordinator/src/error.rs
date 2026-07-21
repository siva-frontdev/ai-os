use thiserror::Error;

pub type CoordinatorResult<T> = Result<T, CoordinatorError>;

#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error("coordinator not running")]
    NotRunning,
    #[error("pipeline channel closed: {0}")]
    ChannelClosed(String),
    #[error("lifecycle conflict: {0}")]
    LifecycleConflict(String),
    #[error("stage error: {0}")]
    Stage(String),
    #[error("no capable model for {0:?}")]
    NoCapableModel(intelligence_core::types::CapabilityKind),
}
