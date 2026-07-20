use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Event emitted when a new entity is added to the knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeAdded {
    /// Entity ID.
    pub id: Uuid,
    /// Entity name.
    pub name: String,
    /// Entity type label.
    pub entity_type: String,
    /// Creation timestamp (nanoseconds since epoch).
    pub timestamp: i64,
}

impl KnowledgeAdded {
    /// Unique event type identifier.
    pub fn event_type(&self) -> &'static str {
        "memory.knowledge.knowledge_added"
    }

    /// Structured metadata for logging and observability.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        m.insert("entity_id".into(), self.id.to_string());
        m.insert("entity_name".into(), self.name.clone());
        m.insert("entity_type".into(), self.entity_type.clone());
        m
    }
}

/// Event emitted when knowledge from multiple sources is merged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeMerged {
    /// Target entity ID receiving merged facts.
    pub target_id: Uuid,
    /// Source entity IDs that were merged in.
    pub source_ids: Vec<Uuid>,
    /// Number of facts merged.
    pub merged_fact_count: usize,
    /// Merge timestamp (nanoseconds since epoch).
    pub timestamp: i64,
}

impl KnowledgeMerged {
    /// Unique event type identifier.
    pub fn event_type(&self) -> &'static str {
        "memory.knowledge.knowledge_merged"
    }

    /// Structured metadata for logging and observability.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        m.insert("target_id".into(), self.target_id.to_string());
        m.insert(
            "merged_fact_count".into(),
            self.merged_fact_count.to_string(),
        );
        m
    }
}

/// Event emitted when knowledge is removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeRemoved {
    /// Entity ID removed.
    pub id: Uuid,
    /// Optional removal reason.
    pub reason: Option<String>,
    /// Removal timestamp (nanoseconds since epoch).
    pub timestamp: i64,
}

impl KnowledgeRemoved {
    /// Unique event type identifier.
    pub fn event_type(&self) -> &'static str {
        "memory.knowledge.knowledge_removed"
    }

    /// Structured metadata for logging and observability.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        m.insert("entity_id".into(), self.id.to_string());
        if let Some(ref r) = self.reason {
            m.insert("reason".into(), r.clone());
        }
        m
    }
}
