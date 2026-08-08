//! Provider configuration, loaded from the environment.
//!
//! Credentials never live in source or in LIFE — they are supplied to the
//! plugin process via environment variables (or a `.env` file the plugin
//! binary loads at startup, as written by `life setup`). When a provider's
//! required configuration is absent the plugin reports
//! [`ProviderError::ConfigurationMissing`] rather than fabricating success.

use std::path::Path;

use crate::provider::error::ProviderError;

/// Gmail OAuth2 configuration (Google API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailConfig {
    /// OAuth2 client id.
    pub client_id: String,
    /// OAuth2 client secret.
    pub client_secret: String,
    /// OAuth2 refresh token (long-lived).
    pub refresh_token: String,
    /// Account user, defaults to `me`.
    pub user: String,
    /// Token endpoint. Overridable for testing.
    pub token_uri: String,
    /// Gmail API base. Overridable for testing.
    pub api_base: String,
}

/// WhatsApp Cloud API configuration (Meta).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatsAppConfig {
    /// Long-lived system-user access token.
    pub access_token: String,
    /// Phone number id that sends and receives.
    pub phone_number_id: String,
    /// Graph API version prefix, e.g. `v21.0`.
    pub api_version: String,
    /// Graph base URL. Overridable for testing.
    pub graph_base: String,
    /// Webhook verify token shared with Meta.
    pub webhook_verify_token: String,
    /// Webhook signature secret for `X-Hub-Signature-256`.
    pub app_secret: String,
    /// Host to bind the webhook listener (empty = do not serve webhooks).
    pub webhook_host: String,
    /// Port to bind the webhook listener.
    pub webhook_port: u16,
    /// URL path served by the webhook listener.
    pub webhook_path: String,
}

/// All enabled provider configurations.
#[derive(Debug, Clone, Default)]
pub struct Providers {
    /// Gmail provider config, if the GMAIL_* env vars are set.
    pub gmail: Option<GmailConfig>,
    /// WhatsApp provider config, if the WHATSAPP_* env vars are set.
    pub whatsapp: Option<WhatsAppConfig>,
}

const fn env_prefix() -> &'static str {
    "AIOS_"
}

fn get_env(name: &str) -> Option<String> {
    let v = std::env::var(name).ok().filter(|s| !s.is_empty());
    if v.is_some() {
        tracing::debug!(name, "provider env var present");
    }
    v
}

/// Parse a `.env` file (`KEY=VALUE` lines, `#` comments) and apply its values
/// to the process environment, leaving variables that are already set
/// untouched.
///
/// This is how the `ai-os-mcp-server` binary picks up credentials written by
/// `life setup` (default `runtime/config/.env`) without extra tooling.
/// A missing file loads as empty and is not an error.
pub fn load_env_file(path: &Path) -> Result<(), ProviderError> {
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(path).map_err(|e| {
        ProviderError::unexpected_response(format!("read env file {}: {e}", path.display()))
    })?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            if !key.is_empty() && std::env::var_os(key).is_none() {
                std::env::set_var(key, value);
            }
        }
    }
    Ok(())
}

impl Providers {
    /// Load provider configuration from the environment.
    ///
    /// Providers whose required variables are missing are simply absent; the
    /// plugin then reports `ConfigurationMissing` when invoked.
    pub fn from_env() -> Self {
        Self {
            gmail: GmailConfig::from_env(),
            whatsapp: WhatsAppConfig::from_env(),
        }
    }

    /// Load and validate, returning a missing-configuration error naming the
    /// provider when a provider is requested but not configured.
    pub fn require_gmail(&self) -> Result<GmailConfig, ProviderError> {
        self.gmail.clone().ok_or_else(|| {
            ProviderError::configuration_missing(
                "Gmail provider is not configured: set AIOS_GMAIL_CLIENT_ID, \
                 AIOS_GMAIL_CLIENT_SECRET and AIOS_GMAIL_REFRESH_TOKEN",
            )
        })
    }

    /// Load and validate the WhatsApp provider.
    pub fn require_whatsapp(&self) -> Result<WhatsAppConfig, ProviderError> {
        self.whatsapp.clone().ok_or_else(|| {
            ProviderError::configuration_missing(
                "WhatsApp provider is not configured: set AIOS_WHATSAPP_ACCESS_TOKEN \
                 and AIOS_WHATSAPP_PHONE_NUMBER_ID",
            )
        })
    }
}

impl GmailConfig {
    /// Build a Gmail config from `AIOS_GMAIL_*` environment variables.
    fn from_env() -> Option<Self> {
        let client_id = get_env(&format!("{}GMAIL_CLIENT_ID", env_prefix()))?;
        let client_secret = get_env(&format!("{}GMAIL_CLIENT_SECRET", env_prefix()))?;
        let refresh_token = get_env(&format!("{}GMAIL_REFRESH_TOKEN", env_prefix()))?;
        let user = get_env(&format!("{}GMAIL_USER", env_prefix())).unwrap_or_else(|| "me".into());
        let token_uri = get_env(&format!("{}GMAIL_TOKEN_URI", env_prefix()))
            .unwrap_or_else(|| "https://oauth2.googleapis.com/token".into());
        let api_base = get_env(&format!("{}GMAIL_API_BASE", env_prefix()))
            .unwrap_or_else(|| "https://gmail.googleapis.com".into());
        Some(Self {
            client_id,
            client_secret,
            refresh_token,
            user,
            token_uri,
            api_base,
        })
    }
}

impl WhatsAppConfig {
    /// Build a WhatsApp config from `AIOS_WHATSAPP_*` environment variables.
    fn from_env() -> Option<Self> {
        let access_token = get_env(&format!("{}WHATSAPP_ACCESS_TOKEN", env_prefix()))?;
        let phone_number_id = get_env(&format!("{}WHATSAPP_PHONE_NUMBER_ID", env_prefix()))?;
        let api_version = get_env(&format!("{}WHATSAPP_API_VERSION", env_prefix()))
            .unwrap_or_else(|| "v21.0".into());
        let graph_base = get_env(&format!("{}WHATSAPP_GRAPH_BASE", env_prefix()))
            .unwrap_or_else(|| "https://graph.facebook.com".into());
        let webhook_verify_token =
            get_env(&format!("{}WHATSAPP_WEBHOOK_VERIFY_TOKEN", env_prefix())).unwrap_or_default();
        let app_secret =
            get_env(&format!("{}WHATSAPP_APP_SECRET", env_prefix())).unwrap_or_default();
        let webhook_host = get_env(&format!("{}WHATSAPP_WEBHOOK_HOST", env_prefix()))
            .unwrap_or_else(|| "127.0.0.1".into());
        let webhook_port = get_env(&format!("{}WHATSAPP_WEBHOOK_PORT", env_prefix()))
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
        let webhook_path = get_env(&format!("{}WHATSAPP_WEBHOOK_PATH", env_prefix()))
            .unwrap_or_else(|| "/webhook/whatsapp".into());
        Some(Self {
            access_token,
            phone_number_id,
            api_version,
            graph_base,
            webhook_verify_token,
            app_secret,
            webhook_host,
            webhook_port,
            webhook_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env(vars: &[(&str, &str)], f: impl FnOnce()) {
        // Tests mutate the process environment; serialize them so parallel
        // execution cannot observe another test's variables.
        let _guard: MutexGuard<()> = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        struct Guard(Vec<String>);
        impl Drop for Guard {
            fn drop(&mut self) {
                for key in &self.0 {
                    std::env::remove_var(key);
                }
            }
        }
        let mut guard = Guard(Vec::new());
        for (k, v) in vars {
            std::env::set_var(k, v);
            guard.0.push(k.to_string());
        }
        f();
    }

    #[test]
    fn gmail_config_absent_without_env() {
        with_env(&[], || assert!(GmailConfig::from_env().is_none()));
    }

    #[test]
    fn gmail_config_loads_required_vars() {
        with_env(
            &[
                ("AIOS_GMAIL_CLIENT_ID", "cid"),
                ("AIOS_GMAIL_CLIENT_SECRET", "csecret"),
                ("AIOS_GMAIL_REFRESH_TOKEN", "rtok"),
            ],
            || {
                let cfg = GmailConfig::from_env().expect("config present");
                assert_eq!(cfg.client_id, "cid");
                assert_eq!(cfg.user, "me");
                assert_eq!(cfg.api_base, "https://gmail.googleapis.com");
            },
        );
    }

    #[test]
    fn whatsapp_config_loads_required_vars() {
        with_env(
            &[
                ("AIOS_WHATSAPP_ACCESS_TOKEN", "at"),
                ("AIOS_WHATSAPP_PHONE_NUMBER_ID", "pnid"),
            ],
            || {
                let cfg = WhatsAppConfig::from_env().expect("config present");
                assert_eq!(cfg.access_token, "at");
                assert_eq!(cfg.phone_number_id, "pnid");
                assert_eq!(cfg.api_version, "v21.0");
            },
        );
    }

    #[test]
    fn whatsapp_config_absent_without_env() {
        with_env(&[], || assert!(WhatsAppConfig::from_env().is_none()));
    }

    #[test]
    fn providers_require_methods_error_clearly() {
        with_env(&[], || {
            let providers = Providers::from_env();
            let err = providers.require_gmail().unwrap_err();
            assert_eq!(err.code, "ConfigurationMissing");
            let err = providers.require_whatsapp().unwrap_err();
            assert_eq!(err.code, "ConfigurationMissing");
        });
    }

    #[test]
    fn load_env_file_populates_missing_vars() {
        with_env(&[], || {
            let dir = std::env::temp_dir().join(format!("envload-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join(".env");
            std::fs::write(
                &path,
                "# comment\nAIOS_GMAIL_CLIENT_ID=cid\nAIOS_GMAIL_CLIENT_SECRET=csecret\n\n",
            )
            .unwrap();
            load_env_file(&path).unwrap();
            assert_eq!(std::env::var("AIOS_GMAIL_CLIENT_ID").unwrap(), "cid");
            assert_eq!(
                std::env::var("AIOS_GMAIL_CLIENT_SECRET").unwrap(),
                "csecret"
            );
            std::fs::remove_dir_all(&dir).ok();
        });
    }

    #[test]
    fn load_env_file_does_not_override_set_vars() {
        with_env(&[("AIOS_GMAIL_CLIENT_ID", "already")], || {
            let dir = std::env::temp_dir().join(format!("envload-keep-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join(".env");
            std::fs::write(&path, "AIOS_GMAIL_CLIENT_ID=from-file\n").unwrap();
            load_env_file(&path).unwrap();
            assert_eq!(std::env::var("AIOS_GMAIL_CLIENT_ID").unwrap(), "already");
            std::fs::remove_dir_all(&dir).ok();
        });
    }

    #[test]
    fn load_env_file_missing_is_not_an_error() {
        with_env(&[], || {
            let dir = std::env::temp_dir().join(format!("envload-none-{}", uuid::Uuid::new_v4()));
            let path = dir.join(".env");
            assert!(load_env_file(&path).is_ok());
        });
    }
}
