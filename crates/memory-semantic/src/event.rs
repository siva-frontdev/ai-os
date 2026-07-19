use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Event emitted when a new concept is created in semantic memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptCreated {
    /// The generated concept ID.
    pub id: Uuid,
    /// Human-readable concept name.
    pub name: String,
    /// Assigned category.
    pub category: String,
    /// Creation timestamp (nanoseconds since epoch).
    pub timestamp: i64,
}

impl ConceptCreated {
    /// Unique event type identifier.
    pub fn event_type(&self) -> &'static str { "memory.semantic.concept_created" }

    /// Structured metadata for logging and observability.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        m.insert("concept_id".into(), self.id.to_string());
        m.insert("concept_name".into(), self.name.clone());
        m.insert("category".into(), self.category.clone());
        m
    }
}

/// Event emitted when an existing concept is updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptUpdated {
    /// The concept ID that was updated.
    pub id: Uuid,
    /// New name, if changed.
    pub name: Option<String>,
    /// New category, if changed.
    pub category: Option<String>,
    /// Update timestamp (nanoseconds since epoch).
    pub timestamp: i64,
}

impl ConceptUpdated {
    /// Unique event type identifier.
    pub fn event_type(&self) -> &'static str { "memory.semantic.concept_updated" }

    /// Structured metadata for logging and observability.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        m.insert("concept_id".into(), self.id.to_string());
        if let Some(ref n) = self.name { m.insert("name".into(), n.clone()); }
        m
    }
}

/// Event emitted when a new fact is created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactCreated {
    /// The generated fact ID.
    pub id: Uuid,
    /// Subject concept ID.
    pub subject: Uuid,
    /// Predicate (relationship label).
    pub predicate: String,
    /// Object value (concept ID or literal string).
    pub object: String,
    /// Confidence score [0.0, 1.0].
    pub confidence: f32,
    /// Creation timestamp (nanoseconds since epoch).
    pub timestamp: i64,
}

impl FactCreated {
    /// Unique event type identifier.
    pub fn event_type(&self) -> &'static str { "memory.semantic.fact_created" }

    /// Structured metadata for logging and observability.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        m.insert("fact_id".into(), self.id.to_string());
        m.insert("subject".into(), self.subject.to_string());
        m.insert("predicate".into(), self.predicate.clone());
        m.insert("confidence".into(), self.confidence.to_string());
        m
    }
}

/// Event emitted when an existing fact is updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactUpdated {
    /// The fact ID that was updated.
    pub id: Uuid,
    /// New object value, if changed.
    pub object: Option<String>,
    /// New confidence score, if changed.
    pub confidence: Option<f32>,
    /// Update timestamp (nanoseconds since epoch).
    pub timestamp: i64,
}

impl FactUpdated {
    /// Unique event type identifier.
    pub fn event_type(&self) -> &'static str { "memory.semantic.fact_updated" }

    /// Structured metadata for logging and observability.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        m.insert("fact_id".into(), self.id.to_string());
        if let Some(c) = self.confidence { m.insert("confidence".into(), c.to_string()); }
        m
    }
}
