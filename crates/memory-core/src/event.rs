use serde::{Deserialize, Serialize};

use crate::types::{MemoryId, MemoryTier, MemoryType, Timestamp};
use std::collections::HashMap;

/// All events emitted by the Memory Platform.
///
/// Each variant corresponds to a specific event type string
/// following the `<module>.<event_name>` convention.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum MemoryEvent {
    // -- Object lifecycle events --
    /// A memory object was stored.
    #[serde(rename = "memory.object.stored")]
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

    /// A memory object was recalled (retrieved).
    #[serde(rename = "memory.object.recalled")]
    ObjectRecalled {
        /// The recalled object's ID.
        id: MemoryId,
        /// The cognitive memory type.
        memory_type: MemoryType,
        /// The storage tier.
        tier: MemoryTier,
        /// Retrieval latency in microseconds.
        latency_us: u64,
        /// Whether the result came from cache.
        cache_hit: bool,
    },

    /// A memory object was updated (new version created).
    #[serde(rename = "memory.object.updated")]
    ObjectUpdated {
        /// The updated object's ID.
        id: MemoryId,
        /// The new version number.
        new_version: u64,
        /// When the update occurred.
        timestamp: Timestamp,
    },

    /// A memory object was deleted.
    #[serde(rename = "memory.object.deleted")]
    ObjectDeleted {
        /// The deleted object's ID.
        id: MemoryId,
        /// The cognitive memory type.
        memory_type: MemoryType,
        /// The storage tier.
        tier: MemoryTier,
    },

    // -- Consolidation events --
    /// A memory object was consolidated to a higher tier.
    #[serde(rename = "memory.consolidated")]
    Consolidated {
        /// The original object's ID in the source tier.
        id: MemoryId,
        /// The source tier.
        from_tier: MemoryTier,
        /// The target tier.
        to_tier: MemoryTier,
        /// The new object's ID in the target tier.
        new_id: MemoryId,
    },

    /// Consolidation failed for a memory object.
    #[serde(rename = "memory.consolidation.failed")]
    ConsolidationFailed {
        /// The object's ID.
        id: MemoryId,
        /// The source tier.
        from_tier: MemoryTier,
        /// The failure reason.
        reason: String,
    },

    // -- Pruning events --
    /// A pruning cycle completed.
    #[serde(rename = "memory.pruned")]
    Pruned {
        /// IDs of removed objects.
        ids_removed: Vec<MemoryId>,
        /// Total bytes freed.
        bytes_freed: u64,
        /// The tier that was pruned.
        tier: MemoryTier,
        /// The policy name or description.
        policy: String,
    },

    // -- Context events --
    /// A new memory context was created.
    #[serde(rename = "memory.context.created")]
    ContextCreated {
        /// The new context ID.
        context_id: uuid::Uuid,
        /// The parent context ID, if any.
        parent_id: Option<uuid::Uuid>,
        /// The session that owns this context.
        session_id: String,
    },

    /// A memory context was destroyed.
    #[serde(rename = "memory.context.destroyed")]
    ContextDestroyed {
        /// The destroyed context ID.
        context_id: uuid::Uuid,
    },

    // -- Snapshot events --
    /// A memory snapshot was created.
    #[serde(rename = "memory.snapshot.created")]
    SnapshotCreated {
        /// The snapshot ID.
        snapshot_id: uuid::Uuid,
        /// Total size in bytes.
        size_bytes: u64,
        /// Scope description.
        scope: String,
    },

    /// A memory snapshot was restored.
    #[serde(rename = "memory.snapshot.restored")]
    SnapshotRestored {
        /// The snapshot ID that was restored.
        snapshot_id: uuid::Uuid,
        /// When the restore occurred.
        timestamp: Timestamp,
    },

    // -- Pattern discovery events --
    /// A new memory pattern was discovered.
    #[serde(rename = "memory.pattern.discovered")]
    PatternDiscovered {
        /// The pattern ID.
        pattern_id: uuid::Uuid,
        /// The pattern type.
        pattern_type: String,
        /// Confidence score (0.0 - 1.0).
        confidence: f32,
        /// Support count (number of instances).
        support: u64,
    },

    // -- Capacity events --
    /// A memory tier is approaching capacity.
    #[serde(rename = "memory.tier.capacity_warning")]
    TierCapacityWarning {
        /// The tier approaching capacity.
        tier: MemoryTier,
        /// Current usage percentage (0-100).
        usage_pct: u8,
        /// Current bytes used.
        current_bytes: u64,
        /// Maximum bytes allowed.
        max_bytes: u64,
    },

    // -- Index events --
    /// The search index was rebuilt.
    #[serde(rename = "memory.index.rebuilt")]
    IndexRebuilt {
        /// Number of entries indexed.
        entries_indexed: u64,
        /// Elapsed time in milliseconds.
        elapsed_ms: u64,
        /// Embedding dimension.
        dimension: usize,
    },

    // -- Cache events --
    /// A cache entry was evicted.
    #[serde(rename = "memory.cache.eviction")]
    CacheEviction {
        /// The evicted cache key.
        key: String,
        /// The reason for eviction.
        reason: String,
    },
}

impl MemoryEvent {
    /// Return the event type string (e.g. `"memory.object.stored"`).
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ObjectStored { .. } => "memory.object.stored",
            Self::ObjectRecalled { .. } => "memory.object.recalled",
            Self::ObjectUpdated { .. } => "memory.object.updated",
            Self::ObjectDeleted { .. } => "memory.object.deleted",
            Self::Consolidated { .. } => "memory.consolidated",
            Self::ConsolidationFailed { .. } => "memory.consolidation.failed",
            Self::Pruned { .. } => "memory.pruned",
            Self::ContextCreated { .. } => "memory.context.created",
            Self::ContextDestroyed { .. } => "memory.context.destroyed",
            Self::SnapshotCreated { .. } => "memory.snapshot.created",
            Self::SnapshotRestored { .. } => "memory.snapshot.restored",
            Self::PatternDiscovered { .. } => "memory.pattern.discovered",
            Self::TierCapacityWarning { .. } => "memory.tier.capacity_warning",
            Self::IndexRebuilt { .. } => "memory.index.rebuilt",
            Self::CacheEviction { .. } => "memory.cache.eviction",
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
            Self::ObjectRecalled { id, latency_us, .. } => {
                m.insert("id".into(), id.to_string());
                m.insert("latency_us".into(), latency_us.to_string());
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
            Self::Consolidated { id, new_id, .. } => {
                m.insert("id".into(), id.to_string());
                m.insert("new_id".into(), new_id.to_string());
            }
            Self::ConsolidationFailed { id, reason, .. } => {
                m.insert("id".into(), id.to_string());
                m.insert("reason".into(), reason.clone());
            }
            Self::Pruned { ids_removed, .. } => {
                m.insert("count".into(), ids_removed.len().to_string());
            }
            Self::ContextCreated { context_id, .. } => {
                m.insert("context_id".into(), context_id.to_string());
            }
            Self::ContextDestroyed { context_id } => {
                m.insert("context_id".into(), context_id.to_string());
            }
            Self::SnapshotCreated { snapshot_id, .. } => {
                m.insert("snapshot_id".into(), snapshot_id.to_string());
            }
            Self::SnapshotRestored { snapshot_id, .. } => {
                m.insert("snapshot_id".into(), snapshot_id.to_string());
            }
            Self::PatternDiscovered { pattern_id, .. } => {
                m.insert("pattern_id".into(), pattern_id.to_string());
            }
            Self::TierCapacityWarning {
                tier, usage_pct, ..
            } => {
                m.insert("tier".into(), format!("{:?}", tier));
                m.insert("usage_pct".into(), usage_pct.to_string());
            }
            Self::IndexRebuilt {
                entries_indexed, ..
            } => {
                m.insert("entries_indexed".into(), entries_indexed.to_string());
            }
            Self::CacheEviction { key, .. } => {
                m.insert("key".into(), key.clone());
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

        let events: Vec<MemoryEvent> = vec![
            MemoryEvent::ObjectStored {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
                timestamp: ts,
            },
            MemoryEvent::ObjectRecalled {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
                latency_us: 5,
                cache_hit: true,
            },
            MemoryEvent::ObjectUpdated {
                id,
                new_version: 2,
                timestamp: ts,
            },
            MemoryEvent::ObjectDeleted {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
            },
            MemoryEvent::Consolidated {
                id,
                from_tier: MemoryTier::Working,
                to_tier: MemoryTier::Episodic,
                new_id: MemoryId::new(),
            },
            MemoryEvent::ConsolidationFailed {
                id,
                from_tier: MemoryTier::Working,
                reason: "timeout".into(),
            },
            MemoryEvent::Pruned {
                ids_removed: vec![id],
                bytes_freed: 1024,
                tier: MemoryTier::Working,
                policy: "ttl".into(),
            },
            MemoryEvent::ContextCreated {
                context_id: uuid::Uuid::new_v4(),
                parent_id: None,
                session_id: "sess".into(),
            },
            MemoryEvent::ContextDestroyed {
                context_id: uuid::Uuid::new_v4(),
            },
            MemoryEvent::SnapshotCreated {
                snapshot_id: uuid::Uuid::new_v4(),
                size_bytes: 4096,
                scope: "full".into(),
            },
            MemoryEvent::SnapshotRestored {
                snapshot_id: uuid::Uuid::new_v4(),
                timestamp: ts,
            },
            MemoryEvent::PatternDiscovered {
                pattern_id: uuid::Uuid::new_v4(),
                pattern_type: "temporal".into(),
                confidence: 0.85,
                support: 42,
            },
            MemoryEvent::TierCapacityWarning {
                tier: MemoryTier::Working,
                usage_pct: 95,
                current_bytes: 95000,
                max_bytes: 100000,
            },
            MemoryEvent::IndexRebuilt {
                entries_indexed: 1000,
                elapsed_ms: 250,
                dimension: 384,
            },
            MemoryEvent::CacheEviction {
                key: "mem:123".into(),
                reason: "ttl_expired".into(),
            },
        ];

        let expected_types = [
            "memory.object.stored",
            "memory.object.recalled",
            "memory.object.updated",
            "memory.object.deleted",
            "memory.consolidated",
            "memory.consolidation.failed",
            "memory.pruned",
            "memory.context.created",
            "memory.context.destroyed",
            "memory.snapshot.created",
            "memory.snapshot.restored",
            "memory.pattern.discovered",
            "memory.tier.capacity_warning",
            "memory.index.rebuilt",
            "memory.cache.eviction",
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
        let event = MemoryEvent::ObjectStored {
            id,
            memory_type: MemoryType::Working,
            tier: MemoryTier::Working,
            timestamp: Timestamp::now(),
        };
        let meta = event.metadata();
        assert_eq!(meta.get("event_type").unwrap(), "memory.object.stored");
        assert_eq!(meta.get("id").unwrap(), &id.to_string());
    }

    #[test]
    fn test_all_events_produce_metadata() {
        let id = MemoryId::new();
        let ts = Timestamp::now();
        let events: Vec<MemoryEvent> = vec![
            MemoryEvent::ObjectStored {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
                timestamp: ts,
            },
            MemoryEvent::ObjectRecalled {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
                latency_us: 0,
                cache_hit: false,
            },
            MemoryEvent::ObjectUpdated {
                id,
                new_version: 1,
                timestamp: ts,
            },
            MemoryEvent::ObjectDeleted {
                id,
                memory_type: MemoryType::Working,
                tier: MemoryTier::Working,
            },
            MemoryEvent::Consolidated {
                id,
                from_tier: MemoryTier::Working,
                to_tier: MemoryTier::Episodic,
                new_id: MemoryId::new(),
            },
            MemoryEvent::ConsolidationFailed {
                id,
                from_tier: MemoryTier::Working,
                reason: "".into(),
            },
            MemoryEvent::Pruned {
                ids_removed: vec![],
                bytes_freed: 0,
                tier: MemoryTier::Working,
                policy: "".into(),
            },
            MemoryEvent::ContextCreated {
                context_id: uuid::Uuid::new_v4(),
                parent_id: None,
                session_id: "".into(),
            },
            MemoryEvent::ContextDestroyed {
                context_id: uuid::Uuid::new_v4(),
            },
            MemoryEvent::SnapshotCreated {
                snapshot_id: uuid::Uuid::new_v4(),
                size_bytes: 0,
                scope: "".into(),
            },
            MemoryEvent::SnapshotRestored {
                snapshot_id: uuid::Uuid::new_v4(),
                timestamp: ts,
            },
            MemoryEvent::PatternDiscovered {
                pattern_id: uuid::Uuid::new_v4(),
                pattern_type: "".into(),
                confidence: 0.0,
                support: 0,
            },
            MemoryEvent::TierCapacityWarning {
                tier: MemoryTier::Working,
                usage_pct: 0,
                current_bytes: 0,
                max_bytes: 0,
            },
            MemoryEvent::IndexRebuilt {
                entries_indexed: 0,
                elapsed_ms: 0,
                dimension: 0,
            },
            MemoryEvent::CacheEviction {
                key: "".into(),
                reason: "".into(),
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
