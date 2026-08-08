//! Guided provider setup.
//!
//! Every external provider exposes a [`ProviderSetup`] implementation that
//! runs its own OAuth/credential flow and persists the result to a shared
//! gitignored env file. The wizard plumbing — localhost callback server,
//! browser opening, token exchange, env persistence, structured errors — is
//! shared across providers so `life setup whatsapp|github|calendar|telegram`
//! follow the same pattern without redesign.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::SetupError;

pub mod env_file;
pub mod gmail;
pub mod oauth;
pub mod whatsapp;

/// The file the wizard persists credentials into.
pub const DEFAULT_ENV_FILE: &str = "runtime/config/.env";

/// The default host the localhost callback server binds to.
pub const DEFAULT_CALLBACK_HOST: &str = "127.0.0.1";

/// The default port the localhost callback server binds to.
pub const DEFAULT_CALLBACK_PORT: u16 = 8765;

/// How long to wait for the authorization callback before failing.
pub const DEFAULT_TIMEOUT_SECS: u64 = 300;

/// A guided, one-shot setup procedure for an external provider.
///
/// Implementations are stateless; the [`SetupContext`] carries every
/// parameter the run needs. This keeps the trait dyn-compatible and lets the
/// CLI drive any provider uniformly.
#[async_trait]
pub trait ProviderSetup: std::fmt::Debug + Send + Sync {
    /// Stable provider name (`gmail`, `whatsapp`, ...). Also the CLI token.
    fn name(&self) -> &'static str;

    /// Run the full guided setup and persist credentials on success.
    async fn setup(&self, ctx: &SetupContext) -> Result<(), SetupError>;
}

/// Everything a single `life setup <provider>` run needs.
#[derive(Debug, Clone)]
pub struct SetupContext {
    /// Where credentials are persisted (a gitignored `.env` file).
    pub env_file: PathBuf,
    /// Host the localhost callback server binds to.
    pub callback_host: String,
    /// Port the localhost callback server binds to.
    pub callback_port: u16,
    /// Seconds to wait for the authorization callback.
    pub timeout_secs: u64,
    /// Validate the resulting configuration against the live provider.
    pub verify: bool,
    /// Optional recipient for a real test message after validation.
    pub test_recipient: Option<String>,
    /// Re-run the flow even if a refresh token is already stored.
    pub force: bool,
}

impl Default for SetupContext {
    fn default() -> Self {
        Self {
            env_file: PathBuf::from(DEFAULT_ENV_FILE),
            callback_host: DEFAULT_CALLBACK_HOST.into(),
            callback_port: DEFAULT_CALLBACK_PORT,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            verify: true,
            test_recipient: None,
            force: false,
        }
    }
}

/// The default provider registry for `life setup`.
///
/// New providers register here by adding a [`ProviderSetup`] implementation
/// and pushing it into this list.
pub fn providers() -> Vec<Arc<dyn ProviderSetup>> {
    vec![
        Arc::new(gmail::GmailSetup::new()),
        Arc::new(whatsapp::WhatsAppSetup::new()),
    ]
}

/// Find a registered provider by its CLI name.
pub fn find_provider(name: &str) -> Option<Arc<dyn ProviderSetup>> {
    providers().into_iter().find(|p| p.name() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_is_registered_by_name() {
        let provider = find_provider("gmail").expect("gmail registered");
        assert_eq!(provider.name(), "gmail");
    }

    #[test]
    fn whatsapp_is_registered_by_name() {
        let provider = find_provider("whatsapp").expect("whatsapp registered");
        assert_eq!(provider.name(), "whatsapp");
    }

    #[test]
    fn unknown_provider_is_not_found() {
        assert!(find_provider("teleport").is_none());
    }

    #[test]
    fn default_context_points_at_runtime_config() {
        let ctx = SetupContext::default();
        assert_eq!(ctx.env_file, PathBuf::from(DEFAULT_ENV_FILE));
        assert_eq!(ctx.callback_port, DEFAULT_CALLBACK_PORT);
    }
}
