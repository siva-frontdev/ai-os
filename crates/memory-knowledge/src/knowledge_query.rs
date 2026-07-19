use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

use async_trait::async_trait;

use crate::error::{KnowledgeError, KnowledgeResult};
use crate::knowledge_base::{Entity, Fact, KnowledgeBase};

/// A single query match result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMatch {
    /// Matched entity ID, if applicable.
    pub entity_id: Option<Uuid>,
    /// Matched fact ID, if applicable.
    pub fact_id: Option<Uuid>,
    /// Relevance score [0.0, 1.0].
    pub score: f32,
    /// Payload summary string.
    pub summary: String,
}

/// Consolidated query result set.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryResult {
    /// Ordered list of results (highest score first).
    pub matches: Vec<QueryMatch>,
    /// Total number of candidates examined.
    pub examined: usize,
}

/// Filter criteria for knowledge queries.
#[derive(Debug, Clone, Default)]
pub struct KnowledgeFilter {
    /// Limit to this entity type.
    pub entity_type: Option<String>,
    /// Minimum confidence threshold.
    pub min_confidence: Option<f32>,
    /// Full-text substring filter on entity names.
    pub name_contains: Option<String>,
}

/// Complex query interface across knowledge sub-systems.
#[async_trait]
pub trait KnowledgeQuery: Send + Sync + std::fmt::Debug {
    /// Execute a filtered query.
    async fn query(&self, filter: KnowledgeFilter, limit: usize) -> KnowledgeResult<QueryResult>;
    /// Search facts by predicate.
    async fn search_by_predicate(&self, predicate: &str) -> KnowledgeResult<Vec<Fact>>;
    /// Search entities by name substring.
    async fn search_by_name(&self, name: &str) -> KnowledgeResult<Vec<Entity>>;
    /// Facts with confidence above threshold.
    async fn high_confidence_facts(&self, threshold: f32) -> KnowledgeResult<Vec<Fact>>;
    /// Maximum path length for async route (delegated to graph).
    async fn path(&self, from: Uuid, to: Uuid, max_depth: usize) -> KnowledgeResult<Vec<Uuid>>;
}

/// In-memory implementation scanning HashMap indexes.
#[derive(Debug, Default)]
pub struct DefaultKnowledgeQuery {
    base: std::sync::Arc<dyn KnowledgeBase>,
}

impl DefaultKnowledgeQuery {
    /// Wrap an existing knowledge base.
    pub fn new(base: std::sync::Arc<dyn KnowledgeBase>) -> Self {
        Self { base }
    }
}

#[async_trait]
impl KnowledgeQuery for DefaultKnowledgeQuery {
    async fn query(&self, filter: KnowledgeFilter, limit: usize) -> KnowledgeResult<QueryResult> {
        let entities = self.base.list_entities().await?;
        let mut matches = Vec::new();
        for ent in entities {
            if let Some(ref et) = filter.entity_type {
                if ent.entity_type != *et {
                    continue;
                }
            }
            if let Some(ref substr) = filter.name_contains {
                if !ent.name.contains(substr) {
                    continue;
                }
            }
            let score = 1.0;
            matches.push(QueryMatch {
                entity_id: Some(ent.id),
                fact_id: None,
                score,
                summary: ent.name.clone(),
            });
        }
        matches.truncate(limit);
        Ok(QueryResult {
            matches,
            examined: entities.len(),
        })
    }
    async fn search_by_predicate(&self, predicate: &str) -> KnowledgeResult<Vec<Fact>> {
        let all = self.base.list_entities().await?;
        let mut out = Vec::new();
        for ent in &all {
            let facts = self.base.facts_for_entity(&ent.id).await?;
            out.extend(facts.into_iter().filter(|f| f.predicate == predicate));
        }
        Ok(out)
    }
    async fn search_by_name(&self, name: &str) -> KnowledgeResult<Vec<Entity>> {
        let all = self.base.list_entities().await?;
        Ok(all.into_iter().filter(|e| e.name.contains(name)).collect())
    }
    async fn high_confidence_facts(&self, threshold: f32) -> KnowledgeResult<Vec<Fact>> {
        let entities = self.base.list_entities().await?;
        let mut out = Vec::new();
        for ent in &entities {
            let facts = self.base.facts_for_entity(&ent.id).await?;
            out.extend(facts.into_iter().filter(|f| f.confidence >= threshold));
        }
        Ok(out)
    }
    async fn path(&self, from: Uuid, to: Uuid, max_depth: usize) -> KnowledgeResult<Vec<Uuid>> {
        if from == to {
            return Ok(vec![from]);
        }
        let mut visited = HashMap::new();
        let mut queue = vec![from];
        visited.insert(from, 0usize);
        while let Some(current) = queue.pop() {
            let depth = visited[&current];
            if depth >= max_depth {
                continue;
            }
            let ents = self.base.list_entities().await?;
            let facts = self.base.facts_for_entity(&current).await?;
            for fact in facts {
                if let Ok(Some(target)) = Uuid::parse_str(&fact.object) {
                    if !visited.contains_key(&target) {
                        visited.insert(target, depth + 1);
                        if target == to {
                            return Ok(vec![from, to]);
                        }
                        queue.push(target);
                    }
                }
            }
        }
        Err(KnowledgeError::NoPath(from, to, max_depth))
    }
}
