//! Provider error taxonomy.
//!
//! Every provider failure maps to one of these structured codes so the
//! runtime can relay it verbatim to LIFE (which may escalate to a human or
//! choose to retry). Providers **never** report success unless the external
//! service confirmed execution.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A provider-level failure with a stable machine code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderError {
    /// Stable machine code (e.g. `ConfigurationMissing`).
    pub code: String,
    /// Human-readable explanation safe to surface to a user.
    pub message: String,
    /// Whether retrying is likely to succeed.
    pub retryable: bool,
}

impl ProviderError {
    /// Credentials or configuration are absent.
    pub fn configuration_missing(message: impl Into<String>) -> Self {
        Self {
            code: "ConfigurationMissing".into(),
            message: message.into(),
            retryable: false,
        }
    }

    /// Credentials are present but no longer authorized (e.g. OAuth expired).
    pub fn authentication_required(message: impl Into<String>) -> Self {
        Self {
            code: "AuthenticationRequired".into(),
            message: message.into(),
            retryable: false,
        }
    }

    /// The provider is reachable but refusing service (down, quota, 5xx).
    pub fn provider_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: "ProviderUnavailable".into(),
            message: message.into(),
            retryable: true,
        }
    }

    /// The request timed out.
    pub fn timeout(message: impl Into<String>) -> Self {
        Self {
            code: "Timeout".into(),
            message: message.into(),
            retryable: true,
        }
    }

    /// The caller's input was rejected by the provider.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            code: "InvalidInput".into(),
            message: message.into(),
            retryable: false,
        }
    }

    /// The provider returned a malformed or unexpected response.
    pub fn unexpected_response(message: impl Into<String>) -> Self {
        Self {
            code: "ProviderError".into(),
            message: message.into(),
            retryable: false,
        }
    }

    /// Serialize as a structured tool-result payload.
    pub fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "error": {
                "code": self.code,
                "message": self.message,
                "retryable": self.retryable,
            }
        })
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ProviderError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_maps_expected_codes() {
        assert_eq!(
            ProviderError::configuration_missing("x").code,
            "ConfigurationMissing"
        );
        assert_eq!(
            ProviderError::authentication_required("x").code,
            "AuthenticationRequired"
        );
        assert_eq!(
            ProviderError::provider_unavailable("x").code,
            "ProviderUnavailable"
        );
        assert_eq!(ProviderError::timeout("x").code, "Timeout");
        assert_eq!(ProviderError::invalid_input("x").code, "InvalidInput");
        assert_eq!(
            ProviderError::unexpected_response("x").code,
            "ProviderError"
        );
    }

    #[test]
    fn retryable_flag_distinguishes_transient_failures() {
        assert!(!ProviderError::configuration_missing("x").retryable);
        assert!(!ProviderError::authentication_required("x").retryable);
        assert!(ProviderError::provider_unavailable("x").retryable);
        assert!(ProviderError::timeout("x").retryable);
        assert!(!ProviderError::invalid_input("x").retryable);
    }

    #[test]
    fn json_payload_is_structured() {
        let err = ProviderError::authentication_required("oauth token expired");
        let json = err.as_json();
        assert_eq!(json["error"]["code"], "AuthenticationRequired");
        assert_eq!(json["error"]["message"], "oauth token expired");
        assert_eq!(json["error"]["retryable"], false);
    }
}
