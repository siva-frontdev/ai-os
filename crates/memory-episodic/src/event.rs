use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An episodic event recorded in long-term memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicEvent {
    pub id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub event_type: String,
    pub timestamp: Timestamp,
    pub duration_ms: Option<u64>,
    pub participants: Vec<String>,
    pub location: Option<String>,
    pub sequence_id: Option<uuid::Uuid>,
    pub content: Vec<u8>,
    pub metadata: HashMap<String, String>,
    pub importance: f32,
    pub confidence: f32,
}

impl EpisodicEvent {
    pub fn event_type(&self) -> &'static str {
        "memory.episodic.event"
    }

    /// Structured metadata for logging and observability.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        m.insert("event_id".into(), self.id.to_string());
        m.insert("session_id".into(), self.session_id.to_string());
        m.insert("event_type_name".into(), self.event_type.clone());
        m
    }
}
