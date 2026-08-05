//! # AI-OS Real MCP Plugins
//!
//! Production tool plugins for the AI-OS MCP server ([`ai-os-mcp-server`]).
//! Each plugin is a named collection of tools implementing a real
//! capability area:
//!
//! | Plugin | Tools | Capabilities |
//! |---|---|---|
//! | [`EmailPlugin`] | `email.send`, `email.inject` | send email; simulate inbound |
//! | [`FileSystemPlugin`] | `filesystem.read`, `filesystem.write`, `filesystem.search` | read/write/search files under a root |
//! | [`GitHubPlugin`] | `github.create_issue`, `github.read_repository` | in-memory repo issue lifecycle |
//! | [`CalendarPlugin`] | `calendar.read`, `calendar.create_event` | in-memory event store |
//! | [`TelegramPlugin`] | `telegram.send`, `telegram.inject_inbound` | post message; simulate inbound |
//!
//! These are **real** plugins, not test mocks: they keep state, validate
//! inputs, enforce containment (filesystem root), and emit observations.
//! They are self-contained (no external SaaS credentials) so the platform
//! can be validated deterministically. The MCP client — and the Cognitive
//! Loop — cannot tell them apart from an external connector.
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
pub mod telegram;
pub mod util;

pub use calendar::CalendarPlugin;
pub use email::EmailPlugin;
pub use filesystem::FileSystemPlugin;
pub use github::GitHubPlugin;
pub use telegram::TelegramPlugin;

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use ai_os_mcp_server::{McpPlugin, McpServerError};

/// The set of plugin names the binary understands.
pub const PLUGIN_NAMES: &[&str] = &["email", "filesystem", "github", "calendar", "telegram"];

/// Build the requested plugins. Unknown names in `enabled` are ignored.
pub fn build_plugins(
    root: &Path,
    enabled: &HashSet<String>,
) -> Result<Vec<Arc<dyn McpPlugin>>, McpServerError> {
    let mut plugins: Vec<Arc<dyn McpPlugin>> = Vec::new();
    if enabled.contains("email") {
        plugins.push(Arc::new(EmailPlugin::new()));
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
    Ok(plugins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_plugins_selects_only_enabled() {
        let root = std::env::temp_dir();
        let enabled = HashSet::from(["email".to_string(), "telegram".to_string()]);
        let plugins = build_plugins(&root, &enabled).unwrap();
        let names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
        assert_eq!(names, vec!["email", "telegram"]);
    }

    #[test]
    fn build_plugins_ignores_unknown_names() {
        let root = std::env::temp_dir();
        let enabled = HashSet::from(["bogus".to_string()]);
        let plugins = build_plugins(&root, &enabled).unwrap();
        assert!(plugins.is_empty());
    }
}
