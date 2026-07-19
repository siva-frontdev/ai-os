use async_trait::async_trait;

use crate::error::WorkingMemoryResult;

/// Attention queue for managing what the AI should focus on.
///
/// The attention manager maintains a priority queue of items
/// that the AI should attend to. Items are scored by urgency
/// and importance. Higher priority values indicate more urgent items.
#[async_trait]
pub trait AttentionManager: Send + Sync + std::fmt::Debug {
    /// Enqueue an item with a priority score (higher = more urgent).
    async fn enqueue(&self, item: String, priority: u8) -> WorkingMemoryResult<()>;

    /// Dequeue the highest-priority item.
    async fn dequeue(&self) -> WorkingMemoryResult<Option<String>>;

    /// Peek at the highest-priority item without removing it.
    async fn peek(&self) -> WorkingMemoryResult<Option<(String, u8)>>;

    /// Remove a specific item from the queue.
    async fn remove(&self, item: &str) -> WorkingMemoryResult<()>;

    /// Return the current queue length.
    async fn len(&self) -> WorkingMemoryResult<usize>;

    /// Return true if the queue is empty.
    async fn is_empty(&self) -> WorkingMemoryResult<bool> {
        self.len().await.map(|l| l == 0)
    }

    /// Return all items in priority order (highest first).
    async fn list(&self) -> WorkingMemoryResult<Vec<(String, u8)>>;

    /// Clear the queue.
    async fn clear(&self) -> WorkingMemoryResult<()>;
}

use std::sync::RwLock;

/// A priority-sorted attention manager backed by a `Vec<(String, u8)>`.
///
/// Items are maintained in sorted order (highest priority first).
/// When priorities are equal, items are ordered by insertion time (FIFO).
/// Thread-safe via `RwLock`.
#[derive(Debug)]
pub struct PriorityAttentionManager {
    items: RwLock<Vec<(String, u8)>>,
}

impl Default for PriorityAttentionManager {
    fn default() -> Self {
        Self {
            items: RwLock::new(Vec::new()),
        }
    }
}

impl PriorityAttentionManager {
    /// Create a new empty attention manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an item into the sorted position (highest priority first).
    fn insert_sorted(items: &mut Vec<(String, u8)>, item: String, priority: u8) {
        let pos = items
            .iter()
            .position(|(_, p)| *p < priority)
            .unwrap_or(items.len());
        items.insert(pos, (item, priority));
    }
}

#[async_trait]
impl AttentionManager for PriorityAttentionManager {
    async fn enqueue(&self, item: String, priority: u8) -> WorkingMemoryResult<()> {
        let mut items = self
            .items
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        Self::insert_sorted(&mut items, item, priority);
        Ok(())
    }

    async fn dequeue(&self) -> WorkingMemoryResult<Option<String>> {
        let mut items = self
            .items
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(items.remove(0).0))
    }

    async fn peek(&self) -> WorkingMemoryResult<Option<(String, u8)>> {
        let items = self
            .items
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        Ok(items.first().cloned())
    }

    async fn remove(&self, item: &str) -> WorkingMemoryResult<()> {
        let mut items = self
            .items
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        items.retain(|(name, _)| name != item);
        Ok(())
    }

    async fn len(&self) -> WorkingMemoryResult<usize> {
        let items = self
            .items
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        Ok(items.len())
    }

    async fn list(&self) -> WorkingMemoryResult<Vec<(String, u8)>> {
        let items = self
            .items
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        Ok(items.clone())
    }

    async fn clear(&self) -> WorkingMemoryResult<()> {
        let mut items = self
            .items
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        items.clear();
        Ok(())
    }
}

/// A no-op implementation of [`AttentionManager`] that returns errors on all operations.
///
/// Useful as a default or placeholder when attention management is not yet configured.
#[derive(Debug)]
pub struct DefaultAttentionManager;

#[async_trait]
impl AttentionManager for DefaultAttentionManager {
    async fn enqueue(&self, _item: String, _priority: u8) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultAttentionManager: not configured".into(),
        ))
    }

    async fn dequeue(&self) -> WorkingMemoryResult<Option<String>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultAttentionManager: not configured".into(),
        ))
    }

    async fn peek(&self) -> WorkingMemoryResult<Option<(String, u8)>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultAttentionManager: not configured".into(),
        ))
    }

    async fn remove(&self, _item: &str) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultAttentionManager: not configured".into(),
        ))
    }

    async fn len(&self) -> WorkingMemoryResult<usize> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultAttentionManager: not configured".into(),
        ))
    }

    async fn list(&self) -> WorkingMemoryResult<Vec<(String, u8)>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultAttentionManager: not configured".into(),
        ))
    }

    async fn clear(&self) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultAttentionManager: not configured".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enqueue_and_dequeue() {
        let am = PriorityAttentionManager::new();
        am.enqueue("low".into(), 10).await.unwrap();
        am.enqueue("high".into(), 100).await.unwrap();
        am.enqueue("medium".into(), 50).await.unwrap();

        assert_eq!(am.dequeue().await.unwrap(), Some("high".into()));
        assert_eq!(am.dequeue().await.unwrap(), Some("medium".into()));
        assert_eq!(am.dequeue().await.unwrap(), Some("low".into()));
        assert_eq!(am.dequeue().await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_peek() {
        let am = PriorityAttentionManager::new();
        assert_eq!(am.peek().await.unwrap(), None);

        am.enqueue("task".into(), 50).await.unwrap();
        let peeked = am.peek().await.unwrap();
        assert_eq!(peeked, Some(("task".into(), 50)));

        // Peek should not remove the item
        assert_eq!(am.len().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_remove() {
        let am = PriorityAttentionManager::new();
        am.enqueue("a".into(), 10).await.unwrap();
        am.enqueue("b".into(), 20).await.unwrap();
        am.enqueue("c".into(), 30).await.unwrap();

        am.remove("b").await.unwrap();
        let items = am.list().await.unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|(name, _)| name != "b"));
    }

    #[tokio::test]
    async fn test_len() {
        let am = PriorityAttentionManager::new();
        assert_eq!(am.len().await.unwrap(), 0);
        am.enqueue("item".into(), 1).await.unwrap();
        assert_eq!(am.len().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_list_order() {
        let am = PriorityAttentionManager::new();
        am.enqueue("low".into(), 10).await.unwrap();
        am.enqueue("high".into(), 100).await.unwrap();
        am.enqueue("medium".into(), 50).await.unwrap();

        let items = am.list().await.unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].0, "high");
        assert_eq!(items[1].0, "medium");
        assert_eq!(items[2].0, "low");
    }

    #[tokio::test]
    async fn test_clear() {
        let am = PriorityAttentionManager::new();
        am.enqueue("a".into(), 1).await.unwrap();
        am.enqueue("b".into(), 2).await.unwrap();
        am.clear().await.unwrap();
        assert_eq!(am.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_dequeue_from_empty() {
        let am = PriorityAttentionManager::new();
        assert_eq!(am.dequeue().await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_fifo_for_equal_priority() {
        let am = PriorityAttentionManager::new();
        am.enqueue("first".into(), 50).await.unwrap();
        am.enqueue("second".into(), 50).await.unwrap();
        am.enqueue("third".into(), 50).await.unwrap();

        // With same priority, items should maintain insertion order
        let items = am.list().await.unwrap();
        assert_eq!(items[0].0, "first");
        assert_eq!(items[1].0, "second");
        assert_eq!(items[2].0, "third");
    }

    #[test]
    fn test_priority_attention_manager_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<PriorityAttentionManager>();
        assert_sync::<PriorityAttentionManager>();
    }
}
