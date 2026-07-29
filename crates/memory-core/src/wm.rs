use crate::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(uuid::Uuid);

impl EntityId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &uuid::Uuid {
        &self.0
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationshipId(uuid::Uuid);

impl RelationshipId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for RelationshipId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RelationshipId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityLifecycle {
    Active,
    Archived,
    Forgotten,
}

impl Default for EntityLifecycle {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Timestamp(Timestamp),
    EntityId(EntityId),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    Null,
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Boolean(b)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub entity_type: String,
    pub name: String,
    pub properties: HashMap<String, Value>,
    pub importance: f32,
    pub confidence: f32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub version: u64,
    pub lifecycle: EntityLifecycle,
    pub metadata: HashMap<String, String>,
}

impl Entity {
    pub fn new(entity_type: &str, name: &str) -> Self {
        let now = Timestamp::now();
        Self {
            id: EntityId::new(),
            entity_type: entity_type.to_string(),
            name: name.to_string(),
            properties: HashMap::new(),
            importance: 0.5,
            confidence: 0.5,
            created_at: now,
            updated_at: now,
            version: 1,
            lifecycle: EntityLifecycle::Active,
            metadata: HashMap::new(),
        }
    }

    pub fn with_property(mut self, key: &str, value: Value) -> Self {
        self.properties.insert(key.to_string(), value);
        self
    }

    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn touch(&mut self) {
        self.updated_at = Timestamp::now();
        self.version += 1;
    }

    pub fn property(&self, key: &str) -> Option<&Value> {
        self.properties.get(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: RelationshipId,
    pub relationship_type: String,
    pub source_id: EntityId,
    pub target_id: EntityId,
    pub properties: HashMap<String, Value>,
    pub confidence: f32,
    pub weight: f32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub version: u64,
}

impl Relationship {
    pub fn new(relationship_type: &str, source_id: EntityId, target_id: EntityId) -> Self {
        let now = Timestamp::now();
        Self {
            id: RelationshipId::new(),
            relationship_type: relationship_type.to_string(),
            source_id,
            target_id,
            properties: HashMap::new(),
            confidence: 0.5,
            weight: 0.5,
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight.clamp(0.0, 1.0);
        self
    }

    pub fn touch(&mut self) {
        self.updated_at = Timestamp::now();
        self.version += 1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityObservation {
    pub entity_type: String,
    pub name: String,
    pub properties: HashMap<String, Value>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipObservation {
    pub relationship_type: String,
    pub source_name: String,
    pub target_name: String,
    pub confidence: f32,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub entities: Vec<EntityObservation>,
    pub relationships: Vec<RelationshipObservation>,
}

impl From<EntityObservation> for Entity {
    fn from(obs: EntityObservation) -> Self {
        let now = Timestamp::now();
        Self {
            id: EntityId::new(),
            entity_type: obs.entity_type,
            name: obs.name,
            properties: obs.properties,
            importance: obs.confidence,
            confidence: obs.confidence,
            created_at: now,
            updated_at: now,
            version: 1,
            lifecycle: EntityLifecycle::Active,
            metadata: HashMap::new(),
        }
    }
}

impl From<(RelationshipObservation, EntityId, EntityId)> for Relationship {
    fn from((obs, source_id, target_id): (RelationshipObservation, EntityId, EntityId)) -> Self {
        let now = Timestamp::now();
        Self {
            id: RelationshipId::new(),
            relationship_type: obs.relationship_type,
            source_id,
            target_id,
            properties: HashMap::new(),
            confidence: obs.confidence,
            weight: obs.weight,
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_creation() {
        let e = Entity::new("person", "Alice")
            .with_property("email", Value::String("alice@example.com".into()))
            .with_importance(0.8)
            .with_confidence(0.9);
        assert_eq!(e.entity_type, "person");
        assert_eq!(e.name, "Alice");
        assert!(e.importance - 0.8 < 0.01);
        assert!(e.confidence - 0.9 < 0.01);
    }

    #[test]
    fn test_entity_touch() {
        let mut e = Entity::new("project", "AI-OS");
        let v1 = e.version;
        e.touch();
        assert_eq!(e.version, v1 + 1);
    }

    #[test]
    fn test_relationship_creation() {
        let alice = Entity::new("person", "Alice");
        let macbook = Entity::new("device", "MacBook Pro");
        let r = Relationship::new("owns", alice.id, macbook.id)
            .with_confidence(0.9)
            .with_weight(1.0);
        assert_eq!(r.relationship_type, "owns");
        assert_eq!(r.source_id, alice.id);
        assert_eq!(r.target_id, macbook.id);
    }

    #[test]
    fn test_entity_id_unique() {
        let a = EntityId::new();
        let b = EntityId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn test_value_conversions() {
        let v: Value = "hello".into();
        assert_eq!(v.as_str(), Some("hello"));

        let v: Value = 42.0.into();
        assert_eq!(v.as_number(), Some(42.0));

        let v: Value = true.into();
        assert!(matches!(v, Value::Boolean(true)));
    }
}
