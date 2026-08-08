//! Structured setup errors.
//!
//! Every failure during `life setup <provider>` maps to a `SetupError`
//! variant with a stable code. The CLI serializes these to the same
//! structured shape the runtime uses for provider failures, so LIFE can
//! surface (and optionally escalate) them without guessing.

use ai_os_plugins::provider::ProviderError;

/// A failure during the guided provider setup flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupError {
    /// Stable machine code (e.g. `ConfigurationMissing`).
    pub code: String,
    /// Human-readable explanation safe to surface to a user.
    pub message: String,
    /// Whether retrying is likely to succeed.
    pub retryable: bool,
}

impl SetupError {
    /// Credentials required to start the flow are absent.
    pub fn missing_credentials(message: impl Into<String>) -> Self {
        Self::from_code("ConfigurationMissing", message, false)
    }

    /// The localhost callback server could not be started or served.
    pub fn callback_server(message: impl Into<String>) -> Self {
        Self::from_code("ProviderUnavailable", message, true)
    }

    /// The browser could not be opened to the consent screen.
    pub fn browser(message: impl Into<String>) -> Self {
        Self::from_code("ProviderUnavailable", message, true)
    }

    /// The user (or Google) denied the consent request.
    pub fn access_denied(message: impl Into<String>) -> Self {
        Self::from_code("AuthenticationRequired", message, false)
    }

    /// The authorization callback did not arrive in time.
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::from_code("Timeout", message, true)
    }

    /// The token endpoint rejected the exchange (bad creds or code).
    pub fn token_exchange(message: impl Into<String>) -> Self {
        Self::from_code("AuthenticationRequired", message, false)
    }

    /// The network request to the token endpoint failed.
    pub fn token_network(message: impl Into<String>) -> Self {
        Self::from_code("ProviderUnavailable", message, true)
    }

    /// Credentials could not be persisted.
    pub fn persist(message: impl Into<String>) -> Self {
        Self::from_code("ProviderError", message, false)
    }

    /// The provider rejected the resulting configuration.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::from_code("ProviderError", message, false)
    }

    /// Lift a provider error through the setup layer unchanged.
    pub fn from_provider(error: &ProviderError) -> Self {
        Self {
            code: error.code.clone(),
            message: error.message.clone(),
            retryable: error.retryable,
        }
    }

    /// An unexpected internal condition.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::from_code("ProviderError", message, false)
    }

    fn from_code(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }

    /// Convert into the provider error shape used across the runtime.
    pub fn as_provider_error(&self) -> ProviderError {
        ProviderError {
            code: self.code.clone(),
            message: self.message.clone(),
            retryable: self.retryable,
        }
    }

    /// Serialize as a structured tool-result payload.
    pub fn as_json(&self) -> serde_json::Value {
        self.as_provider_error().as_json()
    }
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for SetupError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_maps_expected_codes() {
        assert_eq!(
            SetupError::missing_credentials("x").code,
            "ConfigurationMissing"
        );
        assert_eq!(SetupError::callback_server("x").code, "ProviderUnavailable");
        assert_eq!(
            SetupError::access_denied("x").code,
            "AuthenticationRequired"
        );
        assert_eq!(
            SetupError::token_exchange("x").code,
            "AuthenticationRequired"
        );
        assert_eq!(SetupError::timeout("x").code, "Timeout");
        assert_eq!(SetupError::persist("x").code, "ProviderError");
    }

    #[test]
    fn retryable_flag_distinguishes_transient_failures() {
        assert!(!SetupError::missing_credentials("x").retryable);
        assert!(SetupError::callback_server("x").retryable);
        assert!(SetupError::token_network("x").retryable);
        assert!(!SetupError::access_denied("x").retryable);
        assert!(!SetupError::validation("x").retryable);
    }

    #[test]
    fn json_payload_is_structured() {
        let err = SetupError::access_denied("user said no");
        let json = err.as_json();
        assert_eq!(json["error"]["code"], "AuthenticationRequired");
        assert_eq!(json["error"]["message"], "user said no");
        assert_eq!(json["error"]["retryable"], false);
    }

    #[test]
    fn provider_conversion_preserves_code() {
        let err = SetupError::missing_credentials("set creds");
        let provider = err.as_provider_error();
        assert_eq!(provider.code, "ConfigurationMissing");
        assert_eq!(provider.message, "set creds");
    }
}
