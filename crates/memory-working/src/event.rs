use memory_core::{MemoryId, MemoryTier, MemoryType, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All events emitted by the working memory subsystem.
///
/// Each variant corresponds to a specific working memory operation
/// and carries the relevant contextual data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum WorkingMemoryEvent {
    /// A memory object was stored in working memory.
    #[serde(rename = "working.object.stored")]
    ObjectStored {
        /// The stored object's ID.
        id: MemoryId,
        /// The cognitive memory type.
        memory_type: MemoryType,
        /// The storage tier.
        tier: MemoryTier,
        /// When the event was created.
        timestamp: Timestamp,
    },

    /// A memory object in working memory was updated.
    #[serde(rename = "working.object.updated")]
    ObjectUpdated {
        /// The updated object's ID.
        id: MemoryId,
        /// The new version number.
        new_version: u64,
        /// When the update occurred.
        timestamp: Timestamp,
    },

    /// A memory object was deleted from working memory.
    #[serde(rename = "working.object.deleted")]
    ObjectDeleted {
        /// The deleted object's ID.
        id: MemoryId,
        /// The cognitive memory type.
        memory_type: MemoryType,
        /// The storage tier.
        tier: MemoryTier,
    },

    /// A memory object was recalled (retrieved) from working memory.
    #[serde(rename = "working.object.recalled")]
    ObjectRecalled {
        /// The recalled object's ID.
        id: MemoryId,
        /// Retrieval latency in microseconds.
        latency_us: u64,
    },

    /// A memory object expired from working memory.
    #[serde(rename = "working.object.expired")]
    ObjectExpired {
        /// The expired object's ID.
        id: MemoryId,
    },

    /// The active goal changed in a session.
    #[serde(rename = "working.goal.changed")]
    GoalChanged {
        /// The new goal description.
        new_goal: String,
        /// The session that changed goals.
        session_id: uuid::Uuid,
        /// When the change occurred.
        timestamp: Timestamp,
    },

    /// The active task changed in a session.
    #[serde(rename = "working.task.changed")]
    TaskChanged {
        /// The task identifier.
        task_id: String,
        /// The new task status.
        status: String,
        /// The session that changed tasks.
        session_id: uuid::Uuid,
    },

    /// Working memory capacity is approaching its limit.
    #[serde(rename = "working.capacity.warning")]
    CapacityWarning {
        /// Current usage percentage (0-100).
        usage_pct: u8,
        /// Current bytes used.
        current_bytes: u64,
        /// Maximum bytes allowed.
        max_bytes: u64,
    },
}

impl WorkingMemoryEvent {
    /// Return the event type string (e.g. `"working.object.stored"`).
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ObjectStored { .. } => "working.object.stored",
            Self::ObjectUpdated { .. } => "working.object.updated",
            Self::ObjectDeleted { .. } => "working.object.deleted",
            Self::ObjectRecalled { .. } => "working.object.recalled",
            Self::ObjectExpired { .. } => "working.object.expired",
            Self::GoalChanged { .. } => "working.goal.changed",
            Self::TaskChanged { .. } => "working.task.changed",
            Self::CapacityWarning { .. } => "working.capacity.warning",
        }
    }

    /// Return a map of key-value metadata for structured logging.
    pub fn metadata(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("event_type".into(), self.event_type().into());
        match self {
            Self::ObjectStored { id, .. } => {
                m.insert("id".into(), id.to_string());
            }
            Self::ObjectUpdated {
                id, new_version, ..
            } => {
                m.insert("id".into(), id.to_string());
                m.insert("new_version".into(), new_version.to_string());
            }
            Self::ObjectDeleted { id, .. } => {
                m.insert("id".into(), id.to_string());
            }
            Self::ObjectRecalled { id, latency_us, .. } => {
                m.insert("id".into(), id.to_string());
                m.insert("latency_us".into(), latency_us.to_string());
            }
            Self::ObjectExpired { id } => {
                m.insert("id".into(), id.to_string());
            }
            Self::GoalChanged {
                new_goal,
                session_id,
                ..
            } => {
                m.insert("new_goal".into(), new_goal.clone());
                m.insert("session_id".into(), session_id.to_string());
            }
            Self::TaskChanged {
                task_id,
                status,
                session_id,
            } => {
                m.insert("task_id".into(), task_id.clone());
                m.insert("status".into(), status.clone());
                m.insert("session_id".into(), session_id.to_string());
            }
            Self::CapacityWarning {
                usage_pct,
                current_bytes,
                max_bytes,
            } => {
                m.insert("usage_pct".into(), usage_pct.to_string());
                m.insert("current_bytes".into(), current_bytes.to_string());
                m.insert("max_bytes".into(), max_bytes.to_string());
            }
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_strings() {
        let id = MemoryId::new();
        let ts = Timestamp::now();
        let session_id = uuid::Uuid::new_v4();

        let events: Vec<WorkingMemoryEvent> = vec![
            WorkingMemoryEvent::ObjectStored {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
                timestamp: ts,
            },
            WorkingMemoryEvent::ObjectUpdated {
                id,
                new_version: 2,
                timestamp: ts,
            },
            WorkingMemoryEvent::ObjectDeleted {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
            },
            WorkingMemoryEvent::ObjectRecalled { id, latency_us: 5 },
            WorkingMemoryEvent::ObjectExpired { id },
            WorkingMemoryEvent::GoalChanged {
                new_goal: "test".into(),
                session_id,
                timestamp: ts,
            },
            WorkingMemoryEvent::TaskChanged {
                task_id: "task-1".into(),
                status: "running".into(),
                session_id,
            },
            WorkingMemoryEvent::CapacityWarning {
                usage_pct: 95,
                current_bytes: 95000,
                max_bytes: 100000,
            },
        ];

        let expected_types = [
            "working.object.stored",
            "working.object.updated",
            "working.object.deleted",
            "working.object.recalled",
            "working.object.expired",
            "working.goal.changed",
            "working.task.changed",
            "working.capacity.warning",
        ];

        for (event, expected) in events.iter().zip(expected_types.iter()) {
            assert_eq!(
                event.event_type(),
                *expected,
                "event type mismatch for {:?}",
                event
            );
        }
    }

    #[test]
    fn test_event_metadata() {
        let id = MemoryId::new();
        let event = WorkingMemoryEvent::ObjectStored {
            id,
            memory_type: MemoryType::Working,
            tier: MemoryTier::Working,
            timestamp: Timestamp::now(),
        };
        let meta = event.metadata();
        assert_eq!(meta.get("event_type").unwrap(), "working.object.stored");
        assert_eq!(meta.get("id").unwrap(), &id.to_string());
    }

    #[test]
    fn test_all_events_produce_metadata() {
        let id = MemoryId::new();
        let ts = Timestamp::now();
        let session_id = uuid::Uuid::new_v4();
        let events: Vec<WorkingMemoryEvent> = vec![
            WorkingMemoryEvent::ObjectStored {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
                timestamp: ts,
            },
            WorkingMemoryEvent::ObjectUpdated {
                id,
                new_version: 1,
                timestamp: ts,
            },
            WorkingMemoryEvent::ObjectDeleted {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
            },
            WorkingMemoryEvent::ObjectRecalled { id, latency_us: 0 },
            WorkingMemoryEvent::ObjectExpired { id },
            WorkingMemoryEvent::GoalChanged {
                new_goal: "".into(),
                session_id,
                timestamp: ts,
            },
            WorkingMemoryEvent::TaskChanged {
                task_id: "".into(),
                status: "".into(),
                session_id,
            },
            WorkingMemoryEvent::CapacityWarning {
                usage_pct: 0,
                current_bytes: 0,
                max_bytes: 0,
            },
        ];
        for event in &events {
            let meta = event.metadata();
            assert!(
                meta.contains_key("event_type"),
                "missing event_type for {:?}",
                event
            );
        }
    }
}
