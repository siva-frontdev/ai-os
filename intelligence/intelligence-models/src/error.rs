use intelligence_core::types::CapabilityKind;
use thiserror::Error;

pub type ModelsResult<T> = Result<T, ModelsError>;

#[derive(Debug, Error)]
pub enum ModelsError {
    #[error("model {0} already registered")]
    AlreadyRegistered(String),
    #[error("model {0} not found")]
    NotFound(String),
    #[error("capability not available: {0:?}")]
    CapabilityNotAvailable(CapabilityKind),
    #[error("registry error: {0}")]
    Registry(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn error_messages() {
        let e = ModelsError::NotFound("m1".into());
        assert!(e.to_string().contains("m1"));
    }
}
