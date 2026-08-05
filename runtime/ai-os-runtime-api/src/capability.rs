//! Capabilities: what a runtime can execute.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A namespaced capability identifier, e.g. `"email.send"`,
/// `"telegram.post_message"`, `"browser.navigate"`.
///
/// Naming convention: `<domain>.<verb>`, lowercase ASCII, dots as
/// separators. The id is the only handle cognition uses to address a
/// runtime action — vendors never appear here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityId(pub String);

impl CapabilityId {
    /// Create a capability id from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for CapabilityId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for CapabilityId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// A capability the runtime can execute right now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Canonical, namespaced identifier.
    pub id: CapabilityId,
    /// Short human label, e.g. "Send email".
    pub name: String,
    /// What the capability does and when to use it (feeds the Planner).
    pub description: String,
    /// JSON Schema for `Action.input`.
    pub input_schema: serde_json::Value,
    /// Optional JSON Schema for `ActionResult.output`.
    pub output_schema: Option<serde_json::Value>,
    /// Declared side effects, for planner transparency and policy.
    pub side_effects: Vec<SideEffect>,
    /// Free-form metadata (e.g. `runtime`, backing tool name).
    pub metadata: HashMap<String, String>,
}

/// Declared side effect of a capability.
///
/// The Planner may use these to weigh risk before dispatching. A
/// capability that mutates external state should declare it here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideEffect {
    /// Sends a message to a person or channel.
    SendsMessage,
    /// Mutates external (non-AI-OS) state.
    MutatesExternalState,
    /// Reads external state without modifying it.
    ReadsExternalState,
    /// Spawns or controls a process.
    SpawnsProcess,
    /// Accesses the network.
    AccessesNetwork,
    /// Accesses the filesystem.
    AccessesFilesystem,
    /// No side effects.
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_id_round_trips_through_serde() {
        let id = CapabilityId::new("email.send");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"email.send\"");
        let back: CapabilityId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn test_capability_id_is_hashable_and_equatable() {
        let a = CapabilityId::new("email.send");
        let b = CapabilityId::new("email.send");
        let c = CapabilityId::new("telegram.post_message");
        let mut map = HashMap::new();
        map.insert(a.clone(), 1);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(map.get(&b), Some(&1));
    }

    #[test]
    fn test_capability_serializes_with_all_fields() {
        let cap = Capability {
            id: CapabilityId::new("email.send"),
            name: "Send email".into(),
            description: "Send an email message".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: None,
            side_effects: vec![SideEffect::SendsMessage],
            metadata: HashMap::from([("runtime".into(), "openclaw".into())]),
        };
        let json = serde_json::to_value(&cap).unwrap();
        assert_eq!(json["id"], "email.send");
        assert_eq!(json["side_effects"][0], "SendsMessage");
    }
}
