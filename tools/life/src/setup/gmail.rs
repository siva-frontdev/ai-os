//! Guided Gmail OAuth2 setup.
//!
//! The flow:
//!
//! 1. Require `AIOS_GMAIL_CLIENT_ID` / `AIOS_GMAIL_CLIENT_SECRET` (from the
//!    env file or the process environment).
//! 2. Build a Google consent URL (`access_type=offline`, `prompt=consent`),
//!    open it in the default browser, and run a localhost callback server to
//!    capture the authorization code.
//! 3. Exchange the code for a refresh token at the Google token endpoint.
//! 4. Persist `AIOS_GMAIL_CLIENT_ID`, `AIOS_GMAIL_CLIENT_SECRET` and
//!    `AIOS_GMAIL_REFRESH_TOKEN` to the env file. The short-lived access
//!    token is **never** persisted.
//! 5. Validate the stored configuration against the live Gmail API
//!    ([`ai_os_plugins::provider::GmailProvider::validate`]) and optionally
//!    send a real test message.
//!
//! When a refresh token is already stored and `--force` was not passed, the
//! OAuth dance is skipped and the run goes straight to validation.

use std::env;
use std::path::Path;
use std::sync::Arc;

use ai_os_plugins::provider::{GmailConfig, GmailProvider, Providers};
use async_trait::async_trait;

use crate::error::SetupError;
use crate::setup::env_file::EnvFile;
use crate::setup::oauth::{
    build_google_auth_url, exchange_code, open_browser, CallbackServer, DEFAULT_CALLBACK_PATH,
};
use crate::setup::{ProviderSetup, SetupContext};

/// The OAuth scopes requested during consent.
///
/// `gmail.send` grants the ability to send mail. `gmail.metadata` grants
/// read access to message metadata (headers and labels, not body content);
/// it is required by [`GmailProvider::verify_sent`] which re-fetches the
/// sent message to confirm the `SENT` label before reporting success.
const GMAIL_SCOPE: &str = concat!(
    "https://www.googleapis.com/auth/gmail.send ",
    "https://www.googleapis.com/auth/gmail.metadata"
);

/// The Google OAuth2 token endpoint.
const GOOGLE_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";

/// The Gmail API base URL.
const GOOGLE_GMAIL_API_BASE: &str = "https://gmail.googleapis.com";

/// Environment variable for the Google OAuth2 client id.
pub const GMAIL_CLIENT_ID_ENV: &str = "AIOS_GMAIL_CLIENT_ID";
/// Environment variable for the Google OAuth2 client secret.
pub const GMAIL_CLIENT_SECRET_ENV: &str = "AIOS_GMAIL_CLIENT_SECRET";
/// Environment variable for the long-lived Google refresh token.
pub const GMAIL_REFRESH_TOKEN_ENV: &str = "AIOS_GMAIL_REFRESH_TOKEN";

/// Opens the consent URL in the user's browser. Swappable in tests.
pub type BrowserOpener = Arc<dyn Fn(&str) -> Result<(), SetupError> + Send + Sync>;

/// The guided Gmail setup wizard.
pub struct GmailSetup {
    browser: BrowserOpener,
}

impl std::fmt::Debug for GmailSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GmailSetup").finish_non_exhaustive()
    }
}

impl GmailSetup {
    /// Create a new Gmail setup wizard using the system browser.
    pub fn new() -> Self {
        Self {
            browser: Arc::new(open_browser),
        }
    }

    /// Create a wizard with a custom browser opener (for tests).
    pub fn with_browser(opener: BrowserOpener) -> Self {
        Self { browser: opener }
    }
}

impl Default for GmailSetup {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderSetup for GmailSetup {
    fn name(&self) -> &'static str {
        "gmail"
    }

    async fn setup(&self, ctx: &SetupContext) -> Result<(), SetupError> {
        let mut env = EnvFile::load(&ctx.env_file)?;
        let (client_id, client_secret) = resolve_client_credentials(&env)?;

        let needs_oauth = ctx.force || env.get(GMAIL_REFRESH_TOKEN_ENV).is_none();
        if needs_oauth {
            let refresh_token =
                run_oauth_flow(ctx, &client_id, &client_secret, &self.browser).await?;
            env.set(GMAIL_CLIENT_ID_ENV, &client_id);
            env.set(GMAIL_CLIENT_SECRET_ENV, &client_secret);
            env.set(GMAIL_REFRESH_TOKEN_ENV, &refresh_token);
            env.write(&ctx.env_file)?;
        }

        if ctx.verify {
            validate_and_verify(ctx, &env).await?;
        }
        Ok(())
    }
}

/// Resolve client id/secret from the env file or the process environment.
fn resolve_client_credentials(env: &EnvFile) -> Result<(String, String), SetupError> {
    let client_id = env
        .get(GMAIL_CLIENT_ID_ENV)
        .map(str::to_string)
        .or_else(|| env::var(GMAIL_CLIENT_ID_ENV).ok())
        .filter(|s| !s.is_empty());
    let client_secret = env
        .get(GMAIL_CLIENT_SECRET_ENV)
        .map(str::to_string)
        .or_else(|| env::var(GMAIL_CLIENT_SECRET_ENV).ok())
        .filter(|s| !s.is_empty());

    match (client_id, client_secret) {
        (Some(id), Some(secret)) => Ok((id, secret)),
        _ => Err(SetupError::missing_credentials(
            "Gmail OAuth client credentials are required. Set AIOS_GMAIL_CLIENT_ID and \
             AIOS_GMAIL_CLIENT_SECRET in the env file or the environment. See \
             docs/runtime/gmail-setup.md for how to create a Google OAuth client.",
        )),
    }
}

/// Run the browser consent flow and return a refresh token.
async fn run_oauth_flow(
    ctx: &SetupContext,
    client_id: &str,
    client_secret: &str,
    browser: &BrowserOpener,
) -> Result<String, SetupError> {
    let mut server =
        CallbackServer::bind(&ctx.callback_host, ctx.callback_port, DEFAULT_CALLBACK_PATH).await?;
    let port = server.port();
    let redirect_uri = server.redirect_uri(&ctx.callback_host, port);

    let state = uuid::Uuid::new_v4().to_string();
    let auth_url = build_google_auth_url(client_id, &redirect_uri, GMAIL_SCOPE, &state)?;

    println!(
        "Opening the browser to authorize AI-OS with Gmail...\n\
         If it does not open automatically, paste this URL into your browser:\n{auth_url}\n"
    );
    match browser(&auth_url) {
        Ok(()) => {}
        Err(_) => println!("(browser open failed — use the URL above)"),
    }

    let code = server.wait_for_code(ctx.timeout_secs, &state).await?;

    let http = reqwest::Client::new();
    let token_uri =
        env::var("AIOS_GMAIL_TOKEN_URI").unwrap_or_else(|_| GOOGLE_TOKEN_URI.to_string());
    let tokens = exchange_code(
        &http,
        &token_uri,
        client_id,
        client_secret,
        &code,
        &redirect_uri,
    )
    .await?;
    Ok(tokens.refresh_token)
}

/// Build a Gmail config from env-file values plus process-env overrides.
fn build_gmail_config(env: &EnvFile) -> Result<GmailConfig, SetupError> {
    let client_id = env
        .get(GMAIL_CLIENT_ID_ENV)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| SetupError::validation("AIOS_GMAIL_CLIENT_ID not stored"))?;
    let client_secret = env
        .get(GMAIL_CLIENT_SECRET_ENV)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| SetupError::validation("AIOS_GMAIL_CLIENT_SECRET not stored"))?;
    let refresh_token = env
        .get(GMAIL_REFRESH_TOKEN_ENV)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| SetupError::validation("AIOS_GMAIL_REFRESH_TOKEN not stored"))?;

    Ok(GmailConfig {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        refresh_token: refresh_token.to_string(),
        user: env::var("AIOS_GMAIL_USER").unwrap_or_else(|_| "me".into()),
        token_uri: env::var("AIOS_GMAIL_TOKEN_URI")
            .unwrap_or_else(|_| GOOGLE_TOKEN_URI.to_string()),
        api_base: env::var("AIOS_GMAIL_API_BASE")
            .unwrap_or_else(|_| GOOGLE_GMAIL_API_BASE.to_string()),
    })
}

/// Validate the stored config by exchanging the refresh token (proves
/// credentials are valid) and optionally send a real test message.
async fn validate_and_verify(ctx: &SetupContext, env: &EnvFile) -> Result<(), SetupError> {
    let config = build_gmail_config(env)?;
    let provider = GmailProvider::new(config);
    provider
        .check_credentials()
        .await
        .map_err(|e| SetupError::from_provider(&e))?;
    println!("Gmail credentials valid");

    if let Some(recipient) = &ctx.test_recipient {
        let sender = provider.user().to_string();
        let receipt = provider
            .send(
                recipient,
                "AI-OS setup confirmation",
                "This email confirms AI-OS can send mail through your Gmail account.",
                &sender,
            )
            .await
            .map_err(|e| SetupError::from_provider(&e))?;
        println!(
            "Test email sent to {recipient} (message {})",
            receipt.message_id
        );
    }
    Ok(())
}

/// Convenience: load provider configs as the plugin binary would, so `life`
/// can confirm what the runtime will see.
pub fn configured_providers(env_file: &Path) -> Providers {
    let env = EnvFile::load(env_file).ok();
    let gmail = env.and_then(|e| build_gmail_config(&e).ok());
    Providers {
        gmail,
        whatsapp: None,
    }
}

/// The provider instance the plugin runtime will construct from `env_file`.
pub fn gmail_provider(env_file: &Path) -> Option<Arc<GmailProvider>> {
    configured_providers(env_file)
        .gmail
        .map(GmailProvider::new)
        .map(Arc::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_credentials_from_env_file() {
        let mut env = EnvFile::default();
        env.set(GMAIL_CLIENT_ID_ENV, "cid");
        env.set(GMAIL_CLIENT_SECRET_ENV, "csecret");
        let (id, secret) = resolve_client_credentials(&env).unwrap();
        assert_eq!(id, "cid");
        assert_eq!(secret, "csecret");
    }

    #[test]
    fn resolve_credentials_errors_when_absent() {
        let env = EnvFile::default();
        let err = resolve_client_credentials(&env).unwrap_err();
        assert_eq!(err.code, "ConfigurationMissing");
    }

    #[test]
    fn build_gmail_config_requires_all_pieces() {
        let mut env = EnvFile::default();
        env.set(GMAIL_CLIENT_ID_ENV, "cid");
        env.set(GMAIL_CLIENT_SECRET_ENV, "csecret");
        let err = build_gmail_config(&env).unwrap_err();
        assert_eq!(err.message, "AIOS_GMAIL_REFRESH_TOKEN not stored");
    }

    #[test]
    fn configured_providers_reflects_env_file() {
        let dir = std::env::temp_dir().join(format!("life-cfg-{}", uuid::Uuid::new_v4()));
        let path = dir.join(".env");
        let mut env = EnvFile::default();
        env.set(GMAIL_CLIENT_ID_ENV, "cid");
        env.set(GMAIL_CLIENT_SECRET_ENV, "csecret");
        env.set(GMAIL_REFRESH_TOKEN_ENV, "rtok");
        env.write(&path).unwrap();
        let providers = configured_providers(&path);
        assert!(providers.gmail.is_some());
        assert!(gmail_provider(&path).is_some());
        std::fs::remove_dir_all(&dir).ok();
    }
}
