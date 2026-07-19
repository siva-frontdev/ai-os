use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

use async_trait::async_trait;

use crate::error::{KnowledgeError, KnowledgeResult};

/// Directed edge in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEdge {
    /// Unique edge identifier.
    pub id: Uuid,
    /// Source entity ID.
    pub source: Uuid,
    /// Target entity ID.
    pub target: Uuid,
    /// Edge label.
    pub relation: String,
    /// Weight [0.0, 1.0].
    pub weight: f32,
}

/// Graph traversal interface for the knowledge base.
#[async_trait]
pub trait KnowledgeGraph: Send + Sync + std::fmt::Debug {
    /// Add a directed edge.
    async fn add_edge(&self, edge: KnowledgeEdge) -> KnowledgeResult<()>;
    /// Remove an edge by ID.
    async fn remove_edge(&self, id: Uuid) -> KnowledgeResult<()>;
    /// Find neighbors of an entity (outgoing edges).
    async fn neighbors(&self, entity: Uuid) -> KnowledgeResult<Vec<Uuid>>;
    /// Find incoming edges (inbound neighbors).
    async fn inbound_neighbors(&self, entity: Uuid) -> KnowledgeResult<Vec<Uuid>>;
    /// Find shortest path between two entities (BFS).
    async fn shortest_path(&self, from: Uuid, to: Uuid) -> KnowledgeResult<Vec<Uuid>>;
    /// Count edges for a given relation label.
    async fn count_relation(&self, relation: &str) -> KnowledgeResult<u64>;
}

/// In-memory implementation using adjacency lists in `RwLock<HashMap<Uuid, Vec<KnowledgeEdge>>>`.
#[derive(Debug, Default)]
pub struct DefaultKnowledgeGraph {
    edges: RwLock<HashMap<Uuid, KnowledgeEdge>>,
    adjacency: RwLock<HashMap<Uuid, Vec<Uuid>>>,
    reverse: RwLock<HashMap<Uuid, Vec<Uuid>>>,
}

impl DefaultKnowledgeGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KnowledgeGraph for DefaultKnowledgeGraph {
    async fn add_edge(&self, edge: KnowledgeEdge) -> KnowledgeResult<()> {
        let mut edges = self
            .edges
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        let mut adj = self
            .adjacency
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        let mut rev = self
            .reverse
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        edges.insert(edge.id, edge.clone());
        adj.entry(edge.source).or_default().push(edge.target);
        rev.entry(edge.target).or_default().push(edge.source);
        Ok(())
    }
    async fn remove_edge(&self, id: Uuid) -> KnowledgeResult<()> {
        let mut edges = self
            .edges
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        let mut adj = self
            .adjacency
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        let mut rev = self
            .reverse
            .write()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        let Some(edge) = edges.remove(&id) else {
            return Ok(());
        };
        if let Some(vec) = adj.get_mut(&edge.source) {
            vec.retain(|n| *n != edge.target);
        }
        if let Some(vec) = rev.get_mut(&edge.target) {
            vec.retain(|n| *n != edge.source);
        }
        Ok(())
    }
    async fn neighbors(&self, entity: Uuid) -> KnowledgeResult<Vec<Uuid>> {
        let adj = self
            .adjacency
            .read()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        Ok(adj.get(&entity).cloned().unwrap_or_default())
    }
    async fn inbound_neighbors(&self, entity: Uuid) -> KnowledgeResult<Vec<Uuid>> {
        let rev = self
            .reverse
            .read()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        Ok(rev.get(&entity).cloned().unwrap_or_default())
    }
    async fn shortest_path(&self, from: Uuid, to: Uuid) -> KnowledgeResult<Vec<Uuid>> {
        if from == to {
            return Ok(vec![from]);
        }
        let mut visited = HashMap::new();
        let mut queue = vec![from];
        visited.insert(from, 0usize);
        while let Some(current) = queue.pop() {
            let depth = visited[&current];
            let nbrs = self.neighbors(current).await?;
            for nbr in nbrs {
                if !visited.contains_key(&nbr) {
                    visited.insert(nbr, depth + 1);
                    if nbr == to {
                        let mut path = Vec::new();
                        let mut cur = to;
                        path.push(cur);
                        while cur != from {
                            let prev_depth = visited[&cur] - 1;
                            let rev = self.inbound_neighbors(cur).await?;
                            cur = *rev
                                .first()
                                .ok_or_else(|| KnowledgeError::NoPath(from, to, depth))?;
                            path.push(cur);
                        }
                        path.reverse();
                        return Ok(path);
                    }
                    queue.push(nbr);
                }
            }
        }
        Err(KnowledgeError::NoPath(from, to, usize::MAX))
    }
    async fn count_relation(&self, relation: &str) -> KnowledgeResult<u64> {
        let edges = self
            .edges
            .read()
            .map_err(|e| KnowledgeError::Internal(e.to_string()))?;
        Ok(edges.values().filter(|e| e.relation == relation).count() as u64)
    }
}
