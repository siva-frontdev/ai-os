use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use crate::error::{SemanticError, SemanticResult};

/// A typed relationship between two concepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptRelation {
    pub id: Uuid,
    pub subject: Uuid,
    pub relation: String,
    pub object: Uuid,
    pub confidence: f32,
    pub timestamp: i64,
}

/// Trait for concept relationship storage and query.
#[async_trait]
pub trait RelationshipStore: Send + Sync + std::fmt::Debug {
    async fn insert(&self, rel: ConceptRelation) -> SemanticResult<()>;
    async fn get(&self, id: &Uuid) -> SemanticResult<Option<ConceptRelation>>;
    async fn list_by_subject(&self, subject: &Uuid) -> SemanticResult<Vec<ConceptRelation>>;
    async fn list_by_predicate(&self, pred: &str) -> SemanticResult<Vec<ConceptRelation>>;
    async fn count(&self) -> SemanticResult<u64>;
}

#[derive(Debug, Default)]
pub struct InMemoryRelationshipStore {
    inner: RwLock<HashMap<Uuid, ConceptRelation>>,
}

impl InMemoryRelationshipStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RelationshipStore for InMemoryRelationshipStore {
    async fn insert(&self, rel: ConceptRelation) -> SemanticResult<()> {
        self.inner
            .write()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .insert(rel.id, rel);
        Ok(())
    }
    async fn get(&self, id: &Uuid) -> SemanticResult<Option<ConceptRelation>> {
        Ok(self.inner
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .get(id)
            .cloned())
    }
    async fn list_by_subject(&self, subject: &Uuid) -> SemanticResult<Vec<ConceptRelation>> {
        Ok(self.inner
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .values()
            .filter(|r| r.subject == *subject)
            .cloned()
            .collect())
    }
    async fn list_by_predicate(&self, pred: &str) -> SemanticResult<Vec<ConceptRelation>> {
        Ok(self.inner
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .values()
            .filter(|r| r.relation == pred)
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
pub struct DefaultRelationshipStore;

#[async_trait]
impl RelationshipStore for DefaultRelationshipStore {
    async fn insert(&self, _: ConceptRelation) -> SemanticResult<()> { Ok(()) }
    async fn get(&self, _: &Uuid) -> SemanticResult<Option<ConceptRelation>> { Ok(None) }
    async fn list_by_subject(&self, _: &Uuid) -> SemanticResult<Vec<ConceptRelation>> { Ok(Vec::new()) }
    async fn list_by_predicate(&self, _: &str) -> SemanticResult<Vec<ConceptRelation>> { Ok(Vec::new()) }
    async fn count(&self) -> SemanticResult<u64> { Ok(0) }
}
