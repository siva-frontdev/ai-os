use crate::types::{MemoryId, RelationType};
use serde::{Deserialize, Serialize};

/// A typed, weighted link between two memory objects.
///
/// Relationships form a directed graph on top of flat memory storage.
/// They are used for knowledge graph traversal, associative recall,
/// and deriving causal or sequential patterns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relationship {
    /// The target memory object's ID.
    pub target_id: MemoryId,
    /// The type of relationship.
    pub relation_type: RelationType,
    /// The strength of the relationship (0.0 - 1.0).
    pub weight: f32,
}

impl Relationship {
    /// Create a new relationship.
    pub fn new(target_id: MemoryId, relation_type: RelationType, weight: f32) -> Self {
        Self {
            target_id,
            relation_type,
            weight,
        }
    }

    /// Create a "References" relationship with weight 1.0.
    pub fn reference(target_id: MemoryId) -> Self {
        Self {
            target_id,
            relation_type: RelationType::References,
            weight: 1.0,
        }
    }

    /// Create a "PartOf" relationship with weight 1.0.
    pub fn part_of(target_id: MemoryId) -> Self {
        Self {
            target_id,
            relation_type: RelationType::PartOf,
            weight: 1.0,
        }
    }

    /// Create a "Sequence" relationship with weight 1.0.
    pub fn sequence(target_id: MemoryId) -> Self {
        Self {
            target_id,
            relation_type: RelationType::Sequence,
            weight: 1.0,
        }
    }

    /// Create a "Supports" relationship with weight 1.0.
    pub fn supports(target_id: MemoryId) -> Self {
        Self {
            target_id,
            relation_type: RelationType::Supports,
            weight: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relationship_creation() {
        let id = MemoryId::new();
        let rel = Relationship::new(id, RelationType::References, 0.8);
        assert_eq!(rel.target_id, id);
        assert_eq!(rel.relation_type, RelationType::References);
        assert!((rel.weight - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_convenience_constructors() {
        let id = MemoryId::new();
        assert_eq!(
            Relationship::reference(id.clone()).relation_type,
            RelationType::References
        );
        assert_eq!(
            Relationship::part_of(id.clone()).relation_type,
            RelationType::PartOf
        );
        assert_eq!(
            Relationship::sequence(id.clone()).relation_type,
            RelationType::Sequence
        );
        assert_eq!(
            Relationship::supports(id).relation_type,
            RelationType::Supports
        );
    }

    #[test]
    fn test_relationship_equality() {
        let id = MemoryId::new();
        let a = Relationship::new(id, RelationType::References, 0.5);
        let b = Relationship::new(id, RelationType::References, 0.5);
        assert_eq!(a, b);
    }
}
