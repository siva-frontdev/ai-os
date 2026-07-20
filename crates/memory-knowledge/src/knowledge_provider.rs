use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{KnowledgeError, KnowledgeResult};
use crate::knowledge_base::{Entity, Fact, KnowledgeBase, KnowledgeStats};

/// Merge strategy for combining entity facts.
#[derive(Debug, Clone, Copy, Default)]
pub enum MergeStrategy {
    /// Take the fact with the highest confidence.
    #[default]
    HighestConfidence,
    /// Combine all unique facts.
    Union,
}

/// Cross-memory knowledge provider merging information from episodic and semantic sources.
#[async_trait]
pub trait KnowledgeProvider: Send + Sync + std::fmt::Debug {
    /// Merge facts from source entities into a target entity.
    async fn merge_entities(
        &self,
        target: Uuid,
        sources: &[Uuid],
        strategy: MergeStrategy,
    ) -> KnowledgeResult<usize>;
    /// Retrieve all facts for an entity, joining all backing sources.
    async fn get_entity_facts(&self, entity_id: Uuid) -> KnowledgeResult<Vec<Fact>>;
    /// Compute aggregate knowledge statistics.
    async fn overall_stats(&self) -> KnowledgeResult<KnowledgeStats>;
}

/// In-memory implementation wrapping a single `Arc<dyn KnowledgeBase>`.
/// In production this would compose episodic + semantic backends. For now
/// it delegates to the wrapped knowledge base, satisfying the trait contract.
#[derive(Debug)]
pub struct DefaultKnowledgeProvider {
    base: std::sync::Arc<dyn KnowledgeBase>,
}

impl Default for DefaultKnowledgeProvider {
    fn default() -> Self {
        Self {
            base: std::sync::Arc::new(crate::knowledge_base::DefaultKnowledgeBase),
        }
    }
}

impl DefaultKnowledgeProvider {
    /// Wrap an existing knowledge base.
    pub fn new(base: std::sync::Arc<dyn KnowledgeBase>) -> Self {
        Self { base }
    }
}

#[async_trait]
impl KnowledgeProvider for DefaultKnowledgeProvider {
    async fn merge_entities(
        &self,
        target: Uuid,
        sources: &[Uuid],
        _strategy: MergeStrategy,
    ) -> KnowledgeResult<usize> {
        let mut merged = 0usize;
        for src in sources {
            let facts = self.base.facts_for_entity(src).await?;
            for fact in facts {
                self.base
                    .insert_fact(Fact {
                        entity_id: target,
                        ..fact
                    })
                    .await?;
                merged += 1;
            }
        }
        Ok(merged)
    }
    async fn get_entity_facts(&self, entity_id: Uuid) -> KnowledgeResult<Vec<Fact>> {
        self.base.facts_for_entity(&entity_id).await
    }
    async fn overall_stats(&self) -> KnowledgeResult<KnowledgeStats> {
        self.base.stats().await
    }
}
