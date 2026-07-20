use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

use async_trait::async_trait;

use crate::error::{KnowledgeError, KnowledgeResult};
use crate::event::{KnowledgeAdded, KnowledgeRemoved};

/// An entity in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// Entity type (e.g., "person", "organization").
    pub entity_type: String,
    /// Creation timestamp (nanoseconds since epoch).
    pub timestamp: i64,
    /// Source attribution IDs.
    pub sources: Vec<Uuid>,
}

/// A fact associated with an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    /// Unique fact identifier.
    pub id: Uuid,
    /// Owning entity ID.
    pub entity_id: Uuid,
    /// Predicate label.
    pub predicate: String,
    /// Object value (literal or entity ID).
    pub object: String,
    /// Confidence score [0.0, 1.0].
    pub confidence: f32,
    /// Unix timestamp (nanoseconds since epoch).
    pub timestamp: i64,
    /// Provenance tracking IDs.
    pub sources: Vec<Uuid>,
}

/// Unified CRUD interface for the knowledge base.
#[async_trait]
pub trait KnowledgeBase: Send + Sync + std::fmt::Debug {
    /// Insert a new entity.
    async fn insert_entity(&self, entity: Entity) -> KnowledgeResult<()>;
    /// Retrieve an entity by ID.
    async fn get_entity(&self, id: &Uuid) -> KnowledgeResult<Option<Entity>>;
    /// Delete an entity by ID.
    async fn delete_entity(&self, id: &Uuid) -> KnowledgeResult<()>;
    /// List all entities.
    async fn list_entities(&self) -> KnowledgeResult<Vec<Entity>>;
    /// Insert a new fact.
    async fn insert_fact(&self, fact: Fact) -> KnowledgeResult<()>;
    /// Retrieve a fact by ID.
    async fn get_fact(&self, id: &Uuid) -> KnowledgeResult<Option<Fact>>;
    /// Delete a fact by ID.
    async fn delete_fact(&self, id: &Uuid) -> KnowledgeResult<()>;
    /// Facts owned by a given entity.
    async fn facts_for_entity(&self, entity_id: &Uuid) -> KnowledgeResult<Vec<Fact>>;
    /// Aggregate statistics.
    async fn stats(&self) -> KnowledgeResult<KnowledgeStats>;
}

/// Aggregate statistics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KnowledgeStats {
    /// Total entities.
    pub entities: u64,
    /// Total facts.
    pub facts: u64,
}

/// In-memory implementation using `RwLock<HashMap<Uuid, Entity>>` and `RwLock<HashMap<Uuid, Fact>>`.
#[derive(Debug, Default)]
pub struct InMemoryKnowledgeBase {
    entities: RwLock<HashMap<Uuid, Entity>>,
    facts: RwLock<HashMap<Uuid, Fact>>,
    pending_events: RwLock<Vec<String>>,
}

impl InMemoryKnowledgeBase {
    /// Construct an empty knowledge base.
    pub fn new() -> Self {
        Self::default()
    }

    /// Drain pending event type strings (test helper).
    pub fn drain_events(&self) -> Vec<String> {
        std::mem::take(&mut *self.pending_events.write().unwrap())
    }
}

#[async_trait]
impl KnowledgeBase for InMemoryKnowledgeBase {
    async fn insert_entity(&self, entity: Entity) -> KnowledgeResult<()> {
        let mut ents = self
            .entities
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        if ents.contains_key(&entity.id) {
            return Err(KnowledgeError::EntityAlreadyExists(entity.id));
        }
        let id = entity.id;
        let name = entity.name.clone();
        let entity_type = entity.entity_type.clone();
        let ts = entity.timestamp;
        ents.insert(id, entity);
        drop(ents);
        self.pending_events
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        let _evt = KnowledgeAdded {
            id,
            name,
            entity_type,
            timestamp: ts,
        };
        Ok(())
    }
    async fn get_entity(&self, id: &Uuid) -> KnowledgeResult<Option<Entity>> {
        let ents = self
            .entities
            .read()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        Ok(ents.get(id).cloned())
    }
    async fn delete_entity(&self, id: &Uuid) -> KnowledgeResult<()> {
        let mut ents = self
            .entities
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        ents.remove(id);
        drop(ents);
        self.pending_events
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        let _evt = KnowledgeRemoved {
            id: *id,
            reason: None,
            timestamp: memory_core::Timestamp::now().as_nanos(),
        };
        Ok(())
    }
    async fn list_entities(&self) -> KnowledgeResult<Vec<Entity>> {
        let ents = self
            .entities
            .read()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        Ok(ents.values().cloned().collect())
    }
    async fn insert_fact(&self, fact: Fact) -> KnowledgeResult<()> {
        let mut fts = self
            .facts
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        if fts.contains_key(&fact.id) {
            return Err(KnowledgeError::FactNotFound(fact.id));
        }
        fts.insert(fact.id, fact.clone());
        Ok(())
    }
    async fn get_fact(&self, id: &Uuid) -> KnowledgeResult<Option<Fact>> {
        let fts = self
            .facts
            .read()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        Ok(fts.get(id).cloned())
    }
    async fn delete_fact(&self, id: &Uuid) -> KnowledgeResult<()> {
        let mut fts = self
            .facts
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        fts.remove(id);
        Ok(())
    }
    async fn facts_for_entity(&self, entity_id: &Uuid) -> KnowledgeResult<Vec<Fact>> {
        let fts = self
            .facts
            .read()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        Ok(fts
            .values()
            .filter(|f| f.entity_id == *entity_id)
            .cloned()
            .collect())
    }
    async fn stats(&self) -> KnowledgeResult<KnowledgeStats> {
        let ents = self
            .entities
            .read()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        let fts = self
            .facts
            .read()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        Ok(KnowledgeStats {
            entities: ents.len() as u64,
            facts: fts.len() as u64,
        })
    }
}

/// No-op default implementation.
#[derive(Debug, Default)]
pub struct DefaultKnowledgeBase;

#[async_trait]
impl KnowledgeBase for DefaultKnowledgeBase {
    async fn insert_entity(&self, _: Entity) -> KnowledgeResult<()> {
        Ok(())
    }
    async fn get_entity(&self, _: &Uuid) -> KnowledgeResult<Option<Entity>> {
        Ok(None)
    }
    async fn delete_entity(&self, _: &Uuid) -> KnowledgeResult<()> {
        Ok(())
    }
    async fn list_entities(&self) -> KnowledgeResult<Vec<Entity>> {
        Ok(Vec::new())
    }
    async fn insert_fact(&self, _: Fact) -> KnowledgeResult<()> {
        Ok(())
    }
    async fn get_fact(&self, _: &Uuid) -> KnowledgeResult<Option<Fact>> {
        Ok(None)
    }
    async fn delete_fact(&self, _: &Uuid) -> KnowledgeResult<()> {
        Ok(())
    }
    async fn facts_for_entity(&self, _: &Uuid) -> KnowledgeResult<Vec<Fact>> {
        Ok(Vec::new())
    }
    async fn stats(&self) -> KnowledgeResult<KnowledgeStats> {
        Ok(KnowledgeStats::default())
    }
}
