//! # Configuration
//!
//! Abstraction over configuration sources.
//!
//! ## Design decisions
//!
//! * **Value-based** — the [`ConfigProvider`] trait works with
//!   `serde_json::Value` instead of generics so that it remains
//!   **dyn-compatible** (object-safe).  Callers who want typed
//!   access use the [`ConfigExt`] extension trait or the
//!   convenience methods on concrete providers.
//! * **Layered** — [`LayeredConfigProvider`] merges multiple
//!   sources; the first source that contains a key wins.
use std::collections::HashMap;
use std::fmt::Debug;

use serde::de::DeserializeOwned;

use crate::error::CoreError;

// ── Trait ─────────────────────────────────────────────────

/// Read-only configuration source (dyn-compatible).
pub trait ConfigProvider: Debug + Send + Sync {
    /// Retrieve a raw JSON value for `key`.
    fn get_raw(&self, key: &str) -> Result<Option<serde_json::Value>, CoreError>;

    /// Set a raw JSON value at `key`.
    fn set_raw(&self, key: &str, value: serde_json::Value) -> Result<(), CoreError>;

    /// List all keys known to this provider.
    fn keys(&self) -> Vec<String>;
}

// ── Extension trait for typed access ──────────────────────

/// Typed convenience methods for [`ConfigProvider`].
///
/// Automatically implemented for every `&T` where `T:
/// ConfigProvider`.
pub trait ConfigExt {
    fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, CoreError>;
    fn get_or<T: DeserializeOwned>(&self, key: &str, default: T) -> Result<T, CoreError>;
}

impl<T: ConfigProvider + ?Sized> ConfigExt for T {
    fn get<Ty: DeserializeOwned>(&self, key: &str) -> Result<Option<Ty>, CoreError> {
        match self.get_raw(key)? {
            None => Ok(None),
            Some(raw) => {
                let val: Ty = serde_json::from_value(raw).map_err(|e| {
                    CoreError::ConfigParse {
                        key: key.to_string(),
                        detail: e.to_string(),
                    }
                })?;
                Ok(Some(val))
            }
        }
    }

    fn get_or<Ty: DeserializeOwned>(&self, key: &str, default: Ty) -> Result<Ty, CoreError> {
        Ok(self.get(key)?.unwrap_or(default))
    }
}

// ── Implementations ───────────────────────────────────────

/// In-memory configuration backed by a `HashMap`.
#[derive(Debug, Clone)]
pub struct InMemoryConfigProvider {
    data: HashMap<String, serde_json::Value>,
}

impl InMemoryConfigProvider {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn from_json(value: serde_json::Value) -> Self {
        let data = match value {
            serde_json::Value::Object(map) => map.into_iter().collect(),
            other => {
                let mut map = HashMap::new();
                map.insert("".to_string(), other);
                map
            }
        };
        Self { data }
    }

    pub fn from_str(json: &str) -> Result<Self, CoreError> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| CoreError::ConfigParse {
                key: "<root>".into(),
                detail: e.to_string(),
            })?;
        Ok(Self::from_json(value))
    }

    /// Builder-style insert (works on the concrete type, not the trait).
    pub fn with<K: Into<String>>(mut self, key: K, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }
}

impl Default for InMemoryConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigProvider for InMemoryConfigProvider {
    fn get_raw(&self, key: &str) -> Result<Option<serde_json::Value>, CoreError> {
        Ok(self.data.get(key).cloned())
    }

    fn set_raw(&self, _key: &str, _value: serde_json::Value) -> Result<(), CoreError> {
        Err(CoreError::General(
            "InMemoryConfigProvider is immutable via the trait; use ThreadsafeConfigProvider".into(),
        ))
    }

    fn keys(&self) -> Vec<String> {
        let mut k: Vec<String> = self.data.keys().cloned().collect();
        k.sort();
        k
    }
}

/// Thread-safe wrapper using `RwLock`.
#[derive(Debug)]
pub struct ThreadsafeConfigProvider {
    inner: std::sync::RwLock<InMemoryConfigProvider>,
}

impl ThreadsafeConfigProvider {
    pub fn new(inner: InMemoryConfigProvider) -> Self {
        Self {
            inner: std::sync::RwLock::new(inner),
        }
    }
}

impl ConfigProvider for ThreadsafeConfigProvider {
    fn get_raw(&self, key: &str) -> Result<Option<serde_json::Value>, CoreError> {
        let guard = self.inner.read().map_err(|_| CoreError::LockPoisoned)?;
        guard.get_raw(key)
    }

    fn set_raw(&self, key: &str, value: serde_json::Value) -> Result<(), CoreError> {
        let mut guard = self.inner.write().map_err(|_| CoreError::LockPoisoned)?;
        guard.data.insert(key.to_string(), value);
        Ok(())
    }

    fn keys(&self) -> Vec<String> {
        self.inner
            .read()
            .map(|g| g.keys())
            .unwrap_or_default()
    }
}

/// Layered config — first source with the key wins.
#[derive(Debug)]
pub struct LayeredConfigProvider {
    layers: Vec<Box<dyn ConfigProvider>>,
}

impl LayeredConfigProvider {
    pub fn new() -> Self {
        Self { layers: vec![] }
    }

    pub fn push(mut self, provider: Box<dyn ConfigProvider>) -> Self {
        self.layers.push(provider);
        self
    }
}

impl Default for LayeredConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigProvider for LayeredConfigProvider {
    fn get_raw(&self, key: &str) -> Result<Option<serde_json::Value>, CoreError> {
        for layer in &self.layers {
            if let Some(val) = layer.get_raw(key)? {
                return Ok(Some(val));
            }
        }
        Ok(None)
    }

    fn set_raw(&self, _key: &str, _value: serde_json::Value) -> Result<(), CoreError> {
        Err(CoreError::General("LayeredConfigProvider is read-only".into()))
    }

    fn keys(&self) -> Vec<String> {
        let mut all = Vec::new();
        for layer in &self.layers {
            all.extend(layer.keys());
        }
        all.sort();
        all.dedup();
        all
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_get_raw() {
        let provider = InMemoryConfigProvider::from_str(r#"{"host": "localhost", "port": 8080}"#).unwrap();
        assert_eq!(provider.get_raw("host").unwrap(), Some(serde_json::json!("localhost")));
    }

    #[test]
    fn typed_access_via_extension() {
        let provider = InMemoryConfigProvider::from_str(r#"{"host": "localhost", "port": 8080}"#).unwrap();
        let host: Option<String> = provider.get("host").unwrap();
        assert_eq!(host, Some("localhost".into()));
        let port: Option<u16> = provider.get("port").unwrap();
        assert_eq!(port, Some(8080));
    }

    #[test]
    fn missing_key() {
        let provider = InMemoryConfigProvider::new();
        assert!(provider.get_raw("missing").unwrap().is_none());
    }

    #[test]
    fn get_or_default() {
        let provider = InMemoryConfigProvider::new();
        let val: u32 = provider.get_or("missing", 42).unwrap();
        assert_eq!(val, 42);
    }

    #[test]
    fn layered_first_wins() {
        let layer1 = InMemoryConfigProvider::from_str(r#"{"key": "from_layer1"}"#).unwrap();
        let layer2 = InMemoryConfigProvider::from_str(r#"{"key": "from_layer2"}"#).unwrap();
        let layered = LayeredConfigProvider::new()
            .push(Box::new(layer1))
            .push(Box::new(layer2));
        assert_eq!(
            layered.get_raw("key").unwrap(),
            Some(serde_json::json!("from_layer1"))
        );
    }

    #[test]
    fn layered_keys_dedup() {
        let layer1 = InMemoryConfigProvider::from_str(r#"{"a": 1, "b": 2}"#).unwrap();
        let layer2 = InMemoryConfigProvider::from_str(r#"{"b": 3, "c": 4}"#).unwrap();
        let layered = LayeredConfigProvider::new()
            .push(Box::new(layer1))
            .push(Box::new(layer2));
        assert_eq!(layered.keys(), vec!["a", "b", "c"]);
    }

    #[test]
    fn threadsafe_set_get() {
        let inner = InMemoryConfigProvider::new();
        let provider = ThreadsafeConfigProvider::new(inner);
        provider.set_raw("key", serde_json::json!("val")).unwrap();
        assert_eq!(
            provider.get_raw("key").unwrap(),
            Some(serde_json::json!("val"))
        );
    }
}
