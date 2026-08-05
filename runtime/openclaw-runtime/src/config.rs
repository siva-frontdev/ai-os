//! Configuration for the OpenClaw runtime.

use std::collections::{HashMap, HashSet};

use ai_os_runtime_api::{CapabilityId, SideEffect};

/// How the OpenClaw MCP server is reached.
#[derive(Debug, Clone)]
pub enum TransportConfig {
    /// Spawn a subprocess and speak newline-delimited JSON-RPC over its
    /// stdio (MCP stdio transport).
    Stdio {
        /// Command to execute (e.g. `npx`, `openclaw`).
        command: String,
        /// Arguments (e.g. `["openclaw", "mcp"]`).
        args: Vec<String>,
    },
}

/// Configuration for [`OpenClawRuntime`](crate::OpenClawRuntime).
///
/// `#[serde(skip_serializing)]` fields are never logged or serialized.
#[derive(Debug, Clone)]
pub struct OpenClawRuntimeConfig {
    /// Runtime id, default `openclaw`.
    pub runtime_id: String,
    /// How to reach the MCP server.
    pub transport: TransportConfig,
    /// If present, only these OpenClaw tool names become capabilities
    /// (security allowlist).
    pub tool_allowlist: Option<HashSet<String>>,
    /// OpenClaw tool name → AI-OS capability id overrides. Without an
    /// entry the capability id defaults to the tool name.
    pub capability_overrides: HashMap<String, String>,
    /// AI-OS capability id → declared side effects.
    pub side_effects: HashMap<String, Vec<SideEffect>>,
}

impl Default for OpenClawRuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_id: "openclaw".into(),
            transport: TransportConfig::Stdio {
                command: "openclaw".into(),
                args: vec!["mcp".into()],
            },
            tool_allowlist: None,
            capability_overrides: HashMap::new(),
            side_effects: HashMap::new(),
        }
    }
}

impl OpenClawRuntimeConfig {
    /// Convenience builder for a stdio transport.
    pub fn stdio(mut self, command: impl Into<String>, args: Vec<String>) -> Self {
        self.transport = TransportConfig::Stdio {
            command: command.into(),
            args,
        };
        self
    }

    /// Convenience: allow only the given tool names.
    pub fn allow_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tool_allowlist = Some(tools.into_iter().map(Into::into).collect());
        self
    }

    /// Convenience: map an OpenClaw tool to an AI-OS capability id.
    pub fn override_capability(
        mut self,
        tool: impl Into<String>,
        capability: impl Into<String>,
    ) -> Self {
        self.capability_overrides
            .insert(tool.into(), capability.into());
        self
    }

    /// Convenience: declare side effects for a capability id.
    pub fn with_side_effects(mut self, capability: CapabilityId, effects: Vec<SideEffect>) -> Self {
        self.side_effects.insert(capability.0, effects);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OpenClawRuntimeConfig::default();
        assert_eq!(config.runtime_id, "openclaw");
        assert!(matches!(config.transport, TransportConfig::Stdio { .. }));
    }

    #[test]
    fn test_builders_accumulate() {
        let config = OpenClawRuntimeConfig::default()
            .allow_tools(["email.send"])
            .override_capability("send_email", "email.send");
        assert!(config
            .tool_allowlist
            .as_ref()
            .unwrap()
            .contains("email.send"));
        assert_eq!(
            config.capability_overrides.get("send_email").unwrap(),
            "email.send"
        );
    }
}
