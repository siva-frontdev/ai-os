use crate::model::ResourceState;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};

use ai_os_core::events::Event;

/// Published when the World Model is updated with new information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldModelUpdated {
    pub update_type: String,
    pub description: String,
    pub updated_at: Timestamp,
}

impl Event for WorldModelUpdated {
    fn event_type(&self) -> &'static str {
        "brain.model.world.updated"
    }
}

/// Published when resource availability changes significantly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceStateChanged {
    pub resource_type: String,
    pub old_state: ResourceState,
    pub new_state: ResourceState,
    pub changed_at: Timestamp,
}

impl Event for ResourceStateChanged {
    fn event_type(&self) -> &'static str {
        "brain.model.resource.changed"
    }
}
