//! `ModelProviderRegistry` — routes model calls to registered providers.
use std::fmt::Debug;
use std::sync::{Arc, RwLock};
use super::*;

/// Registry for resolving model providers by name.
#[async_trait::async_trait]
pub trait ModelProviderRegistry: Debug + Send + Sync {
    fn resolve(&self, provider_name: Option<&str>) -> Option<Arc<dyn ModelProvider>>;
    fn register_provider(&self, provider: Arc<dyn ModelProvider>);
    fn healthy_providers(&self) -> Vec<Arc<dyn ModelProvider>>;
}

/// A simple in-memory registry for model providers.
#[derive(Debug)]
pub struct InMemoryModelProviderRegistry {
    providers: Arc<RwLock<Vec<Arc<dyn ModelProvider>>>>,
    default_provider: Arc<RwLock<Option<String>>>,
}

impl InMemoryModelProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a provider and return its ID.
    pub fn register(&self, provider: Arc<dyn ModelProvider>) -> ModelId {
        self.providers.write().unwrap().push(provider);
        // Generate a deterministic ID from name (for lookup by name)
        // In real impl, use HashMap<String, ModelId>
        ModelId::new()
    }

    /// Find a provider by name.
    pub fn find_by_name(&self, name: &str) -> Option<Arc<dyn ModelProvider>> {
        self.providers.read().unwrap().iter().find(|p| p.name() == name).cloned()
    }

    /// List all registered provider names.
    pub fn list_providers(&self) -> Vec<String> {
        self.providers.read().unwrap().iter().map(|p| p.name().to_string()).collect()
    }

    /// Set the default provider name.
    pub fn set_default(&self, name: impl Into<String>) {
        *self.default_provider.write().unwrap() = Some(name.into());
    }
}

impl ModelProviderRegistry for InMemoryModelProviderRegistry {
    fn resolve(&self, provider_name: Option<&str>) -> Option<Arc<dyn ModelProvider>> {
        let name = provider_name.map(String::from).or_else(|| self.default_provider.read().unwrap().clone());
        name.as_deref().and_then(|n| self.find_by_name(n))
    }

    fn register_provider(&self, provider: Arc<dyn ModelProvider>) {
        self.register(provider);
    }

    fn healthy_providers(&self) -> Vec<Arc<dyn ModelProvider>> {
        self.providers
            .read()
            .unwrap()
            .iter()
            .filter(|p| p.status() == ProviderStatus::Healthy)
            .cloned()
            .collect()
    }
}

impl Default for InMemoryModelProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
