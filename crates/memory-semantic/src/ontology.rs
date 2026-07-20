use crate::error::{SemanticError, SemanticResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

/// Descriptor for an ontology category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryDescriptor {
    pub name: String,
    pub description: String,
    pub min_confidence: f32,
}

impl CategoryDescriptor {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            min_confidence: 0.0,
        }
    }
}

/// Resolved ontology node for a concept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyNode {
    pub concept_id: Uuid,
    pub category: String,
    pub confidence: f32,
}

/// Trait for ontology registration and concept classification.
#[async_trait]
pub trait OntologyProvider: Send + Sync + std::fmt::Debug {
    async fn register_category(&self, descriptor: CategoryDescriptor) -> SemanticResult<()>;
    async fn assign_category(
        &self,
        concept_id: Uuid,
        category: String,
        confidence: f32,
    ) -> SemanticResult<()>;
    async fn resolve(&self, concept_id: &Uuid) -> SemanticResult<Option<OntologyNode>>;
    async fn list_categories(&self) -> SemanticResult<Vec<CategoryDescriptor>>;
}

#[derive(Debug, Default)]
pub struct DefaultOntologyProvider {
    categories: RwLock<HashMap<String, CategoryDescriptor>>,
    assignments: RwLock<HashMap<Uuid, OntologyNode>>,
}

impl DefaultOntologyProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OntologyProvider for DefaultOntologyProvider {
    async fn register_category(&self, descriptor: CategoryDescriptor) -> SemanticResult<()> {
        self.categories
            .write()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .insert(descriptor.name.clone(), descriptor);
        Ok(())
    }
    async fn assign_category(
        &self,
        concept_id: Uuid,
        category: String,
        confidence: f32,
    ) -> SemanticResult<()> {
        self.assignments
            .write()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .insert(
                concept_id,
                OntologyNode {
                    concept_id,
                    category,
                    confidence,
                },
            );
        Ok(())
    }
    async fn resolve(&self, concept_id: &Uuid) -> SemanticResult<Option<OntologyNode>> {
        Ok(self
            .assignments
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .get(concept_id)
            .cloned())
    }
    async fn list_categories(&self) -> SemanticResult<Vec<CategoryDescriptor>> {
        Ok(self
            .categories
            .read()
            .map_err(|e| SemanticError::Internal(e.to_string()))?
            .values()
            .cloned()
            .collect())
    }
}
