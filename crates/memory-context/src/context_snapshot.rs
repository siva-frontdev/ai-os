use async_trait::async_trait;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::context_manager::ContextId;
use crate::error::{ContextManagerError, ContextManagerResult};

pub type ContextSnapshotId = Uuid;

/// A serializable representation of a context subtree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshotType {
    /// Unique snapshot ID.
    pub snapshot_id: ContextSnapshotId,
    /// The root context this snapshot represents.
    pub root_context_id: ContextId,
    /// All key-value pairs captured recursively.
    pub values: HashMap<String, HashMap<String, Vec<u8>>>,
    /// When this snapshot was taken.
    pub created: Timestamp,
}

impl ContextSnapshotType {
    /// Create a new empty snapshot specification.
    pub fn new(root_context_id: ContextId) -> Self {
        Self {
            snapshot_id: Uuid::now_v7(),
            root_context_id,
            values: HashMap::new(),
            created: Timestamp::now(),
        }
    }
}

/// Trait for capturing, restoring, and diffing context snapshots.
#[async_trait]
pub trait ContextSnapshot: Send + Sync + std::fmt::Debug {
    /// Capture the current state of the given context subtree as JSON bytes.
    async fn capture(&self, ctx: &ContextId) -> ContextManagerResult<ContextSnapshotType>;
    /// Restore a context subtree from a snapshot, returning the restored root ID.
    async fn apply(&self, snapshot: &ContextSnapshotType) -> ContextManagerResult<ContextId>;
    /// Compute a diff between two snapshots.
    /// Returns only keys that changed.
    async fn diff(
        &self,
        before: &ContextSnapshotType,
        after: &ContextSnapshotType,
    ) -> ContextManagerResult<HashMap<String, (Option<Vec<u8>>, Option<Vec<u8>>)>>;
    /// List all previously captured snapshots (includes self if persisted).
    async fn list(&self) -> ContextManagerResult<Vec<ContextSnapshotType>>;
}

/// No-op default implementation.
#[derive(Debug, Default)]
pub struct DefaultContextSnapshot;

#[async_trait]
impl ContextSnapshot for DefaultContextSnapshot {
    async fn capture(&self, _ctx: &ContextId) -> ContextManagerResult<ContextSnapshotType> {
        Err(ContextManagerError::Internal(
            "DefaultContextSnapshot not configured".into(),
        ))
    }
    async fn apply(&self, _snapshot: &ContextSnapshotType) -> ContextManagerResult<ContextId> {
        Err(ContextManagerError::Internal(
            "DefaultContextSnapshot not configured".into(),
        ))
    }
    async fn diff(
        &self,
        _before: &ContextSnapshotType,
        _after: &ContextSnapshotType,
    ) -> ContextManagerResult<HashMap<String, (Option<Vec<u8>>, Option<Vec<u8>>)>> {
        Ok(HashMap::new())
    }
    async fn list(&self) -> ContextManagerResult<Vec<ContextSnapshotType>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_type_new() {
        let id = Uuid::new_v4();
        let snap = ContextSnapshotType::new(id);
        assert_eq!(snap.root_context_id, id);
        assert_eq!(snap.values.len(), 0);
    }
}
