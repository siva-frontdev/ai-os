//! # AI-OS Real MCP Plugins
//!
//! Production tool plugins for the AI-OS MCP server ([`ai-os-mcp-server`]).
//! Each plugin is a named collection of tools implementing a real
//! capability area:
//!
//! | Plugin | Tools | Capabilities |
//! |---|---|---|
//! | [`EmailPlugin`] | `email.send`, `email.inject` | Gmail delivery (real); simulate inbound |
//! | [`WhatsAppPlugin`] | `whatsapp.send`, `whatsapp.receive` | WhatsApp Cloud API send/receive (real) |
//! | [`FileSystemPlugin`] | `filesystem.read`, `filesystem.write`, `filesystem.search` | read/write/search files under a root |
//! | [`GitHubPlugin`] | `github.create_issue`, `github.read_repository` | in-memory repo issue lifecycle |
//! | [`CalendarPlugin`] | `calendar.read`, `calendar.create_event` | in-memory event store |
//! | [`TelegramPlugin`] | `telegram.send`, `telegram.inject_inbound` | post message; simulate inbound |
//!
//! These are **real** plugins, not test mocks: they keep state, validate
//! inputs, enforce containment (filesystem root), and emit observations.
//!
//! Two plugins talk to external SaaS providers and are production-ready:
//!
//! - **Gmail** ([`provider::gmail`]) — `email.send` delivers through the
//!   Gmail API and reports success only after the message is confirmed in
//!   the Sent folder.
//! - **WhatsApp** ([`provider::whatsapp`]) — `whatsapp.send` delivers
//!   through the Meta WhatsApp Cloud API and reports success only when Meta
//!   returns a message id. `whatsapp.receive` drains inbound messages that
//!   arrive on a webhook listener into observations.
//!
//! Providers activate only when credentials are supplied via environment
//! variables (`AIOS_GMAIL_*`, `AIOS_WHATSAPP_*`). Without credentials the
//! tools return a structured `ConfigurationMissing` error — they **never**
//! fabricate success. See `docs/runtime/gmail-provider.md` and
//! `docs/runtime/whatsapp-provider.md` for setup.
//!
//! The other plugins (github, calendar, telegram, filesystem) remain
//! self-contained; their non-production status is tracked in
//! `docs/runtime/runtime-capability-audit.md`.
//!
//! The companion binary `ai-os-mcp-server` (built from this crate) hosts a
//! selected set of plugins over the MCP stdio transport. AI-OS spawns it as
//! the "real MCP server" that Phase 4 of Runtime Validation uses in place of
//! the in-process mock.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod calendar;
pub mod email;
pub mod filesystem;
pub mod github;
pub mod provider;
pub mod telegram;
pub mod util;
pub mod whatsapp;

pub use calendar::CalendarPlugin;
pub use email::EmailPlugin;
pub use filesystem::FileSystemPlugin;
pub use github::GitHubPlugin;
pub use telegram::TelegramPlugin;
pub use whatsapp::WhatsAppPlugin;

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use ai_os_mcp_server::{McpPlugin, McpServerError};

use crate::provider::{GmailProvider, Providers, WebhookConfig, WebhookServer, WhatsAppProvider};

/// The set of plugin names the binary understands.
pub const PLUGIN_NAMES: &[&str] = &[
    "email",
    "filesystem",
    "github",
    "calendar",
    "telegram",
    "whatsapp",
];

/// Build the requested plugins. Unknown names in `enabled` are ignored.
///
/// Provider-backed plugins (email, whatsapp) use `providers` loaded from the
/// environment; absent credentials produce unconfigured plugins that report
/// `ConfigurationMissing` rather than fake success. When WhatsApp is
/// configured with a webhook port, a webhook listener is started.
pub fn build_plugins(
    root: &Path,
    enabled: &HashSet<String>,
    providers: &Providers,
) -> Result<Vec<Arc<dyn McpPlugin>>, McpServerError> {
    let mut plugins: Vec<Arc<dyn McpPlugin>> = Vec::new();
    if enabled.contains("email") {
        let gmail = providers
            .gmail
            .clone()
            .map(GmailProvider::new)
            .map(Arc::new);
        plugins.push(Arc::new(EmailPlugin::new(gmail)));
    }
    if enabled.contains("filesystem") {
        plugins.push(Arc::new(FileSystemPlugin::new(root)?));
    }
    if enabled.contains("github") {
        plugins.push(Arc::new(GitHubPlugin::new("ai-os", "AI-native OS", "main")));
    }
    if enabled.contains("calendar") {
        plugins.push(Arc::new(CalendarPlugin::new()));
    }
    if enabled.contains("telegram") {
        plugins.push(Arc::new(TelegramPlugin::new()));
    }
    if enabled.contains("whatsapp") {
        let mut webhook = None;
        let whatsapp = providers.whatsapp.clone().map(|cfg| {
            let provider = Arc::new(WhatsAppProvider::new(cfg.clone()));
            if cfg.webhook_port > 0 {
                let wcfg = WebhookConfig {
                    host: cfg.webhook_host.clone(),
                    port: cfg.webhook_port,
                    path: cfg.webhook_path.clone(),
                };
                match WebhookServer::start(wcfg, provider.clone()) {
                    Ok(server) => webhook = Some(server),
                    Err(err) => {
                        tracing::error!("failed to start whatsapp webhook: {err}");
                    }
                }
            }
            provider
        });
        plugins.push(Arc::new(
            WhatsAppPlugin::new(whatsapp).with_webhook(webhook),
        ));
    }
    Ok(plugins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_plugins_selects_only_enabled() {
        let root = std::env::temp_dir();
        let enabled = HashSet::from(["email".to_string(), "telegram".to_string()]);
        let plugins = build_plugins(&root, &enabled, &Providers::default()).unwrap();
        let names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
        assert_eq!(names, vec!["email", "telegram"]);
    }

    #[test]
    fn build_plugins_ignores_unknown_names() {
        let root = std::env::temp_dir();
        let enabled = HashSet::from(["bogus".to_string()]);
        let plugins = build_plugins(&root, &enabled, &Providers::default()).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn build_plugins_hosts_whatsapp_when_enabled() {
        let root = std::env::temp_dir();
        let enabled = HashSet::from(["whatsapp".to_string()]);
        let plugins = build_plugins(&root, &enabled, &Providers::default()).unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name(), "whatsapp");
    }
}
