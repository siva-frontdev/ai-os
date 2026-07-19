//! # Dependency Injection Container
//!
//! A string-keyed, thread-safe container for sharing singleton
//! instances across the platform.
//!
//! ## Design decisions
//!
//! * **String keys** — unlike a `TypeId`-based approach,
//!   string keys make the trait **dyn-compatible** and allow
//!   cross-language or runtime-defined service names.
//! * **Arc-based** — every value is stored as
//!   `Arc<dyn Any + Send + Sync>`.  Callers downcast on
//!   retrieval.
//! * **Overwrite by default** — registering the same key twice
//!   overwrites the previous value.
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use crate::error::CoreError;

// ── Trait ─────────────────────────────────────────────────

pub trait Container: Debug + Send + Sync {
    /// Register an instance under `key`.
    fn register(&self, key: &str, instance: Arc<dyn Any + Send + Sync>) -> Result<(), CoreError>;

    /// Resolve an instance by `key`.  Returns `None` if not found.
    fn resolve(&self, key: &str) -> Option<Arc<dyn Any + Send + Sync>>;

    /// Check if a key exists.
    fn contains(&self, key: &str) -> bool;
}

// ── Implementation ────────────────────────────────────────

#[derive(Debug, Default)]
pub struct InMemoryContainer {
    services: RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>,
}

impl InMemoryContainer {
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
        }
    }

    /// Convenience: register a concrete `T`.
    pub fn register_typed<T: Send + Sync + 'static>(
        &self,
        key: &str,
        instance: T,
    ) -> Result<(), CoreError> {
        self.register(key, Arc::new(instance))
    }
}

impl Container for InMemoryContainer {
    fn register(&self, key: &str, instance: Arc<dyn Any + Send + Sync>) -> Result<(), CoreError> {
        let mut guard = self.services.write().map_err(|_| CoreError::LockPoisoned)?;
        guard.insert(key.to_string(), instance);
        Ok(())
    }

    fn resolve(&self, key: &str) -> Option<Arc<dyn Any + Send + Sync>> {
        let guard = self.services.read().ok()?;
        guard.get(key).cloned()
    }

    fn contains(&self, key: &str) -> bool {
        self.services
            .read()
            .map(|g| g.contains_key(key))
            .unwrap_or(false)
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_resolve() {
        let c = InMemoryContainer::new();
        c.register_typed("greeter", String::from("Hello")).unwrap();
        let val = c.resolve("greeter");
        assert!(val.is_some());
        let msg = val.unwrap().downcast::<String>().unwrap();
        assert_eq!(*msg, "Hello");
    }

    #[test]
    fn missing_key() {
        let c = InMemoryContainer::new();
        assert!(c.resolve("nope").is_none());
    }

    #[test]
    fn overwrite_existing() {
        let c = InMemoryContainer::new();
        c.register_typed("key", 1u32).unwrap();
        c.register_typed("key", 2u32).unwrap();
        let val = c.resolve("key").unwrap().downcast::<u32>().unwrap();
        assert_eq!(*val, 2);
    }

    #[test]
    fn contains_check() {
        let c = InMemoryContainer::new();
        assert!(!c.contains("x"));
        c.register_typed("x", 42i32).unwrap();
        assert!(c.contains("x"));
    }

    #[test]
    fn different_types() {
        let c = InMemoryContainer::new();
        c.register_typed("str", "hello").unwrap();
        c.register_typed("num", 42i32).unwrap();

        let s = c.resolve("str").unwrap().downcast::<&str>().unwrap();
        let n = c.resolve("num").unwrap().downcast::<i32>().unwrap();
        assert_eq!(*s, "hello");
        assert_eq!(*n, 42);
    }
}
