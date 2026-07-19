//! # Service Registry
//!
//! A central directory of every service running in the platform.
//! Components can discover each other by name without importing
//! concrete types.
//!
//! ## Design decisions
//!
//! * **Name-based lookup** — unlike the DI container
//!   (type-based), the registry maps string names to
//!   `Arc<dyn Service>` references.  This is useful for
//!   introspection, health checks, and management UIs.
//! * **Immutable metadata** — [`ServiceEntry`] carries version,
//!   description, and dependency list that are set once at
//!   registration time.
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use crate::error::CoreError;
use crate::lifecycle::Service;

// ── Service metadata ──────────────────────────────────────

/// Metadata registered alongside a service.
#[derive(Debug, Clone)]
pub struct ServiceEntry {
    pub name: String,
    pub version: String,
    pub description: String,
    pub dependencies: Vec<String>,
}

// ── Registry trait ────────────────────────────────────────

/// A directory of named services.
pub trait ServiceRegistry: Debug + Send + Sync {
    /// Register `service` with the given metadata.
    fn register(&self, service: Arc<dyn Service>, entry: ServiceEntry) -> Result<(), CoreError>;

    /// Look up a service by name.
    fn resolve(&self, name: &str) -> Result<Arc<dyn Service>, CoreError>;

    /// List every registered entry.
    fn list(&self) -> Vec<ServiceEntry>;

    /// Check whether a service exists.
    fn contains(&self, name: &str) -> bool;
}

// ── Implementation ────────────────────────────────────────

/// Default in-memory service registry.
#[derive(Debug, Default)]
pub struct DefaultServiceRegistry {
    services: RwLock<HashMap<String, ServiceRecord>>,
}

#[derive(Debug, Clone)]
struct ServiceRecord {
    entry: ServiceEntry,
    instance: Arc<dyn Service>,
}

impl DefaultServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
        }
    }
}

impl ServiceRegistry for DefaultServiceRegistry {
    fn register(&self, service: Arc<dyn Service>, entry: ServiceEntry) -> Result<(), CoreError> {
        let name = entry.name.clone();
        let mut guard = self.services.write().map_err(|_| CoreError::LockPoisoned)?;

        if guard.contains_key(&name) {
            return Err(CoreError::ServiceConflict { name });
        }

        guard.insert(
            name,
            ServiceRecord {
                entry,
                instance: service,
            },
        );
        Ok(())
    }

    fn resolve(&self, name: &str) -> Result<Arc<dyn Service>, CoreError> {
        let guard = self.services.read().map_err(|_| CoreError::LockPoisoned)?;
        guard
            .get(name)
            .map(|r| r.instance.clone())
            .ok_or_else(|| CoreError::ServiceNotFound {
                name: name.to_string(),
            })
    }

    fn list(&self) -> Vec<ServiceEntry> {
        self.services
            .read()
            .map(|g| g.values().map(|r| r.entry.clone()).collect())
            .unwrap_or_default()
    }

    fn contains(&self, name: &str) -> bool {
        let guard = self
            .services
            .read()
            .map(|g| g.contains_key(name))
            .unwrap_or(false);
        guard
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::Service;
    use async_trait::async_trait;

    #[derive(Debug)]
    struct MockService {
        name: String,
    }

    #[async_trait]
    impl Service for MockService {
        fn name(&self) -> &str {
            &self.name
        }
        async fn start(&self) -> Result<(), CoreError> {
            Ok(())
        }
        async fn stop(&self) -> Result<(), CoreError> {
            Ok(())
        }
    }

    fn make_entry(name: &str) -> ServiceEntry {
        ServiceEntry {
            name: name.to_string(),
            version: "0.1.0".into(),
            description: format!("{} service", name),
            dependencies: vec![],
        }
    }

    #[test]
    fn register_and_resolve() {
        let reg = DefaultServiceRegistry::new();
        let svc: Arc<dyn Service> = Arc::new(MockService {
            name: "http".into(),
        });
        reg.register(svc.clone(), make_entry("http")).unwrap();

        let resolved = reg.resolve("http").unwrap();
        assert_eq!(resolved.name(), "http");
    }

    #[test]
    fn duplicate_fails() {
        let reg = DefaultServiceRegistry::new();
        let svc: Arc<dyn Service> = Arc::new(MockService { name: "dup".into() });
        reg.register(svc.clone(), make_entry("dup")).unwrap();
        let result = reg.register(svc, make_entry("dup"));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_missing_fails() {
        let reg = DefaultServiceRegistry::new();
        let result = reg.resolve("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn list_returns_all() {
        let reg = DefaultServiceRegistry::new();
        reg.register(Arc::new(MockService { name: "a".into() }), make_entry("a"))
            .unwrap();
        reg.register(Arc::new(MockService { name: "b".into() }), make_entry("b"))
            .unwrap();

        let entries = reg.list();
        assert_eq!(entries.len(), 2);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn contains_works() {
        let reg = DefaultServiceRegistry::new();
        reg.register(Arc::new(MockService { name: "x".into() }), make_entry("x"))
            .unwrap();
        assert!(reg.contains("x"));
        assert!(!reg.contains("y"));
    }
}
