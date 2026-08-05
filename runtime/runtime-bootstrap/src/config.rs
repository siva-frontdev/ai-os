//! Runtime bootstrap configuration: which runtimes to start and how.

use std::collections::HashMap;

/// Configuration for one MCP-based runtime instance.
///
/// An MCP runtime is a subprocess speaking the MCP stdio protocol — in this
/// phase the `ai-os-mcp-server` binary hosting the real plugins.
#[derive(Debug, Clone)]
pub struct McpRuntimeConfig {
    /// Unique runtime id (default `openclaw`).
    pub id: String,
    /// Command to spawn (e.g. the `ai-os-mcp-server` binary).
    pub command: String,
    /// Arguments passed to the command (e.g. `--plugins`, `--root`).
    pub args: Vec<String>,
    /// If present, only these tool names become capabilities (allowlist).
    pub allow_tools: Option<Vec<String>>,
    /// Tool name → capability id overrides.
    pub capability_overrides: HashMap<String, String>,
}

/// Configuration for one runtime instance.
#[derive(Debug, Clone)]
pub enum RuntimeInstanceConfig {
    /// An MCP subprocess runtime.
    Mcp(McpRuntimeConfig),
}

/// The full runtime set configuration.
#[derive(Debug, Clone, Default)]
pub struct RuntimeBootstrapConfig {
    /// Runtime instances to register.
    pub instances: Vec<RuntimeInstanceConfig>,
}

impl RuntimeBootstrapConfig {
    /// Build the configuration from environment variables.
    ///
    /// Returns an empty config unless `AI_OS_RUNTIME_ENABLED` is truthy
    /// (`1`, `true`, `yes`, `on`).
    pub fn from_env() -> Self {
        let enabled = std::env::var("AI_OS_RUNTIME_ENABLED")
            .map(|v| {
                let v = v.to_ascii_lowercase();
                matches!(v.as_str(), "1" | "true" | "yes" | "on")
            })
            .unwrap_or(false);
        if !enabled {
            return Self::default();
        }

        let command =
            std::env::var("AI_OS_MCP_SERVER_BIN").unwrap_or_else(|_| "ai-os-mcp-server".into());
        let plugins = std::env::var("AI_OS_MCP_PLUGINS")
            .unwrap_or_else(|_| "email,filesystem,github,calendar,telegram".into());
        let root = std::env::var("AI_OS_FILESYSTEM_ROOT").unwrap_or_else(|_| ".".into());

        Self {
            instances: vec![RuntimeInstanceConfig::Mcp(McpRuntimeConfig {
                id: "openclaw".into(),
                command,
                args: vec!["--plugins".into(), plugins, "--root".into(), root],
                allow_tools: None,
                capability_overrides: HashMap::new(),
            })],
        }
    }

    /// True when no runtime instances are configured.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// The runtime ids implied by this configuration.
    pub fn runtime_ids(&self) -> Vec<String> {
        self.instances
            .iter()
            .map(|instance| match instance {
                RuntimeInstanceConfig::Mcp(config) => config.id.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_when_env_disabled() {
        std::env::remove_var("AI_OS_RUNTIME_ENABLED");
        assert!(RuntimeBootstrapConfig::from_env().is_empty());
    }

    #[test]
    fn builds_one_mcp_runtime_when_enabled() {
        std::env::set_var("AI_OS_RUNTIME_ENABLED", "1");
        std::env::set_var("AI_OS_MCP_SERVER_BIN", "/tmp/server");
        std::env::set_var("AI_OS_MCP_PLUGINS", "email,filesystem");
        std::env::set_var("AI_OS_FILESYSTEM_ROOT", "/tmp/root");

        let config = RuntimeBootstrapConfig::from_env();
        assert_eq!(config.instances.len(), 1);
        let RuntimeInstanceConfig::Mcp(mcp) = &config.instances[0];
        assert_eq!(mcp.id, "openclaw");
        assert_eq!(mcp.command, "/tmp/server");
        assert_eq!(
            mcp.args,
            vec!["--plugins", "email,filesystem", "--root", "/tmp/root"]
        );
        assert_eq!(config.runtime_ids(), vec!["openclaw"]);

        std::env::remove_var("AI_OS_RUNTIME_ENABLED");
        std::env::remove_var("AI_OS_MCP_SERVER_BIN");
        std::env::remove_var("AI_OS_MCP_PLUGINS");
        std::env::remove_var("AI_OS_FILESYSTEM_ROOT");
    }
}
