# World Model

## Overview

The World Model is the persistent, structured knowledge base that represents the AI-OS companion's understanding of its operating environment. It is continuously updated through observation and reflection, and provides the context for all cognitive operations.

## Implementation

The World Model is implemented across several crates:

| Crate | Responsibility |
|---|---|
| `brain-core` (`model.rs`) | Core `WorldModel`, `Entity`, `Relationship`, `ResourceModel` types |
| `memory-core` (`wm.rs`) | `Entity`, `Relationship`, `EntityLifecycle`, `WorldModelStore` trait |
| `memory-storage` (`wm_store.rs`) | `InMemoryWorldModelStore`, persistence layer |
| `brain-coordinator` (`world_model.rs`) | `WorldModelService` — operational wrapper with app/goal tracking |

## Data Model

### Entity
```rust
struct Entity {
    id: EntityId,
    name: String,
    entity_type: String,        // "person", "project", "skill", "task", etc.
    properties: HashMap<String, Value>,
    confidence: f32,            // 0.0–1.0
    importance: f32,            // 0.0–1.0
    lifecycle: EntityLifecycle, // Active | Archived | Deleted
    created_at: Timestamp,
    updated_at: Timestamp,
}
```

### Relationship
```rust
struct Relationship {
    id: RelationshipId,
    relation_type: String,      // "knows", "owns", "works_on", etc.
    source: EntityId,
    target: EntityId,
    confidence: f32,
    created_at: Timestamp,
}
```

### WorldModel
```rust
struct WorldModel {
    entities: HashMap<EntityId, Entity>,
    relationships: Vec<Relationship>,
    known_apps: Vec<AppEntry>,
    system_facts: HashMap<String, String>,
    resource_state: ResourceModel,
}
```

## Persistence

The World Model is serialized to JSON and restored on startup:

```
~/.local/share/ai-os-companion/world_model.json
```

**Format:**
```json
{
  "entities": [...],
  "relationships": [...],
  "version": 1
}
```

**Load cycle (CompanionHost::load):**
1. `PersistenceManager::load()` reads JSON
2. Entities + relationships inserted into `InMemoryWorldModelStore`
3. `CompanionHost` initializes with restored store

**Save cycle (CompanionHost::save):**
1. `store.all_entities()` + `store.all_relationships()`
2. `PersistenceManager::save()` writes JSON
3. Audit log also persisted

## Query API

| Method | Description |
|---|---|
| `store.insert_entity()` | Add new entity |
| `store.get_entity(id)` | Retrieve by ID |
| `store.all_entities()` | All active entities |
| `store.insert_relationship()` | Add relationship |
| `store.all_relationships()` | All relationships |
| `store.delete_entity()` | Mark as Deleted |

## Integration with Cognitive Loop

The `CognitiveLoopService::build_context()` scores all active entities by:
```
score = importance × 0.5 + recency × 0.3 + confidence × 0.2
```

Top-5 entities become the "continuity context" fed to the LLM for planning.

## Security

- **Sandbox**: File system paths canonicalized before storage
- **Privacy**: `PrivacyConfig` controls encryption of persisted data
- **Retention**: `RetentionConfig` governs TTL for different entity types

## Testing

Unit tests in `brain/brain-coordinator/src/world_model.rs` and `crates/memory-core/tests/`.
Integration test: `cognitive_mixed_plan_dispatches_runtime_only` verifies filesystem writes update the model.