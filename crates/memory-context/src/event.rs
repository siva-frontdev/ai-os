use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use memory_core::{Timestamp, MemoryId};

/// The type of change applied to a context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextChangeType {
    /// A key-value pair was set.
    ValueSet,
    /// A key-value pair was removed.
    ValueDeleted,
    /// A new context was created.
    ContextCreated,
    /// A context was destroyed.
    ContextDestroyed,
    /// Two contexts were merged.
    ContextMerged,
    /// A context snapshot was restored.
    ContextRestored,
}

/// An event emitted when a context changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum ContextEvent {
    #[serde(rename = "memory.context.changed")]
    ContextChanged {
        /// The context that changed.
        context_id: uuid::Uuid,
        /// The type of change.
        change_type: ContextChangeType,
        /// The key affected (if any).
        key: Option<String>,
        /// When the change occurred.
        timestamp: Timestamp,
    },
}

impl ContextEvent {
    /// Return the event type string.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ContextChanged { .. } => "memory.context.changed",
        }
    }

    /// Return a map of key-value metadata for structured logging.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        match self {
            Self::ContextChanged { context_id, change_type, key, .. } => {
                m.insert("context_id".into(), context_id.to_string());
                m.insert("change_type".into(), format!("{:?}", change_type));
                if let Some(k) = key {
                    m.insert("key".into(), k.clone());
                }
            }
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        let ev = ContextEvent::ContextChanged {
            context_id: uuid::Uuid::new_v4(),
            change_type: ContextChangeType::ValueSet,
            key: Some("answer".into()),
            timestamp: Timestamp::now(),
        };
        assert_eq!(ev.event_type(), "memory.context.changed");
    }

    #[test]
    fn test_metadata() {
        let ev = ContextEvent::ContextChanged {
            context_id: uuid::Uuid::new_v4(),
            change_type: ContextChangeType::ContextCreated,
            key: None,
            timestamp: Timestamp::now(),
        };
        let meta = ev.metadata();
        assert_eq!(meta.get("event_type").unwrap(), "memory.context.changed");
        assert!(meta.contains_key("context_id"));
    }
}
