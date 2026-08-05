//! Bootstrap errors.

use ai_os_runtime_api::RuntimeError;

/// A failure while building or starting the runtime set.
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    /// The configuration is invalid.
    #[error("invalid runtime configuration: {0}")]
    Config(String),
    /// A runtime failed to start (connection, discovery, transport).
    #[error("runtime startup failed: {0}")]
    Runtime(#[from] RuntimeError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error_messages_are_informative() {
        let err = BootstrapError::Config("unknown runtime type".into());
        assert!(err.to_string().contains("invalid runtime configuration"));
    }
}
