//! Capability translation: MCP tools → AI-OS [`Capability`]s.

use std::collections::HashMap;

use ai_os_runtime_api::{Capability, CapabilityId, RuntimeId};

use crate::config::OpenClawRuntimeConfig;
use crate::mcp::protocol::McpTool;

/// Build an AI-OS [`Capability`] from an MCP tool declaration.
///
/// The capability id defaults to the tool name, unless the config maps the
/// tool to a different id (`capability_overrides`). This is the only place
/// an OpenClaw tool name enters the capability space — cognition only ever
/// sees the AI-OS id.
pub fn capability_from_tool(
    runtime_id: &RuntimeId,
    tool: &McpTool,
    config: &OpenClawRuntimeConfig,
) -> Option<Capability> {
    if let Some(allowlist) = &config.tool_allowlist {
        if !allowlist.contains(&tool.name) {
            return None;
        }
    }

    let capability_id = config
        .capability_overrides
        .get(&tool.name)
        .cloned()
        .unwrap_or_else(|| tool.name.clone());

    let side_effects = config
        .side_effects
        .get(&capability_id)
        .cloned()
        .unwrap_or_default();

    let mut metadata = HashMap::new();
    metadata.insert("runtime".into(), runtime_id.0.clone());
    metadata.insert("tool".into(), tool.name.clone());

    Some(Capability {
        id: CapabilityId::new(capability_id),
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.input_schema.clone(),
        output_schema: None,
        side_effects,
        metadata,
    })
}

/// Resolve the backing MCP tool name for a capability.
///
/// The tool name is stored in the capability's `metadata["tool"]` at
/// discovery time.
pub fn tool_name_for(capability: &Capability) -> String {
    capability
        .metadata
        .get("tool")
        .cloned()
        .unwrap_or_else(|| capability.id.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::McpTool;
    use ai_os_runtime_api::SideEffect;
    use serde_json::json;

    fn config() -> OpenClawRuntimeConfig {
        OpenClawRuntimeConfig::default()
    }

    fn tool(name: &str) -> McpTool {
        McpTool {
            name: name.into(),
            description: format!("{name} description"),
            input_schema: json!({"type": "object"}),
        }
    }

    #[test]
    fn default_capability_id_is_tool_name() {
        let config = config();
        let runtime_id = RuntimeId("openclaw".into());
        let cap = capability_from_tool(&runtime_id, &tool("email.send"), &config).unwrap();
        assert_eq!(cap.id, CapabilityId::new("email.send"));
        assert_eq!(cap.metadata.get("tool").unwrap(), "email.send");
        assert_eq!(cap.metadata.get("runtime").unwrap(), "openclaw");
    }

    #[test]
    fn override_maps_tool_to_capability_id() {
        let config = config().override_capability("send_email", "email.send");
        let runtime_id = RuntimeId("openclaw".into());
        let cap = capability_from_tool(&runtime_id, &tool("send_email"), &config).unwrap();
        assert_eq!(cap.id, CapabilityId::new("email.send"));
        assert_eq!(tool_name_for(&cap), "send_email");
    }

    #[test]
    fn allowlist_filters_tools() {
        let config = config().allow_tools(["email.send"]);
        let runtime_id = RuntimeId("openclaw".into());
        assert!(capability_from_tool(&runtime_id, &tool("email.send"), &config).is_some());
        assert!(capability_from_tool(&runtime_id, &tool("web.search"), &config).is_none());
    }

    #[test]
    fn side_effects_are_configurable() {
        let config = config().with_side_effects(
            CapabilityId::new("email.send"),
            vec![SideEffect::SendsMessage],
        );
        let runtime_id = RuntimeId("openclaw".into());
        let cap = capability_from_tool(&runtime_id, &tool("email.send"), &config).unwrap();
        assert_eq!(cap.side_effects, vec![SideEffect::SendsMessage]);
    }
}
