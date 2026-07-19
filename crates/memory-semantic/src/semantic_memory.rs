use std::collections::HashMap;
use std::sync::RwLock;

use uuid::Uuid;
use async_trait::async_trait;
use memory_core::Timestamp;

use crate::error::{SemanticError, SemanticResult};
use crate::concept_store::{Concept, ConceptStore, InMemoryConceptStore};
use crate::relationship_store::{RelationshipStore, InMemoryRelationshipStore};
use crate::ontology::OntologyProvider;
use crate::event::{ConceptCreated, ConceptUpdated, FactCreated, FactUpdated};

/// A verified fact about the world.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Fact {
    pub id: Uuid,
    pub subject: Uuid,
    pub predicate: String,
    pub object: String,
    pub confidence: f32,
    pub timestamp: i64,
    pub support_count: u64,
}

impl Fact {
    pub fn new(
        id: Uuid, subject: Uuid, predicate: impl Into<String>,
        object: impl Into<String>, confidence: f32, timestamp: i64
    ) -> Self {
        Self { id, subject, predicate: predicate.into(), object: object.into(), confidence, timestamp, support_count: 1 }
    }
}

/// Aggregate statistics for the semantic store.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SemanticStats {
    pub concepts: u64,
    pub facts: u64,
    pub relationships: u64,
}

/// Trait describing the primary interface for semantic memory.
#[async_trait::async_trait]
pub trait SemanticMemory: Send + Sync + std::fmt::Debug {
    /// Create a new concept.
    async fn create_concept(&self, name: String, category: String) -> SemanticResult<Concept>;
    /// Retrieve a concept by ID.
    async fn get_concept(&self, id: &Uuid) -> SemanticResult<Option<Concept>>;
    /// Update an existing concept's mutable fields.
    async fn update_concept(&self, concept: Concept) -> SemanticResult<()>;
    /// Delete a concept by ID.
    async fn delete_concept(&self, id: &Uuid) -> SemanticResult<()>;
    /// List all concepts.
    async fn list_concepts(&self) -> SemanticResult<Vec<Concept>>;
    /// Emit a ConceptCreated event.
    async fn emit_concept_created(&self, concept: &Concept) -> SemanticResult<()>;
    /// Emit a FactCreated event.
    async fn emit_fact_created(&self, fact: &Fact) -> SemanticResult<()>;
    /// Store a new fact.
    async fn add_fact(&self, fact: Fact) -> SemanticResult<()>;
    /// Retrieve a fact by ID.
    async fn get_fact(&self, id: &Uuid) -> SemanticResult<Option<Fact>>;
    /// List all facts for a subject concept.
    async fn facts_for_subject(&self, subject: &Uuid) -> SemanticResult<Vec<Fact>>;
    /// Retrieve aggregate statistics.
    async fn stats(&self) -> SemanticResult<SemanticStats>;
}

/// In-memory implementation of SemanticMemory.
#[derive(Debug, Default)]
pub struct InMemorySemanticMemory {
    concepts: InMemoryConceptStore,
    relationships: InMemoryRelationshipStore,
    ontology: crate::ontology::DefaultOntologyProvider,
    facts: RwLock<HashMap<Uuid, Fact>>,
}

impl InMemorySemanticMemory {
    pub fn new() -> Self {
        Self {
            concepts: InMemoryConceptStore::new(),
            relationships: InMemoryRelationshipStore::new(),
            ontology: crate::ontology::DefaultOntologyProvider::new(),
            facts: RwLock::new(HashMap::new()),
        }
    }
    pub fn concepts(&self) -> &InMemoryConceptStore { &self.concepts }
    pub fn relationships(&self) -> &InMemoryRelationshipStore { &self.relationships }
    pub fn ontology(&self) -> &crate::ontology::DefaultOntologyProvider { &self.ontology }
}

#[async_trait::async_trait]
impl SemanticMemory for InMemorySemanticMemory {
    async fn create_concept(&self, name: String, category: String) -> SemanticResult<Concept> {
        let concept = Concept {
            id: Uuid::now_v7(),
            name: name.clone(),
            category: category.clone(),
            timestamp: Timestamp::now().as_nanos(),
            version: 1,
        };
        self.concepts.insert(concept.clone()).await?;
        self.emit_concept_created(&concept).await?;
        Ok(concept)
    }
    async fn get_concept(&self, id: &Uuid) -> SemanticResult<Option<Concept>> {
        self.concepts.get(id).await
    }
    async fn update_concept(&self, concept: Concept) -> SemanticResult<()> {
        self.concepts.update(concept.clone()).await?;
        self.emit_concept_created(&concept).await?;
        Ok(())
    }
    async fn delete_concept(&self, id: &Uuid) -> SemanticResult<()> {
        self.concepts.delete(id).await
    }
    async fn list_concepts(&self) -> SemanticResult<Vec<Concept>> {
        self.concepts.list().await
    }
    async fn emit_concept_created(&self, concept: &Concept) -> SemanticResult<()> {
        let _evt = ConceptCreated {
            id: concept.id,
            name: concept.name.clone(),
            category: concept.category.clone(),
            timestamp: concept.timestamp,
        };
        Ok(())
    }
    async fn emit_fact_created(&self, fact: &Fact) -> SemanticResult<()> {
        let _evt = FactCreated {
            id: fact.id,
            subject: fact.subject,
            predicate: fact.predicate.clone(),
            object: fact.object.clone(),
            confidence: fact.confidence,
            timestamp: fact.timestamp,
        };
        Ok(())
    }
    async fn add_fact(&self, fact: Fact) -> SemanticResult<()> {
        self.facts
            .write()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .insert(fact.id, fact.clone());
        self.relationships
            .insert(crate::ConceptRelation {
                id: Uuid::new_v4(),
                subject: fact.subject,
                relation: fact.predicate.clone(),
                object: Uuid::nil(),
                confidence: fact.confidence,
                timestamp: fact.timestamp,
            })
            .await?;
        self.emit_fact_created(&fact).await?;
        Ok(())
    }
    async fn get_fact(&self, id: &Uuid) -> SemanticResult<Option<Fact>> {
        Ok(self.facts
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .get(id)
            .cloned())
    }
    async fn facts_for_subject(&self, subject: &Uuid) -> SemanticResult<Vec<Fact>> {
        Ok(self.facts
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .values()
            .filter(|f| f.subject == *subject)
            .cloned()
            .collect())
    }
    async fn stats(&self) -> SemanticResult<SemanticStats> {
        let c = self.concepts.count().await?;
        let r = self.relationships.count().await?;
        let f = self.facts
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .len() as u64;
        Ok(SemanticStats { concepts: c, facts: f, relationships: r })
    }
}

/// No-op default implementation.
#[derive(Debug, Default)]
pub struct DefaultSemanticMemory;

#[async_trait::async_trait]
impl SemanticMemory for DefaultSemanticMemory {
    async fn create_concept(&self, _: String, _: String) -> SemanticResult<Concept> {
        Err(SemanticError::Internal("DefaultSemanticMemory not configured".into()))
    }
    async fn get_concept(&self, _: &Uuid) -> SemanticResult<Option<Concept>> { Ok(None) }
    async fn update_concept(&self, _: Concept) -> SemanticResult<()> { Ok(()) }
    async fn delete_concept(&self, _: &Uuid) -> SemanticResult<()> { Ok(()) }
    async fn list_concepts(&self) -> SemanticResult<Vec<Concept>> { Ok(Vec::new()) }
    async fn emit_concept_created(&self, _: &Concept) -> SemanticResult<()> { Ok(()) }
    async fn emit_fact_created(&self, _: &Fact) -> SemanticResult<()> { Ok(()) }
    async fn add_fact(&self, _: Fact) -> SemanticResult<()> { Ok(()) }
    async fn get_fact(&self, _: &Uuid) -> SemanticResult<Option<Fact>> { Ok(None) }
    async fn facts_for_subject(&self, _: &Uuid) -> SemanticResult<Vec<Fact>> { Ok(Vec::new()) }
    async fn stats(&self) -> SemanticResult<SemanticStats> { Ok(SemanticStats::default()) }
}
