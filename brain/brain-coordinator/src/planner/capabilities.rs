use std::collections::HashMap;

/// A declared capability the system can execute.
#[derive(Debug, Clone)]
pub struct Capability {
    pub name: String,
    pub description: String,
}

/// Registry of all capabilities the system can perform.
/// The Planner reasons over these to decide what actions to take.
#[derive(Debug)]
pub struct CapabilityRegistry {
    capabilities: HashMap<String, Capability>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
        }
    }

    /// Register a new capability. Requires only a name and description.
    /// No parser changes. No keyword rules. No match arms.
    pub fn register(&mut self, capability: Capability) {
        self.capabilities
            .insert(capability.name.clone(), capability);
    }

    /// Get the list of all registered capabilities for the Planner prompt.
    pub fn list(&self) -> Vec<&Capability> {
        let mut caps: Vec<_> = self.capabilities.values().collect();
        caps.sort_by_key(|c| c.name.as_str());
        caps
    }

    /// Get a capability by name.
    pub fn get(&self, name: &str) -> Option<&Capability> {
        self.capabilities.get(name)
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register(Capability {
            name: "search_memory".into(),
            description: "Search stored knowledge about the user — their projects, interests, preferences, and past conversations. Use when the user asks about what you know, remember, or should know about them.".into(),
        });
        reg.register(Capability {
            name: "search_recent_conversation".into(),
            description:
                "Review recent conversation history for context about what was just discussed."
                    .into(),
        });
        reg.register(Capability {
            name: "respond".into(),
            description: "Generate a natural language response to the user's message.".into(),
        });
        reg.register(Capability {
            name: "ask_clarification".into(),
            description: "Ask the user a clarifying question when their intent is ambiguous."
                .into(),
        });
        reg.register(Capability {
            name: "ignore".into(),
            description:
                "Do nothing. Use when the message is empty, trivial, or requires no action.".into(),
        });
        reg.register(Capability {
            name: "observe".into(),
            description: "Silently record the observation without responding. Use for factual updates the user provides.".into(),
        });
        reg.register(Capability {
            name: "schedule".into(),
            description: "Schedule a future action or reminder.".into(),
        });
        reg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_standard_capabilities() {
        let reg = CapabilityRegistry::default();
        assert!(reg.get("search_memory").is_some());
        assert!(reg.get("respond").is_some());
        assert!(reg.get("ignore").is_some());
        assert!(reg.get("observe").is_some());
    }

    #[test]
    fn test_custom_capability_registration() {
        let mut reg = CapabilityRegistry::new();
        reg.register(Capability {
            name: "web_search".into(),
            description: "Search the web for information.".into(),
        });
        assert!(reg.get("web_search").is_some());
    }

    #[test]
    fn test_list_returns_sorted_capabilities() {
        let reg = CapabilityRegistry::default();
        let list = reg.list();
        assert!(!list.is_empty());
        for i in 1..list.len() {
            assert!(list[i - 1].name <= list[i].name);
        }
    }
}
