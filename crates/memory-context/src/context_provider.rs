use crate::error::{ContextManagerError, ContextManagerResult};
use async_trait::async_trait;
use std::collections::HashMap;

/// Provides context values from external sources (runtime, OSAL, env vars, config).
///
/// Implementations of this trait supply values that are injected into
/// contexts at creation or refresh time. Examples:
/// - `RuntimeContextProvider` pulls values from the Runtime subsystem.
/// - `EnvironmentProvider` reads process environment variables.
/// - `StaticContextProvider` returns pre-configured values.
#[async_trait]
pub trait ContextProvider: Send + Sync + std::fmt::Debug {
    /// Provide context values for the given keys. Returns only keys
    /// that this provider can supply.
    async fn provide_context(
        &self,
        keys: &[String],
    ) -> ContextManagerResult<HashMap<String, Vec<u8>>>;

    /// Return the list of keys this provider can supply.
    async fn available_keys(&self) -> ContextManagerResult<Vec<String>>;

    /// Refresh internal state (e.g., re-read environment vars).
    async fn refresh(&self) -> ContextManagerResult<()>;
}

/// A context provider backed by a static key-value map.
#[derive(Debug, Clone, Default)]
pub struct StaticContextProvider {
    values: HashMap<String, Vec<u8>>,
}

impl StaticContextProvider {
    /// Create a new empty provider.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a provider prepopulated with key-value pairs.
    pub fn from_map(values: HashMap<String, Vec<u8>>) -> Self {
        Self { values }
    }

    /// Add or update a value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Vec<u8>>) {
        self.values.insert(key.into(), value.into());
    }
}

#[async_trait]
impl ContextProvider for StaticContextProvider {
    async fn provide_context(
        &self,
        keys: &[String],
    ) -> ContextManagerResult<HashMap<String, Vec<u8>>> {
        let mut result = HashMap::new();
        for k in keys {
            if let Some(v) = self.values.get(k) {
                result.insert(k.clone(), v.clone());
            }
        }
        Ok(result)
    }

    async fn available_keys(&self) -> ContextManagerResult<Vec<String>> {
        Ok(self.values.keys().cloned().collect())
    }

    async fn refresh(&self) -> ContextManagerResult<()> {
        Ok(())
    }
}

/// No-op default implementation. All methods return errors.
#[derive(Debug, Default)]
pub struct DefaultContextProvider;

#[async_trait]
impl ContextProvider for DefaultContextProvider {
    async fn provide_context(
        &self,
        _keys: &[String],
    ) -> ContextManagerResult<HashMap<String, Vec<u8>>> {
        Ok(HashMap::new())
    }

    async fn available_keys(&self) -> ContextManagerResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn refresh(&self) -> ContextManagerResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_static_provider_returns_values() {
        let mut p = StaticContextProvider::from_map(
            vec![("hostname".into(), b"ai-os".to_vec())]
                .into_iter()
                .collect(),
        );
        let result = p
            .provide_context(&["hostname".into(), "missing".into()])
            .await
            .unwrap();
        assert_eq!(result.get("hostname").unwrap(), b"ai-os");
        assert!(!result.contains_key("missing"));
    }

    #[tokio::test]
    async fn test_available_keys() {
        let mut p = StaticContextProvider::new();
        p.set("a", b"1");
        p.set("b", b"2");
        let keys = p.available_keys().await.unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[tokio::test]
    async fn test_default_provider() {
        let p = DefaultContextProvider;
        let keys = p.available_keys().await.unwrap();
        assert!(keys.is_empty());
    }
}
