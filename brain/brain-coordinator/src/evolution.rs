use std::collections::HashMap;
use std::sync::Arc;

use intelligence_coordinator::world_understanding::{
    StructuredWorldUpdate, WorldEntity, WorldRelationship,
};
use memory_core::wm::{Entity, EntityId, Relationship, RelationshipId, Value};
use memory_storage::wm_store::WorldModelStore;

/// Describes what the Evolution Engine changed during one cycle.
#[derive(Debug, Clone, Default)]
pub struct EvolutionReport {
    pub entities_created: Vec<EntitySnap>,
    pub entities_updated: Vec<EntityUpdate>,
    pub relationships_created: Vec<RelSnap>,
    pub relationships_strengthened: Vec<RelSnap>,
    pub confidence_changes: Vec<ConfidenceChange>,
    pub importance_changes: Vec<ImportanceChange>,
}

#[derive(Debug, Clone)]
pub struct EntitySnap {
    pub id: EntityId,
    pub name: String,
    pub entity_type: String,
    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct EntityUpdate {
    pub id: EntityId,
    pub name: String,
    pub entity_type: String,
    pub version: u64,
    pub previous_version: u64,
    pub fields_changed: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RelSnap {
    pub id: RelationshipId,
    pub source_name: String,
    pub target_name: String,
    pub relationship_type: String,
    pub weight: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct ConfidenceChange {
    pub id: EntityId,
    pub name: String,
    pub previous: f32,
    pub current: f32,
}

#[derive(Debug, Clone)]
pub struct ImportanceChange {
    pub id: EntityId,
    pub name: String,
    pub previous: f32,
    pub current: f32,
}

/// The Evolution Engine.
///
/// Its single responsibility is to evolve the World Model:
///
/// - Merge duplicate entities (match by name)
/// - Update existing entities with new observations
/// - Maintain entity version history
/// - Create relationships for new observations
/// - Strengthen relationships on repeat observation
/// - Track confidence and importance evolution
#[derive(Debug)]
pub struct EvolutionEngine {
    store: Arc<dyn WorldModelStore>,
}

impl EvolutionEngine {
    pub fn new(store: Arc<dyn WorldModelStore>) -> Self {
        Self { store }
    }

    /// Evolve the World Model by merging the given understanding
    /// with the existing state. Returns a report of what changed.
    pub async fn evolve(&self, understanding: &StructuredWorldUpdate) -> EvolutionReport {
        let mut report = EvolutionReport {
            entities_created: Vec::new(),
            entities_updated: Vec::new(),
            relationships_created: Vec::new(),
            relationships_strengthened: Vec::new(),
            confidence_changes: Vec::new(),
            importance_changes: Vec::new(),
        };

        // Phase 1: Evolve entities
        let mut name_to_id: HashMap<String, EntityId> = HashMap::new();
        for we in &understanding.entities {
            let existing = self.find_entity_by_name(&we.name).await;
            match existing {
                Some(mut entity) => {
                    let prev_version = entity.version;
                    let prev_confidence = entity.confidence;
                    let prev_importance = entity.importance;
                    let mut fields_changed = Vec::new();

                    if we.entity_type != entity.entity_type && !we.entity_type.is_empty() {
                        fields_changed.push("entity_type".into());
                        entity.entity_type = we.entity_type.clone();
                    }

                    let new_confidence = merge_confidence(prev_confidence, we.confidence as f32);
                    if (new_confidence - prev_confidence).abs() > 0.01 {
                        fields_changed.push("confidence".into());
                        entity.confidence = new_confidence;
                        report.confidence_changes.push(ConfidenceChange {
                            id: entity.id,
                            name: entity.name.clone(),
                            previous: prev_confidence,
                            current: new_confidence,
                        });
                    }

                    let new_importance = merge_importance(prev_importance, we.importance as f32);
                    if (new_importance - prev_importance).abs() > 0.01 {
                        fields_changed.push("importance".into());
                        entity.importance = new_importance;
                        report.importance_changes.push(ImportanceChange {
                            id: entity.id,
                            name: entity.name.clone(),
                            previous: prev_importance,
                            current: new_importance,
                        });
                    }

                    for (k, v) in &we.properties {
                        let val = Value::String(v.clone());
                        let key_str = k.to_string();
                        let is_new = !entity.properties.contains_key(&key_str);
                        entity.properties.insert(key_str.clone(), val);
                        if is_new {
                            fields_changed.push(format!("property:{key_str}"));
                        } else {
                            fields_changed.push(format!("property:{key_str} (updated)"));
                        }
                    }

                    entity.touch();
                    self.store.update_entity(&entity).await;
                    name_to_id.insert(we.name.clone(), entity.id);

                    report.entities_updated.push(EntityUpdate {
                        id: entity.id,
                        name: entity.name.clone(),
                        entity_type: entity.entity_type.clone(),
                        version: entity.version,
                        previous_version: prev_version,
                        fields_changed,
                    });
                }
                None => {
                    let entity = world_entity_to_entity(we);
                    let id = self.store.insert_entity(entity).await;
                    name_to_id.insert(we.name.clone(), id);
                    report.entities_created.push(EntitySnap {
                        id,
                        name: we.name.clone(),
                        entity_type: we.entity_type.clone(),
                        version: 1,
                    });
                }
            }
        }

        // Phase 2: Evolve relationships
        for wr in &understanding.relationships {
            let src_id = match name_to_id.get(&wr.source).copied() {
                Some(id) => id,
                None => match self.store.search_entities_by_name(&wr.source).await.first() {
                    Some(e) => {
                        name_to_id.insert(wr.source.clone(), e.id);
                        e.id
                    }
                    None => continue,
                },
            };
            let tgt_id = match name_to_id.get(&wr.target).copied() {
                Some(id) => id,
                None => match self.store.search_entities_by_name(&wr.target).await.first() {
                    Some(e) => {
                        name_to_id.insert(wr.target.clone(), e.id);
                        e.id
                    }
                    None => continue,
                },
            };

            let existing_rels = self.store.get_relationships_between(&src_id, &tgt_id).await;
            let matching_rel = existing_rels
                .iter()
                .find(|r| r.relationship_type == wr.relationship_type)
                .cloned();

            match matching_rel {
                Some(mut rel) => {
                    rel.weight = strengthen_weight(rel.weight, wr.weight as f32);
                    rel.confidence = merge_confidence(rel.confidence, wr.confidence as f32);
                    rel.touch();
                    self.store.update_relationship(&rel).await;

                    report.relationships_strengthened.push(RelSnap {
                        id: rel.id,
                        source_name: wr.source.clone(),
                        target_name: wr.target.clone(),
                        relationship_type: wr.relationship_type.clone(),
                        weight: rel.weight,
                        confidence: rel.confidence,
                    });
                }
                None => {
                    let rel = world_relationship_to_relationship(wr, src_id, tgt_id);
                    let id = self.store.insert_relationship(&rel).await;
                    report.relationships_created.push(RelSnap {
                        id,
                        source_name: wr.source.clone(),
                        target_name: wr.target.clone(),
                        relationship_type: wr.relationship_type.clone(),
                        weight: rel.weight,
                        confidence: rel.confidence,
                    });
                }
            }
        }

        report
    }

    async fn find_entity_by_name(&self, name: &str) -> Option<Entity> {
        let results = self.store.search_entities_by_name(name).await;
        let lower = name.to_lowercase();
        results.into_iter().find(|e| e.name.to_lowercase() == lower)
    }
}

// ── Merge heuristics ────────────────────────────────────────

fn merge_confidence(previous: f32, observation: f32) -> f32 {
    let blended = previous + 0.4 * (observation - previous);
    blended.clamp(0.0, 1.0)
}

fn merge_importance(previous: f32, observation: f32) -> f32 {
    let boosted = if observation > previous {
        previous + 0.3 * (observation - previous)
    } else {
        previous + 0.1 * (observation - previous)
    };
    boosted.clamp(0.0, 1.0)
}

fn strengthen_weight(previous: f32, observation: f32) -> f32 {
    let strengthened = previous + 0.2 * (observation - previous);
    strengthened.clamp(0.0, 1.0)
}

// ── Conversion helpers ──────────────────────────────────────

pub fn world_entity_to_entity(we: &WorldEntity) -> Entity {
    let mut properties = HashMap::new();
    for (k, v) in &we.properties {
        properties.insert(k.clone(), Value::String(v.clone()));
    }
    let mut entity = Entity::new(&we.entity_type, &we.name)
        .with_confidence(we.confidence as f32)
        .with_importance(we.importance as f32);
    for (k, v) in properties {
        entity = entity.with_property(&k, v);
    }
    entity
}

pub fn world_relationship_to_relationship(
    wr: &WorldRelationship,
    source_id: EntityId,
    target_id: EntityId,
) -> Relationship {
    Relationship::new(&wr.relationship_type, source_id, target_id)
        .with_confidence(wr.confidence as f32)
        .with_weight(wr.weight as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_storage::wm_store::InMemoryWorldModelStore;

    fn empty_understanding() -> StructuredWorldUpdate {
        StructuredWorldUpdate {
            entities: vec![],
            relationships: vec![],
            state_changes: vec![],
            new_observations: vec![],
            open_questions: vec![],
            possible_hypotheses: vec![],
        }
    }

    #[tokio::test]
    async fn test_evolve_creates_new_entities() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let engine = EvolutionEngine::new(store.clone());

        let understanding = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "Rust".into(),
                entity_type: "skill".into(),
                properties: HashMap::new(),
                confidence: 0.8,
                importance: 0.6,
            }],
            ..empty_understanding()
        };

        let report = engine.evolve(&understanding).await;
        assert_eq!(report.entities_created.len(), 1);
        assert_eq!(report.entities_created[0].name, "Rust");
        assert_eq!(store.entity_count().await, 1);
    }

    #[tokio::test]
    async fn test_evolve_merges_existing_entity() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let engine = EvolutionEngine::new(store.clone());

        let day1 = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "Rust".into(),
                entity_type: "skill".into(),
                properties: HashMap::new(),
                confidence: 0.7,
                importance: 0.5,
            }],
            ..empty_understanding()
        };
        engine.evolve(&day1).await;

        let day2 = StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: "Rust".into(),
                entity_type: "skill".into(),
                properties: {
                    let mut m = HashMap::new();
                    m.insert("progress".into(), "ownership".into());
                    m
                },
                confidence: 0.9,
                importance: 0.8,
            }],
            ..empty_understanding()
        };
        let report = engine.evolve(&day2).await;

        assert!(report.entities_created.is_empty());
        assert_eq!(report.entities_updated.len(), 1);
        assert_eq!(report.entities_updated[0].name, "Rust");
        assert_eq!(report.entities_updated[0].version, 2);
        assert_eq!(store.entity_count().await, 1);

        let all = store.all_entities().await;
        let rust = all.iter().find(|e| e.name == "Rust").unwrap();
        assert!(rust.confidence > 0.7);
        assert!(rust.confidence <= 1.0);
        assert_eq!(
            rust.properties.get("progress").unwrap().as_str(),
            Some("ownership")
        );
    }

    #[tokio::test]
    async fn test_evolve_creates_relationships() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let engine = EvolutionEngine::new(store.clone());

        let understanding = StructuredWorldUpdate {
            entities: vec![
                WorldEntity {
                    name: "Alice".into(),
                    entity_type: "person".into(),
                    properties: HashMap::new(),
                    confidence: 0.9,
                    importance: 0.7,
                },
                WorldEntity {
                    name: "Rust".into(),
                    entity_type: "skill".into(),
                    properties: HashMap::new(),
                    confidence: 0.8,
                    importance: 0.6,
                },
            ],
            relationships: vec![WorldRelationship {
                source: "Alice".into(),
                target: "Rust".into(),
                relationship_type: "learning".into(),
                confidence: 0.85,
                weight: 0.7,
            }],
            ..empty_understanding()
        };

        let report = engine.evolve(&understanding).await;

        assert_eq!(report.entities_created.len(), 2);
        assert_eq!(report.relationships_created.len(), 1);
        assert_eq!(store.relationship_count().await, 1);

        let rel = &report.relationships_created[0];
        assert_eq!(rel.relationship_type, "learning");
        assert_eq!(rel.source_name, "Alice");
        assert_eq!(rel.target_name, "Rust");
    }

    #[tokio::test]
    async fn test_evolve_strengthens_repeat_relationships() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let engine = EvolutionEngine::new(store.clone());

        // Day 1: first observation of relationship
        let day1 = StructuredWorldUpdate {
            entities: vec![
                WorldEntity {
                    name: "Alice".into(),
                    entity_type: "person".into(),
                    properties: HashMap::new(),
                    confidence: 0.9,
                    importance: 0.7,
                },
                WorldEntity {
                    name: "Rust".into(),
                    entity_type: "skill".into(),
                    properties: HashMap::new(),
                    confidence: 0.8,
                    importance: 0.6,
                },
            ],
            relationships: vec![WorldRelationship {
                source: "Alice".into(),
                target: "Rust".into(),
                relationship_type: "learning".into(),
                confidence: 0.7,
                weight: 0.6,
            }],
            ..empty_understanding()
        };
        let r1 = engine.evolve(&day1).await;
        assert_eq!(r1.relationships_created.len(), 1);
        assert_eq!(r1.relationships_strengthened.len(), 0);

        // Day 2: same relationship observed again, with higher confidence
        let day2 = StructuredWorldUpdate {
            entities: vec![
                WorldEntity {
                    name: "Alice".into(),
                    entity_type: "person".into(),
                    properties: HashMap::new(),
                    confidence: 0.95,
                    importance: 0.75,
                },
                WorldEntity {
                    name: "Rust".into(),
                    entity_type: "skill".into(),
                    properties: {
                        let mut m = HashMap::new();
                        m.insert("level".into(), "advanced".into());
                        m
                    },
                    confidence: 0.9,
                    importance: 0.85,
                },
            ],
            relationships: vec![WorldRelationship {
                source: "Alice".into(),
                target: "Rust".into(),
                relationship_type: "learning".into(),
                confidence: 0.9,
                weight: 0.85,
            }],
            ..empty_understanding()
        };
        let r2 = engine.evolve(&day2).await;

        // Relationship should be strengthened, not created
        assert_eq!(r2.relationships_created.len(), 0);
        assert_eq!(r2.relationships_strengthened.len(), 1);

        let strengthened = &r2.relationships_strengthened[0];
        assert!(strengthened.confidence > 0.7);
        assert!(strengthened.confidence <= 1.0);
        assert!(strengthened.weight > 0.6);

        // Should still only have 1 relationship
        assert_eq!(store.relationship_count().await, 1);
    }

    #[tokio::test]
    async fn test_evolve_multiple_observations_no_duplicates() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let engine = EvolutionEngine::new(store.clone());

        let day1 = StructuredWorldUpdate {
            entities: vec![
                WorldEntity {
                    name: "Alice".into(),
                    entity_type: "person".into(),
                    properties: HashMap::new(),
                    confidence: 0.9,
                    importance: 0.7,
                },
                WorldEntity {
                    name: "Rust".into(),
                    entity_type: "skill".into(),
                    properties: HashMap::new(),
                    confidence: 0.8,
                    importance: 0.6,
                },
            ],
            relationships: vec![WorldRelationship {
                source: "Alice".into(),
                target: "Rust".into(),
                relationship_type: "learning".into(),
                confidence: 0.85,
                weight: 0.7,
            }],
            ..empty_understanding()
        };
        let r1 = engine.evolve(&day1).await;
        assert_eq!(r1.entities_created.len(), 2);
        assert_eq!(store.entity_count().await, 2);

        let day20 = StructuredWorldUpdate {
            entities: vec![
                WorldEntity {
                    name: "Alice".into(),
                    entity_type: "person".into(),
                    properties: HashMap::new(),
                    confidence: 0.9,
                    importance: 0.7,
                },
                WorldEntity {
                    name: "Rust".into(),
                    entity_type: "skill".into(),
                    properties: {
                        let mut m = HashMap::new();
                        m.insert("progress".into(), "mastery".into());
                        m
                    },
                    confidence: 0.9,
                    importance: 0.85,
                },
            ],
            relationships: vec![WorldRelationship {
                source: "Alice".into(),
                target: "Rust".into(),
                relationship_type: "learning".into(),
                confidence: 0.9,
                weight: 0.85,
            }],
            ..empty_understanding()
        };
        let r20 = engine.evolve(&day20).await;

        assert_eq!(r20.entities_created.len(), 0);
        assert_eq!(r20.entities_updated.len(), 2);
        assert_eq!(store.entity_count().await, 2);

        let all = store.all_entities().await;
        let rust = all.iter().find(|e| e.name == "Rust").unwrap();
        assert_eq!(rust.version, 2);
        assert!(
            rust.importance > 0.6,
            "importance should increase from repeated observation"
        );
    }

    #[tokio::test]
    async fn test_confidence_evolves_with_repeated_observations() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let engine = EvolutionEngine::new(store.clone());

        let obs = |name: &str, conf: f64, imp: f64| StructuredWorldUpdate {
            entities: vec![WorldEntity {
                name: name.into(),
                entity_type: "skill".into(),
                properties: HashMap::new(),
                confidence: conf,
                importance: imp,
            }],
            ..empty_understanding()
        };

        engine.evolve(&obs("Rust", 0.5, 0.3)).await;
        engine.evolve(&obs("Rust", 0.7, 0.5)).await;
        let r3 = engine.evolve(&obs("Rust", 0.9, 0.8)).await;

        assert_eq!(r3.confidence_changes.len(), 1);
        let change = &r3.confidence_changes[0];
        assert!(change.current > change.previous);

        let all = store.all_entities().await;
        let rust = all.iter().find(|e| e.name == "Rust").unwrap();
        assert_eq!(rust.version, 3);
    }

    #[tokio::test]
    async fn test_evolve_new_entity_does_not_match_different_name() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let engine = EvolutionEngine::new(store.clone());

        engine
            .evolve(&StructuredWorldUpdate {
                entities: vec![WorldEntity {
                    name: "Rust".into(),
                    entity_type: "skill".into(),
                    properties: HashMap::new(),
                    confidence: 0.8,
                    importance: 0.6,
                }],
                ..empty_understanding()
            })
            .await;

        engine
            .evolve(&StructuredWorldUpdate {
                entities: vec![WorldEntity {
                    name: "Python".into(),
                    entity_type: "skill".into(),
                    properties: HashMap::new(),
                    confidence: 0.7,
                    importance: 0.5,
                }],
                ..empty_understanding()
            })
            .await;

        assert_eq!(store.entity_count().await, 2);
    }

    #[tokio::test]
    async fn test_entity_lifecycle_project_evolution() {
        let store = Arc::new(InMemoryWorldModelStore::new());
        let engine = EvolutionEngine::new(store.clone());

        engine
            .evolve(&StructuredWorldUpdate {
                entities: vec![WorldEntity {
                    name: "AI-OS".into(),
                    entity_type: "project".into(),
                    properties: {
                        let mut m = HashMap::new();
                        m.insert("status".into(), "started".into());
                        m
                    },
                    confidence: 0.7,
                    importance: 0.6,
                }],
                ..empty_understanding()
            })
            .await;

        assert_eq!(store.entity_count().await, 1);
        let project = &store.all_entities().await[0];
        assert_eq!(
            project.properties.get("status").unwrap().as_str(),
            Some("started")
        );
        assert_eq!(project.version, 1);

        engine
            .evolve(&StructuredWorldUpdate {
                entities: vec![WorldEntity {
                    name: "AI-OS".into(),
                    entity_type: "project".into(),
                    properties: {
                        let mut m = HashMap::new();
                        m.insert("status".into(), "active".into());
                        m
                    },
                    confidence: 0.85,
                    importance: 0.8,
                }],
                ..empty_understanding()
            })
            .await;

        assert_eq!(store.entity_count().await, 1);
        let project = store
            .all_entities()
            .await
            .into_iter()
            .find(|e| e.name == "AI-OS")
            .unwrap();
        assert_eq!(
            project.properties.get("status").unwrap().as_str(),
            Some("active")
        );
        assert_eq!(project.version, 2);
        assert!(project.importance > 0.6);

        engine
            .evolve(&StructuredWorldUpdate {
                entities: vec![WorldEntity {
                    name: "AI-OS".into(),
                    entity_type: "project".into(),
                    properties: {
                        let mut m = HashMap::new();
                        m.insert("status".into(), "blocked".into());
                        m
                    },
                    confidence: 0.8,
                    importance: 0.75,
                }],
                ..empty_understanding()
            })
            .await;

        let project = store
            .all_entities()
            .await
            .into_iter()
            .find(|e| e.name == "AI-OS")
            .unwrap();
        assert_eq!(
            project.properties.get("status").unwrap().as_str(),
            Some("blocked")
        );
        assert_eq!(project.version, 3);
    }

    #[tokio::test]
    async fn test_merge_confidence_never_exceeds_1() {
        let mut c = 0.5;
        for _ in 0..100 {
            c = merge_confidence(c, 1.0);
        }
        assert!(c <= 1.0);
        assert!(c > 0.9);
    }

    #[tokio::test]
    async fn test_merge_importance_saturates() {
        let mut imp = 0.5;
        for _ in 0..50 {
            imp = merge_importance(imp, 1.0);
        }
        assert!(imp <= 1.0);
    }

    #[test]
    fn test_strengthen_weight_saturates() {
        let mut w = 0.5;
        for _ in 0..50 {
            w = strengthen_weight(w, 1.0);
        }
        assert!(w <= 1.0);
    }
}
