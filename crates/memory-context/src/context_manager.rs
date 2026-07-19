use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use memory_core::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ContextManagerError, ContextManagerResult};

pub type ContextId = Uuid;

/// A single node in the context tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextNode {
    /// Unique identifier for this context.
    pub id: ContextId,
    /// Parent context, if any. Root contexts have no parent.
    pub parent_id: Option<ContextId>,
    /// Key-value pairs stored in this context. Child contexts inherit
    /// parent values, so lookup traverses up the tree.
    pub values: HashMap<String, Vec<u8>>,
    /// When this context was created (UNIX epoch nanoseconds).
    pub created: Timestamp,
    /// IDs of direct child contexts.
    pub children: Vec<ContextId>,
}

impl ContextNode {
    fn new(id: ContextId, parent_id: Option<ContextId>) -> Self {
        Self {
            id,
            parent_id,
            values: HashMap::new(),
            created: Timestamp::now(),
            children: Vec::new(),
        }
    }
}

/// Statistics about the context manager's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagerStats {
    /// Total number of contexts currently stored.
    pub total_contexts: u64,
    /// Total number of key-value pairs across all contexts.
    pub total_values: u64,
    /// Maximum depth of the context tree (root = depth 1).
    pub max_depth: usize,
    /// Number of contexts that have a parent.
    pub contexts_with_parents: u64,
}

/// Manages hierarchical execution contexts for the AI-native OS.
///
/// Contexts form a tree where child contexts inherit values from their
/// parents and can override them. This models the AI's execution state:
/// global defaults at the root, session-specific overrides in intermediate
/// nodes, and task-specific values at the leaves.
///
/// Contexts are not persisted — they are scoped to the current platform
/// runtime. Snapshot/restore can be used to persist them.
#[async_trait]
pub trait ContextManager: Send + Sync + std::fmt::Debug {
    /// Create a new context. If `parent` is provided, it must already exist.
    /// The child inherits all parent values (read-through).
    async fn create_context(&self, parent: Option<ContextId>) -> ContextManagerResult<ContextId>;

    /// Destroy a context and all of its descendants.
    async fn destroy_context(&self, id: &ContextId) -> ContextManagerResult<()>;

    /// Set a key-value pair in a context. Overrides any inherited value.
    async fn set_value(
        &self,
        ctx: &ContextId,
        key: &str,
        value: Vec<u8>,
    ) -> ContextManagerResult<()>;

    /// Get a value from a context. Traverses up the tree if not found locally.
    async fn get_value(&self, ctx: &ContextId, key: &str) -> ContextManagerResult<Option<Vec<u8>>>;

    /// Remove a key-value pair from a context (local only, does not affect parents).
    async fn delete_value(&self, ctx: &ContextId, key: &str) -> ContextManagerResult<()>;

    /// Serialize a context subtree as JSON bytes (for persistence).
    async fn snapshot(&self, ctx: &ContextId) -> ContextManagerResult<Vec<u8>>;

    /// Deserialize and restore a context subtree from JSON bytes.
    async fn restore(&self, snapshot: &[u8]) -> ContextManagerResult<ContextId>;

    /// Merge all values and children from `source` into `target`.
    /// Source values overwrite target values on conflict.
    async fn merge(&self, target: &ContextId, source: &ContextId) -> ContextManagerResult<()>;

    /// Return all currently active context IDs.
    async fn active_contexts(&self) -> ContextManagerResult<Vec<ContextId>>;

    /// Return aggregate statistics about the context tree.
    async fn stats(&self) -> ContextManagerResult<ContextManagerStats>;
}

/// Tree-backed implementation of `ContextManager`.
///
/// Uses `std::sync::RwLock<HashMap<ContextId, ContextNode>>` for
/// thread-safe access. Critical sections are short (single hash map
/// lookups) and never held across `.await` points.
#[derive(Debug, Default)]
pub struct TreeContextManager {
    contexts: RwLock<HashMap<ContextId, ContextNode>>,
}

impl TreeContextManager {
    /// Create a new empty `TreeContextManager`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute the depth of a context in the tree (root = 1).
    fn depth(&self, ctx: &ContextId, visited: &mut Vec<ContextId>) -> usize {
        if visited.contains(ctx) {
            return 0;
        }
        visited.push(*ctx);

        let contexts = self.contexts.read().unwrap();
        let node = match contexts.get(ctx) {
            Some(n) => n,
            None => return 0,
        };

        let parent_depth = match node.parent_id {
            Some(pid) => self.depth(&pid, visited),
            None => 0,
        };

        parent_depth + 1
    }

    /// Collect all descendant IDs of a context (BFS, includes self).
    fn collect_descendants(&self, id: &ContextId) -> ContextManagerResult<Vec<ContextId>> {
        let contexts = self.contexts.read().unwrap();
        let mut result = Vec::new();
        let mut queue = vec![*id];

        while let Some(current) = queue.pop() {
            if !contexts.contains_key(&current) {
                return Err(ContextManagerError::ContextNotFound(current));
            }
            result.push(current);
            let node = &contexts[&current];
            queue.extend(node.children.iter().copied());
        }

        Ok(result)
    }
}

#[async_trait]
impl ContextManager for TreeContextManager {
    async fn create_context(&self, parent: Option<ContextId>) -> ContextManagerResult<ContextId> {
        if let Some(pid) = parent {
            let contexts = self.contexts.read().unwrap();
            if !contexts.contains_key(&pid) {
                return Err(ContextManagerError::ParentNotFound(pid));
            }
        }

        let id = Uuid::now_v7();
        let mut contexts = self.contexts.write().unwrap();
        contexts.insert(id, ContextNode::new(id, parent));

        if let Some(pid) = parent {
            if let Some(parent_node) = contexts.get_mut(&pid) {
                parent_node.children.push(id);
            }
        }

        Ok(id)
    }

    async fn destroy_context(&self, id: &ContextId) -> ContextManagerResult<()> {
        if *id == Uuid::nil() {
            return Err(ContextManagerError::InvalidOperation(
                "cannot destroy nil context".into(),
            ));
        }

        let to_destroy = self.collect_descendants(id)?;

        let mut contexts = self.contexts.write().unwrap();

        for did in &to_destroy {
            let node = contexts
                .remove(did)
                .ok_or_else(|| ContextManagerError::ContextNotFound(*did))?;

            if let Some(pid) = node.parent_id {
                if let Some(parent) = contexts.get_mut(&pid) {
                    parent.children.retain(|c| c != did);
                }
            }
        }

        Ok(())
    }

    async fn set_value(
        &self,
        ctx: &ContextId,
        key: &str,
        value: Vec<u8>,
    ) -> ContextManagerResult<()> {
        let mut contexts = self.contexts.write().unwrap();
        let node = contexts
            .get_mut(ctx)
            .ok_or_else(|| ContextManagerError::ContextNotFound(*ctx))?;
        node.values.insert(key.to_string(), value);
        Ok(())
    }

    async fn get_value(&self, ctx: &ContextId, key: &str) -> ContextManagerResult<Option<Vec<u8>>> {
        let contexts = self.contexts.read().unwrap();
        let node = contexts
            .get(ctx)
            .ok_or_else(|| ContextManagerError::ContextNotFound(*ctx))?;

        if let Some(val) = node.values.get(key) {
            return Ok(Some(val.clone()));
        }

        // Walk up parent chain (inheritance)
        let mut current_id = node.parent_id;
        while let Some(pid) = current_id {
            let parent = contexts
                .get(&pid)
                .ok_or_else(|| ContextManagerError::ParentNotFound(pid))?;
            if let Some(val) = parent.values.get(key) {
                return Ok(Some(val.clone()));
            }
            current_id = parent.parent_id;
        }

        Ok(None)
    }

    async fn delete_value(&self, ctx: &ContextId, key: &str) -> ContextManagerResult<()> {
        let mut contexts = self.contexts.write().unwrap();
        let node = contexts
            .get_mut(ctx)
            .ok_or_else(|| ContextManagerError::ContextNotFound(*ctx))?;
        node.values.remove(key);
        Ok(())
    }

    async fn snapshot(&self, ctx: &ContextId) -> ContextManagerResult<Vec<u8>> {
        let contexts = self.contexts.read().unwrap();
        let node = contexts
            .get(ctx)
            .ok_or_else(|| ContextManagerError::ContextNotFound(*ctx))?;
        serde_json::to_vec(node).map_err(|e| ContextManagerError::SerializationError(e.to_string()))
    }

    async fn restore(&self, snapshot: &[u8]) -> ContextManagerResult<ContextId> {
        let mut node: ContextNode = serde_json::from_slice(snapshot)
            .map_err(|e| ContextManagerError::SerializationError(e.to_string()))?;

        // Assign a fresh ID so restore always creates a new context tree.
        let new_id = Uuid::now_v7();
        let parent_id = node.parent_id;
        node.id = new_id;
        node.parent_id = parent_id;
        node.children.clear();

        let mut contexts = self.contexts.write().unwrap();

        if contexts.contains_key(&new_id) {
            return Err(ContextManagerError::ContextAlreadyExists(new_id));
        }

        contexts.insert(new_id, node.clone());

        if let Some(pid) = parent_id {
            if let Some(parent) = contexts.get_mut(&pid) {
                if !parent.children.contains(&new_id) {
                    parent.children.push(new_id);
                }
            }
        }

        Ok(new_id)
    }

    async fn merge(&self, target: &ContextId, source: &ContextId) -> ContextManagerResult<()> {
        let source_node = {
            let contexts = self.contexts.read().unwrap();
            contexts
                .get(source)
                .ok_or_else(|| ContextManagerError::ContextNotFound(*source))?
                .clone()
        };

        let mut contexts = self.contexts.write().unwrap();
        let target_node = contexts
            .get_mut(target)
            .ok_or_else(|| ContextManagerError::ContextNotFound(*target))?;

        // Merge values (source overwrites target on conflict)
        for (k, v) in source_node.values {
            target_node.values.insert(k, v);
        }

        // Merge children
        for child in source_node.children {
            if !target_node.children.contains(&child) {
                target_node.children.push(child);
            }
        }

        Ok(())
    }

    async fn active_contexts(&self) -> ContextManagerResult<Vec<ContextId>> {
        let contexts = self.contexts.read().unwrap();
        Ok(contexts.keys().cloned().collect())
    }

    async fn stats(&self) -> ContextManagerResult<ContextManagerStats> {
        let contexts = self.contexts.read().unwrap();
        let total_contexts = contexts.len() as u64;
        let total_values: u64 = contexts.values().map(|n| n.values.len() as u64).sum();
        let contexts_with_parents =
            contexts.values().filter(|n| n.parent_id.is_some()).count() as u64;

        let mut max_depth = 0usize;
        for id in contexts.keys() {
            let mut visited = Vec::new();
            let depth = self.depth(id, &mut visited);
            max_depth = max_depth.max(depth);
        }

        Ok(ContextManagerStats {
            total_contexts,
            total_values,
            max_depth,
            contexts_with_parents,
        })
    }
}

#[derive(Debug, Default)]
pub struct DefaultContextManager;

#[async_trait]
impl ContextManager for DefaultContextManager {
    async fn create_context(&self, _parent: Option<ContextId>) -> ContextManagerResult<ContextId> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
    async fn destroy_context(&self, _id: &ContextId) -> ContextManagerResult<()> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
    async fn set_value(
        &self,
        _ctx: &ContextId,
        _key: &str,
        _value: Vec<u8>,
    ) -> ContextManagerResult<()> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
    async fn get_value(
        &self,
        _ctx: &ContextId,
        _key: &str,
    ) -> ContextManagerResult<Option<Vec<u8>>> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
    async fn delete_value(&self, _ctx: &ContextId, _key: &str) -> ContextManagerResult<()> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
    async fn snapshot(&self, _ctx: &ContextId) -> ContextManagerResult<Vec<u8>> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
    async fn restore(&self, _snapshot: &[u8]) -> ContextManagerResult<ContextId> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
    async fn merge(&self, _target: &ContextId, _source: &ContextId) -> ContextManagerResult<()> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
    async fn active_contexts(&self) -> ContextManagerResult<Vec<ContextId>> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
    async fn stats(&self) -> ContextManagerResult<ContextManagerStats> {
        Err(ContextManagerError::Internal(
            "DefaultContextManager not configured".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> TreeContextManager {
        TreeContextManager::new()
    }

    #[tokio::test]
    async fn test_create_and_destroy() {
        let mgr = make_manager();
        let id = mgr.create_context(None).await.unwrap();
        assert!(mgr.get_value(&id, "anything").await.unwrap().is_none());
        mgr.destroy_context(&id).await.unwrap();
        assert!(mgr.get_value(&id, "anything").await.is_err());
    }

    #[tokio::test]
    async fn test_value_inheritance() {
        let mgr = make_manager();
        let parent = mgr.create_context(None).await.unwrap();
        mgr.set_value(&parent, "color", b"blue".to_vec())
            .await
            .unwrap();

        let child = mgr.create_context(Some(parent)).await.unwrap();
        assert_eq!(
            mgr.get_value(&child, "color").await.unwrap(),
            Some(b"blue".to_vec())
        );
    }

    #[tokio::test]
    async fn test_value_override() {
        let mgr = make_manager();
        let parent = mgr.create_context(None).await.unwrap();
        mgr.set_value(&parent, "color", b"blue".to_vec())
            .await
            .unwrap();

        let child = mgr.create_context(Some(parent)).await.unwrap();
        mgr.set_value(&child, "color", b"red".to_vec())
            .await
            .unwrap();
        assert_eq!(
            mgr.get_value(&child, "color").await.unwrap(),
            Some(b"red".to_vec())
        );
    }

    #[tokio::test]
    async fn test_snapshot_and_restore() {
        let mgr = make_manager();
        let id = mgr.create_context(None).await.unwrap();
        mgr.set_value(&id, "key1", b"val1".to_vec()).await.unwrap();

        let snap = mgr.snapshot(&id).await.unwrap();
        let restored_id = mgr.restore(&snap).await.unwrap();

        assert_eq!(
            mgr.get_value(&restored_id, "key1").await.unwrap(),
            Some(b"val1".to_vec())
        );
    }

    #[tokio::test]
    async fn test_merge() {
        let mgr = make_manager();
        let target = mgr.create_context(None).await.unwrap();
        let child = mgr.create_context(Some(target)).await.unwrap();

        mgr.set_value(&child, "from_child", b"yes".to_vec())
            .await
            .unwrap();
        mgr.set_value(&target, "from_target", b"origin".to_vec())
            .await
            .unwrap();

        mgr.merge(&target, &child).await.unwrap();
        assert_eq!(
            mgr.get_value(&target, "from_child").await.unwrap(),
            Some(b"yes".to_vec())
        );
        assert_eq!(
            mgr.get_value(&target, "from_target").await.unwrap(),
            Some(b"origin".to_vec())
        );
    }

    #[tokio::test]
    async fn test_stats() {
        let mgr = make_manager();
        let root = mgr.create_context(None).await.unwrap();
        let child = mgr.create_context(Some(root)).await.unwrap();
        mgr.set_value(&root, "a", b"1".to_vec()).await.unwrap();
        mgr.set_value(&child, "b", b"2".to_vec()).await.unwrap();

        let stats = mgr.stats().await.unwrap();
        assert_eq!(stats.total_contexts, 2);
        assert_eq!(stats.total_values, 2);
        assert_eq!(stats.max_depth, 2);
        assert_eq!(stats.contexts_with_parents, 1);
    }

    #[tokio::test]
    async fn test_destroy_cascades_to_children() {
        let mgr = make_manager();
        let root = mgr.create_context(None).await.unwrap();
        let child = mgr.create_context(Some(root)).await.unwrap();

        mgr.destroy_context(&root).await.unwrap();
        assert!(mgr.get_value(&child, "anything").await.is_err());
    }

    #[tokio::test]
    async fn test_active_contexts() {
        let mgr = make_manager();
        let a = mgr.create_context(None).await.unwrap();
        let b = mgr.create_context(None).await.unwrap();

        let ids = mgr.active_contexts().await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&a));
        assert!(ids.contains(&b));
    }

    #[test]
    fn test_tree_context_manager_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TreeContextManager>();
    }
}
