use std::collections::HashMap;
use std::sync::RwLock;
use async_trait::async_trait;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

use crate::error::{SemanticError, SemanticResult};

/// Data model for a concept in semantic memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: Uuid,
    pub name: String,
    pub category: String,
    pub timestamp: i64,
    pub version: u64,
}

/// Trait for concept storage.
#[async_trait]
pub trait ConceptStore: Send + Sync + std::fmt::Debug {
    async fn insert(&self, concept: Concept) -> SemanticResult<()>;
    async fn get(&self, id: &Uuid) -> SemanticResult<Option<Concept>>;
    async fn update(&self, concept: Concept) -> SemanticResult<()>;
    async fn delete(&self, id: &Uuid) -> SemanticResult<()>;
    async fn list(&self) -> SemanticResult<Vec<Concept>>;
    async fn count(&self) -> SemanticResult<u64>;
}

#[derive(Debug, Default)]
pub struct InMemoryConceptStore {
    inner: RwLock<HashMap<Uuid, Concept>>,
}

impl InMemoryConceptStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ConceptStore for InMemoryConceptStore {
    async fn insert(&self, concept: Concept) -> SemanticResult<()> {
        self.inner
            .write()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .insert(concept.id, concept);
        Ok(())
    }
    async fn get(&self, id: &Uuid) -> SemanticResult<Option<Concept>> {
        Ok(self.inner
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .get(id)
            .cloned())
    }
    async fn update(&self, concept: Concept) -> SemanticResult<()> {
        self.inner
            .write()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .insert(concept.id, concept);
        Ok(())
    }
    async fn delete(&self, id: &Uuid) -> SemanticResult<()> {
        self.inner
            .write()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .remove(id);
        Ok(())
    }
    async fn list(&self) -> SemanticResult<Vec<Concept>> {
        Ok(self.inner
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .values()
            .cloned()
            .collect())
    }
    async fn count(&self) -> SemanticResult<u64> {
        Ok(self.inner
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .len() as u64)
    }
}

/// No-op default implementation.
#[derive(Debug, Default)]
pub struct DefaultConceptStore;

#[async_trait]
impl ConceptStore for DefaultConceptStore {
    async fn insert(&self, _: Concept) -> SemanticResult<()> { Ok(()) }
    async fn get(&self, _: &Uuid) -> SemanticResult<Option<Concept>> { Ok(None) }
    async fn update(&self, _: Concept) -> SemanticResult<()> { Ok(()) }
    async fn delete(&self, _: &Uuid) -> SemanticResult<()> { Ok(()) }
    async fn list(&self) -> SemanticResult<Vec<Concept>> { Ok(Vec::new()) }
    async fn count(&self) -> SemanticResult<u64> { Ok(0) }
}
