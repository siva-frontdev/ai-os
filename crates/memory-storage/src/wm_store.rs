use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use memory_core::Timestamp;
use memory_core::wm::{Entity, EntityId, Relationship, RelationshipId, Value};

#[derive(Debug, Clone)]
pub struct EntityDiff {
    pub entity_id: EntityId,
    pub entity_type: String,
    pub name: String,
    pub changes: Vec<String>,
    pub previous_importance: f32,
    pub new_importance: f32,
    pub previous_confidence: f32,
    pub new_confidence: f32,
}

#[derive(Debug, Clone)]
pub enum WorldModelChange {
    EntityCreated(Entity),
    EntityUpdated(EntityDiff),
    RelationshipCreated(Relationship),
    EntityArchived(EntityId),
}

#[async_trait]
pub trait WorldModelStore: Send + Sync + std::fmt::Debug {
    async fn insert_entity(&self, entity: Entity) -> EntityId;
    async fn get_entity(&self, id: &EntityId) -> Option<Entity>;
    async fn update_entity(&self, entity: &Entity);
    async fn delete_entity(&self, id: &EntityId);
    async fn search_entities_by_type(&self, entity_type: &str) -> Vec<Entity>;
    async fn search_entities_by_name(&self, name: &str) -> Vec<Entity>;
    async fn all_entities(&self) -> Vec<Entity>;

    async fn all_relationships(&self) -> Vec<Relationship>;

    async fn insert_relationship(&self, rel: &Relationship) -> RelationshipId;
    async fn update_relationship(&self, rel: &Relationship);
    async fn get_relationship(&self, id: &RelationshipId) -> Option<Relationship>;
    async fn get_relationships_for_entity(&self, entity_id: &EntityId) -> Vec<Relationship>;
    async fn get_relationships_between(
        &self,
        source: &EntityId,
        target: &EntityId,
    ) -> Vec<Relationship>;

    async fn get_neighbors(
        &self,
        entity_id: &EntityId,
        rel_types: Option<&[&str]>,
    ) -> Vec<(Relationship, Entity)>;

    async fn get_incoming_neighbors(
        &self,
        entity_id: &EntityId,
        rel_types: Option<&[&str]>,
    ) -> Vec<(Relationship, Entity)>;

    async fn entity_count(&self) -> usize;
    async fn relationship_count(&self) -> usize;

    async fn clear(&self);
}

#[derive(Debug)]
pub struct InMemoryWorldModelStore {
    entities: RwLock<HashMap<EntityId, Entity>>,
    relationships: RwLock<HashMap<RelationshipId, Relationship>>,
    outgoing_edges: RwLock<HashMap<EntityId, Vec<RelationshipId>>>,
    incoming_edges: RwLock<HashMap<EntityId, Vec<RelationshipId>>>,
}

impl InMemoryWorldModelStore {
    pub fn new() -> Self {
        Self {
            entities: RwLock::new(HashMap::new()),
            relationships: RwLock::new(HashMap::new()),
            outgoing_edges: RwLock::new(HashMap::new()),
            incoming_edges: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryWorldModelStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorldModelStore for InMemoryWorldModelStore {
    async fn insert_entity(&self, entity: Entity) -> EntityId {
        let id = entity.id;
        if let Ok(mut entities) = self.entities.write() {
            entities.insert(id, entity);
        }
        id
    }

    async fn get_entity(&self, id: &EntityId) -> Option<Entity> {
        self.entities.read().ok()?.get(id).cloned()
    }

    async fn update_entity(&self, entity: &Entity) {
        if let Ok(mut entities) = self.entities.write() {
            if let Some(existing) = entities.get_mut(&entity.id) {
                *existing = entity.clone();
            }
        }
    }

    async fn delete_entity(&self, id: &EntityId) {
        if let Ok(mut entities) = self.entities.write() {
            entities.remove(id);
        }
        if let Ok(mut outgoing) = self.outgoing_edges.write() {
            if let Some(rel_ids) = outgoing.remove(id) {
                if let Ok(mut relationships) = self.relationships.write() {
                    for rid in &rel_ids {
                        relationships.remove(rid);
                    }
                }
            }
        }
    }

    async fn search_entities_by_type(&self, entity_type: &str) -> Vec<Entity> {
        self.entities
            .read()
            .map(|e| {
                e.values()
                    .filter(|ent| ent.entity_type == entity_type)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    async fn search_entities_by_name(&self, name: &str) -> Vec<Entity> {
        let lower = name.to_lowercase();
        self.entities
            .read()
            .map(|e| {
                e.values()
                    .filter(|ent| ent.name.to_lowercase().contains(&lower))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    async fn all_entities(&self) -> Vec<Entity> {
        self.entities
            .read()
            .map(|e| e.values().cloned().collect())
            .unwrap_or_default()
    }

    async fn all_relationships(&self) -> Vec<Relationship> {
        self.relationships
            .read()
            .map(|r| r.values().cloned().collect())
            .unwrap_or_default()
    }

    async fn insert_relationship(&self, rel: &Relationship) -> RelationshipId {
        let id = rel.id;
        if let Ok(mut relationships) = self.relationships.write() {
            relationships.insert(id, rel.clone());
        }
        if let Ok(mut outgoing) = self.outgoing_edges.write() {
            outgoing.entry(rel.source_id).or_default().push(id);
        }
        if let Ok(mut incoming) = self.incoming_edges.write() {
            incoming.entry(rel.target_id).or_default().push(id);
        }
        id
    }

    async fn update_relationship(&self, rel: &Relationship) {
        if let Ok(mut relationships) = self.relationships.write() {
            if let Some(existing) = relationships.get_mut(&rel.id) {
                *existing = rel.clone();
            }
        }
    }

    async fn get_relationship(&self, id: &RelationshipId) -> Option<Relationship> {
        self.relationships.read().ok()?.get(id).cloned()
    }

    async fn get_relationships_for_entity(&self, entity_id: &EntityId) -> Vec<Relationship> {
        let rel_ids = {
            let outgoing = self.outgoing_edges.read().ok();
            let incoming = self.incoming_edges.read().ok();
            let mut ids = Vec::new();
            if let Some(out) = outgoing {
                if let Some(out_ids) = out.get(entity_id) {
                    ids.extend(out_ids.iter());
                }
            }
            if let Some(inc) = incoming {
                if let Some(inc_ids) = inc.get(entity_id) {
                    ids.extend(inc_ids.iter());
                }
            }
            ids
        };
        self.relationships
            .read()
            .map(|r| {
                rel_ids
                    .iter()
                    .filter_map(|rid| r.get(rid).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    async fn get_relationships_between(
        &self,
        source: &EntityId,
        target: &EntityId,
    ) -> Vec<Relationship> {
        self.relationships
            .read()
            .map(|r| {
                r.values()
                    .filter(|rel| rel.source_id == *source && rel.target_id == *target)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    async fn get_neighbors(
        &self,
        entity_id: &EntityId,
        rel_types: Option<&[&str]>,
    ) -> Vec<(Relationship, Entity)> {
        let rels = {
            let outgoing = self.outgoing_edges.read().ok();
            let relationships = self.relationships.read().ok();
            let mut result = Vec::new();
            if let (Some(out), Some(rels)) = (&outgoing, &relationships) {
                if let Some(rel_ids) = out.get(entity_id) {
                    for rid in rel_ids {
                        if let Some(rel) = rels.get(rid) {
                            if let Some(types) = rel_types {
                                if !types.contains(&rel.relationship_type.as_str()) {
                                    continue;
                                }
                            }
                            result.push(rel.clone());
                        }
                    }
                }
            }
            result
        };
        let entities = self.entities.read().ok();
        rels.into_iter()
            .filter_map(|rel| {
                entities
                    .as_ref()
                    .and_then(|e| e.get(&rel.target_id))
                    .map(|entity| (rel, entity.clone()))
            })
            .collect()
    }

    async fn get_incoming_neighbors(
        &self,
        entity_id: &EntityId,
        rel_types: Option<&[&str]>,
    ) -> Vec<(Relationship, Entity)> {
        let rels = {
            let incoming = self.incoming_edges.read().ok();
            let relationships = self.relationships.read().ok();
            let mut result = Vec::new();
            if let (Some(inc), Some(rels)) = (&incoming, &relationships) {
                if let Some(rel_ids) = inc.get(entity_id) {
                    for rid in rel_ids {
                        if let Some(rel) = rels.get(rid) {
                            if let Some(types) = rel_types {
                                if !types.contains(&rel.relationship_type.as_str()) {
                                    continue;
                                }
                            }
                            result.push(rel.clone());
                        }
                    }
                }
            }
            result
        };
        let entities = self.entities.read().ok();
        rels.into_iter()
            .filter_map(|rel| {
                entities
                    .as_ref()
                    .and_then(|e| e.get(&rel.source_id))
                    .map(|entity| (rel, entity.clone()))
            })
            .collect()
    }

    async fn entity_count(&self) -> usize {
        self.entities.read().map(|e| e.len()).unwrap_or(0)
    }

    async fn relationship_count(&self) -> usize {
        self.relationships.read().map(|r| r.len()).unwrap_or(0)
    }

    async fn clear(&self) {
        if let Ok(mut e) = self.entities.write() {
            e.clear();
        }
        if let Ok(mut r) = self.relationships.write() {
            r.clear();
        }
        if let Ok(mut o) = self.outgoing_edges.write() {
            o.clear();
        }
        if let Ok(mut i) = self.incoming_edges.write() {
            i.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_core::wm::Entity;

    #[tokio::test]
    async fn test_insert_and_get_entity() {
        let store = InMemoryWorldModelStore::new();
        let entity = Entity::new("person", "Alice");
        let id = store.insert_entity(entity.clone()).await;
        let retrieved = store.get_entity(&id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Alice");
    }

    #[tokio::test]
    async fn test_relationships() {
        let store = InMemoryWorldModelStore::new();
        let alice = store.insert_entity(Entity::new("person", "Alice")).await;
        let macbook = store.insert_entity(Entity::new("device", "MacBook")).await;

        let rel = Relationship::new("owns", alice, macbook);
        store.insert_relationship(&rel).await;

        let rels = store.get_relationships_for_entity(&alice).await;
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].relationship_type, "owns");

        let neighbors = store.get_neighbors(&alice, None).await;
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].1.name, "MacBook");
    }

    #[tokio::test]
    async fn test_search() {
        let store = InMemoryWorldModelStore::new();
        store.insert_entity(Entity::new("person", "Alice")).await;
        store.insert_entity(Entity::new("project", "AI-OS")).await;

        let persons = store.search_entities_by_type("person").await;
        assert_eq!(persons.len(), 1);
        assert_eq!(persons[0].name, "Alice");

        let found = store.search_entities_by_name("AI-OS").await;
        assert_eq!(found.len(), 1);
    }

    #[tokio::test]
    async fn test_entity_count() {
        let store = InMemoryWorldModelStore::new();
        assert_eq!(store.entity_count().await, 0);
        store.insert_entity(Entity::new("person", "Alice")).await;
        assert_eq!(store.entity_count().await, 1);
    }
}
