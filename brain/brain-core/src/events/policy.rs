//! Policy events.
use ai_os_core::events::Event;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyEvaluated {
    pub plan_id: String,       // PlanId serialized
    pub evaluation_id: String, // UUID
    pub passed: bool,
    pub rules_evaluated: u32,
    pub violations: Vec<String>, // rule names that violated
    pub duration_us: u64,
    pub evaluated_at: Timestamp,
}

impl Event for PolicyEvaluated {
    fn event_type(&self) -> &'static str {
        "brain.policy.evaluated"
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyReloaded {
    pub source: String, // "config_file", "hot_reload", "api"
    pub rulesets_loaded: u32,
    pub guard_rails_loaded: u32,
    pub reloaded_at: Timestamp,
}

impl Event for PolicyReloaded {
    fn event_type(&self) -> &'static str {
        "brain.policy.reloaded"
    }
}
