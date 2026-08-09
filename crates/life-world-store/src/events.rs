//! World Store events published on the EventBus (RFC-0008 §Events).
//!
//! Downstream subscribers (Open Space live views, notifications) consume these
//! to react to changes in the canonical world.

use ai_os_core::events::Event;
use memory_core::wm::{EntityId, RelationshipId};
use serde::{Deserialize, Serialize};

/// Emitted after a successful entity create.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCreated {
    /// Created entity id.
    pub id: EntityId,
    /// Normalized entity type.
    pub entity_type: String,
}

impl Event for EntityCreated {
    fn event_type(&self) -> &'static str {
        "worldstore.entity_created"
    }
}

/// Emitted after a successful entity update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityUpdated {
    /// Updated entity id.
    pub id: EntityId,
    /// New version after the update.
    pub version: u64,
}

impl Event for EntityUpdated {
    fn event_type(&self) -> &'static str {
        "worldstore.entity_updated"
    }
}

/// Emitted after an entity is soft deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityArchived {
    /// Archived entity id.
    pub id: EntityId,
    /// Reason supplied by the caller.
    pub reason: String,
}

impl Event for EntityArchived {
    fn event_type(&self) -> &'static str {
        "worldstore.entity_archived"
    }
}

/// Emitted after an entity is restored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRestored {
    /// Restored entity id.
    pub id: EntityId,
}

impl Event for EntityRestored {
    fn event_type(&self) -> &'static str {
        "worldstore.entity_restored"
    }
}

/// Emitted after a relationship is created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipCreated {
    /// Relationship id.
    pub id: RelationshipId,
    /// Source endpoint.
    pub source: EntityId,
    /// Target endpoint.
    pub target: EntityId,
}

impl Event for RelationshipCreated {
    fn event_type(&self) -> &'static str {
        "worldstore.relationship_created"
    }
}

/// Emitted after an observation is ingested.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationRecorded {
    /// Observation id.
    pub id: crate::types::ObservationId,
    /// Source kind.
    pub kind: String,
}

impl Event for ObservationRecorded {
    fn event_type(&self) -> &'static str {
        "worldstore.observation_recorded"
    }
}

/// Emitted after a document is added.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCreated {
    /// Document id.
    pub id: crate::types::DocumentId,
}

impl Event for DocumentCreated {
    fn event_type(&self) -> &'static str {
        "worldstore.document_created"
    }
}
