//! Guided WhatsApp Cloud API setup.
//!
//! Unlike Gmail there is no browser OAuth dance: WhatsApp uses a long-lived
//! system-user access token issued by Meta. Setup therefore:
//!
//! 1. Requires `AIOS_WHATSAPP_ACCESS_TOKEN` and `AIOS_WHATSAPP_PHONE_NUMBER_ID`
//!    (from the env file or the process environment).
//! 2. Persists them (plus the optional business account id and webhook verify
//!    token) to the env file.
//! 3. Validates the token against the live WhatsApp Cloud API
//!    ([`ai_os_plugins::provider::WhatsAppProvider::validate`]).
//!
//! A `--force` run simply re-persists and re-validates.

use std::env;
use std::path::Path;
use std::sync::Arc;

use ai_os_plugins::provider::{Providers, WhatsAppConfig, WhatsAppProvider};
use async_trait::async_trait;

use crate::error::SetupError;
use crate::setup::env_file::EnvFile;
use crate::setup::{ProviderSetup, SetupContext};

/// The WhatsApp Cloud API base URL.
const GRAPH_BASE: &str = "https://graph.facebook.com";

/// Environment variable for the WhatsApp Cloud API system-user token.
pub const WHATSAPP_ACCESS_TOKEN_ENV: &str = "AIOS_WHATSAPP_ACCESS_TOKEN";
/// Environment variable for the WhatsApp phone number id (sender).
pub const WHATSAPP_PHONE_NUMBER_ID_ENV: &str = "AIOS_WHATSAPP_PHONE_NUMBER_ID";
/// Environment variable for the WhatsApp Business Account id (reference).
pub const WHATSAPP_BUSINESS_ACCOUNT_ID_ENV: &str = "AIOS_WHATSAPP_BUSINESS_ACCOUNT_ID";
/// Environment variable for the WhatsApp webhook hub verify token.
pub const WHATSAPP_WEBHOOK_VERIFY_TOKEN_ENV: &str = "AIOS_WHATSAPP_WEBHOOK_VERIFY_TOKEN";

/// The guided WhatsApp setup.
#[derive(Debug, Clone, Default)]
pub struct WhatsAppSetup;

impl WhatsAppSetup {
    /// Create a new WhatsApp setup.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProviderSetup for WhatsAppSetup {
    fn name(&self) -> &'static str {
        "whatsapp"
    }

    async fn setup(&self, ctx: &SetupContext) -> Result<(), SetupError> {
        let mut env = EnvFile::load(&ctx.env_file)?;
        let (access_token, phone_number_id) = resolve_whatsapp_credentials(&env)?;

        env.set(WHATSAPP_ACCESS_TOKEN_ENV, &access_token);
        env.set(WHATSAPP_PHONE_NUMBER_ID_ENV, &phone_number_id);
        if let Some(business_account_id) = env::var(WHATSAPP_BUSINESS_ACCOUNT_ID_ENV)
            .ok()
            .filter(|s| !s.is_empty())
        {
            env.set(WHATSAPP_BUSINESS_ACCOUNT_ID_ENV, &business_account_id);
        }
        if let Some(verify_token) = env::var(WHATSAPP_WEBHOOK_VERIFY_TOKEN_ENV)
            .ok()
            .filter(|s| !s.is_empty())
        {
            env.set(WHATSAPP_WEBHOOK_VERIFY_TOKEN_ENV, &verify_token);
        }
        env.write(&ctx.env_file)?;

        if ctx.verify {
            validate_credentials(ctx, &env).await?;
        }
        Ok(())
    }
}

/// Resolve access token / phone number id from the env file or process env.
fn resolve_whatsapp_credentials(env: &EnvFile) -> Result<(String, String), SetupError> {
    let access_token = env
        .get(WHATSAPP_ACCESS_TOKEN_ENV)
        .map(str::to_string)
        .or_else(|| env::var(WHATSAPP_ACCESS_TOKEN_ENV).ok())
        .filter(|s| !s.is_empty());
    let phone_number_id = env
        .get(WHATSAPP_PHONE_NUMBER_ID_ENV)
        .map(str::to_string)
        .or_else(|| env::var(WHATSAPP_PHONE_NUMBER_ID_ENV).ok())
        .filter(|s| !s.is_empty());

    match (access_token, phone_number_id) {
        (Some(token), Some(id)) => Ok((token, id)),
        _ => Err(SetupError::missing_credentials(
            "WhatsApp Cloud API credentials are required. Set AIOS_WHATSAPP_ACCESS_TOKEN and \
             AIOS_WHATSAPP_PHONE_NUMBER_ID in the env file or the environment. See \
             docs/runtime/whatsapp-provider.md.",
        )),
    }
}

/// Build a WhatsApp config from env-file values plus process-env overrides.
fn build_whatsapp_config(env: &EnvFile) -> Result<WhatsAppConfig, SetupError> {
    let access_token = env
        .get(WHATSAPP_ACCESS_TOKEN_ENV)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| SetupError::validation("AIOS_WHATSAPP_ACCESS_TOKEN not stored"))?;
    let phone_number_id = env
        .get(WHATSAPP_PHONE_NUMBER_ID_ENV)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| SetupError::validation("AIOS_WHATSAPP_PHONE_NUMBER_ID not stored"))?;

    Ok(WhatsAppConfig {
        access_token: access_token.to_string(),
        phone_number_id: phone_number_id.to_string(),
        api_version: env::var("AIOS_WHATSAPP_API_VERSION").unwrap_or_else(|_| "v21.0".into()),
        graph_base: env::var("AIOS_WHATSAPP_GRAPH_BASE").unwrap_or_else(|_| GRAPH_BASE.into()),
        webhook_verify_token: env
            .get(WHATSAPP_WEBHOOK_VERIFY_TOKEN_ENV)
            .unwrap_or_default()
            .to_string(),
        app_secret: env::var("AIOS_WHATSAPP_APP_SECRET").unwrap_or_default(),
        webhook_host: env::var("AIOS_WHATSAPP_WEBHOOK_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
        webhook_port: env::var("AIOS_WHATSAPP_WEBHOOK_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(0),
        webhook_path: env::var("AIOS_WHATSAPP_WEBHOOK_PATH")
            .unwrap_or_else(|_| "/webhook/whatsapp".into()),
    })
}

/// Validate the stored config against the live WhatsApp Cloud API.
async fn validate_credentials(ctx: &SetupContext, env: &EnvFile) -> Result<(), SetupError> {
    let config = build_whatsapp_config(env)?;
    let provider = WhatsAppProvider::new(config);
    let phone = provider
        .validate()
        .await
        .map_err(|e| SetupError::validation(e.message.clone()))?;
    println!("WhatsApp credentials valid — phone number {phone}");

    if let Some(recipient) = &ctx.test_recipient {
        let receipt = provider
            .send(
                recipient,
                "AI-OS setup confirmation — WhatsApp delivery works.",
            )
            .await
            .map_err(|e| SetupError::validation(e.message.clone()))?;
        println!(
            "Test message sent to {recipient} (message {})",
            receipt.message_id
        );
    }
    Ok(())
}

/// Load provider configs as the plugin binary would, so `life` can confirm
/// what the runtime will see.
pub fn configured_providers(env_file: &Path) -> Providers {
    let env = EnvFile::load(env_file).ok();
    let whatsapp = env.and_then(|e| build_whatsapp_config(&e).ok());
    Providers {
        whatsapp,
        ..Default::default()
    }
}

/// The provider instance the plugin runtime will construct from `env_file`.
pub fn whatsapp_provider(env_file: &Path) -> Option<Arc<WhatsAppProvider>> {
    configured_providers(env_file)
        .whatsapp
        .map(WhatsAppProvider::new)
        .map(Arc::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_credentials_from_env_file() {
        let mut env = EnvFile::default();
        env.set(WHATSAPP_ACCESS_TOKEN_ENV, "tok");
        env.set(WHATSAPP_PHONE_NUMBER_ID_ENV, "pnid");
        let (token, id) = resolve_whatsapp_credentials(&env).unwrap();
        assert_eq!(token, "tok");
        assert_eq!(id, "pnid");
    }

    #[test]
    fn resolve_credentials_errors_when_absent() {
        let env = EnvFile::default();
        let err = resolve_whatsapp_credentials(&env).unwrap_err();
        assert_eq!(err.code, "ConfigurationMissing");
    }

    #[test]
    fn build_whatsapp_config_requires_all_pieces() {
        let mut env = EnvFile::default();
        env.set(WHATSAPP_ACCESS_TOKEN_ENV, "tok");
        let err = build_whatsapp_config(&env).unwrap_err();
        assert_eq!(err.message, "AIOS_WHATSAPP_PHONE_NUMBER_ID not stored");
    }

    #[test]
    fn configured_providers_reflects_env_file() {
        let dir = std::env::temp_dir().join(format!("life-wa-{}", uuid::Uuid::new_v4()));
        let path = dir.join(".env");
        let mut env = EnvFile::default();
        env.set(WHATSAPP_ACCESS_TOKEN_ENV, "tok");
        env.set(WHATSAPP_PHONE_NUMBER_ID_ENV, "pnid");
        env.write(&path).unwrap();
        let providers = configured_providers(&path);
        assert!(providers.whatsapp.is_some());
        assert!(whatsapp_provider(&path).is_some());
        std::fs::remove_dir_all(&dir).ok();
    }
}
