use async_trait::async_trait;

use crate::error::WorkingMemoryResult;

/// Temporary reasoning state storage.
///
/// The scratchpad holds intermediate computation results,
/// partial reasoning chains, and temporary data that does not
/// need to survive beyond the current reasoning step.
/// Entries are key-value pairs with byte-serialized values.
#[async_trait]
pub trait Scratchpad: Send + Sync + std::fmt::Debug {
    /// Write a value to the scratchpad.
    async fn set(&self, key: &str, value: Vec<u8>) -> WorkingMemoryResult<()>;

    /// Read a value from the scratchpad.
    async fn get(&self, key: &str) -> WorkingMemoryResult<Option<Vec<u8>>>;

    /// Remove a value.
    async fn remove(&self, key: &str) -> WorkingMemoryResult<()>;

    /// Clear the entire scratchpad.
    async fn clear(&self) -> WorkingMemoryResult<()>;

    /// Return the number of entries.
    async fn len(&self) -> WorkingMemoryResult<usize>;

    /// Return true if the scratchpad is empty.
    async fn is_empty(&self) -> WorkingMemoryResult<bool> {
        self.len().await.map(|l| l == 0)
    }

    /// Return all keys.
    async fn keys(&self) -> WorkingMemoryResult<Vec<String>>;
}

use std::collections::HashMap;
use std::sync::RwLock;

/// In-memory scratchpad backed by a `HashMap<String, Vec<u8>>`.
///
/// Thread-safe via `RwLock`. All operations are synchronous and
/// operate on the current process memory. No persistence.
#[derive(Debug)]
pub struct InMemoryScratchpad {
    data: RwLock<HashMap<String, Vec<u8>>>,
}

impl Default for InMemoryScratchpad {
    fn default() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl InMemoryScratchpad {
    /// Create a new empty scratchpad.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Scratchpad for InMemoryScratchpad {
    async fn set(&self, key: &str, value: Vec<u8>) -> WorkingMemoryResult<()> {
        let mut data = self
            .data
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        data.insert(key.to_string(), value);
        Ok(())
    }

    async fn get(&self, key: &str) -> WorkingMemoryResult<Option<Vec<u8>>> {
        let data = self
            .data
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        Ok(data.get(key).cloned())
    }

    async fn remove(&self, key: &str) -> WorkingMemoryResult<()> {
        let mut data = self
            .data
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        data.remove(key);
        Ok(())
    }

    async fn clear(&self) -> WorkingMemoryResult<()> {
        let mut data = self
            .data
            .write()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        data.clear();
        Ok(())
    }

    async fn len(&self) -> WorkingMemoryResult<usize> {
        let data = self
            .data
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        Ok(data.len())
    }

    async fn keys(&self) -> WorkingMemoryResult<Vec<String>> {
        let data = self
            .data
            .read()
            .map_err(|e| crate::WorkingMemoryError::Internal(format!("lock poisoned: {e}")))?;
        Ok(data.keys().cloned().collect())
    }
}

/// A no-op implementation of [`Scratchpad`] that returns errors on all operations.
///
/// Useful as a default or placeholder when a scratchpad is not yet configured.
#[derive(Debug)]
pub struct DefaultScratchpad;

#[async_trait]
impl Scratchpad for DefaultScratchpad {
    async fn set(&self, _key: &str, _value: Vec<u8>) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultScratchpad: not configured".into(),
        ))
    }

    async fn get(&self, _key: &str) -> WorkingMemoryResult<Option<Vec<u8>>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultScratchpad: not configured".into(),
        ))
    }

    async fn remove(&self, _key: &str) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultScratchpad: not configured".into(),
        ))
    }

    async fn clear(&self) -> WorkingMemoryResult<()> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultScratchpad: not configured".into(),
        ))
    }

    async fn len(&self) -> WorkingMemoryResult<usize> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultScratchpad: not configured".into(),
        ))
    }

    async fn keys(&self) -> WorkingMemoryResult<Vec<String>> {
        Err(crate::WorkingMemoryError::Internal(
            "DefaultScratchpad: not configured".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_and_get() {
        let pad = InMemoryScratchpad::new();
        pad.set("key1", b"value1".to_vec()).await.unwrap();
        let val = pad.get("key1").await.unwrap();
        assert_eq!(val, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let pad = InMemoryScratchpad::new();
        let val = pad.get("nonexistent").await.unwrap();
        assert_eq!(val, None);
    }

    #[tokio::test]
    async fn test_remove() {
        let pad = InMemoryScratchpad::new();
        pad.set("key1", b"value1".to_vec()).await.unwrap();
        pad.remove("key1").await.unwrap();
        assert_eq!(pad.get("key1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_clear() {
        let pad = InMemoryScratchpad::new();
        pad.set("a", vec![1]).await.unwrap();
        pad.set("b", vec![2]).await.unwrap();
        pad.clear().await.unwrap();
        assert_eq!(pad.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_len() {
        let pad = InMemoryScratchpad::new();
        assert_eq!(pad.len().await.unwrap(), 0);
        pad.set("a", vec![1]).await.unwrap();
        assert_eq!(pad.len().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_keys() {
        let pad = InMemoryScratchpad::new();
        pad.set("a", vec![1]).await.unwrap();
        pad.set("b", vec![2]).await.unwrap();
        let mut keys = pad.keys().await.unwrap();
        keys.sort();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn test_overwrite() {
        let pad = InMemoryScratchpad::new();
        pad.set("key", b"old".to_vec()).await.unwrap();
        pad.set("key", b"new".to_vec()).await.unwrap();
        let val = pad.get("key").await.unwrap();
        assert_eq!(val, Some(b"new".to_vec()));
    }
}
