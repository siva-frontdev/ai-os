# ADR-0006: World Model Architecture

## Status

Accepted

## Date

2026-07-27

## Context

### The Problem

The current AI-native OS architecture organizes knowledge and system behavior into layered modules with predefined responsibilities: Memory Platform (4-tier store), Brain Platform (goals/planning/reasoning/decision), Perception Platform (observation pipeline), Execution Platform (tool dispatch), and Intelligence Platform (model integration).

While this layered separation provides clean engineering boundaries, it embeds an implicit assumption: intelligence can be decomposed into independent functional domains. The Memory Platform stores "memories." The Brain Platform manages "goals" and "plans." Perception observes "events." Each platform owns a slice of the user's world, labeled with human-convenient categories.

This approach creates several architectural problems:

1. **Knowledge silos.** A user's "project" exists as scattered fragments: a goal in Brain, a working memory entry in Memory, an observation in Perception, a task in Runtime. No single structure represents the project as a coherent entity in the user's world.

2. **Rigid domain boundaries.** The architecture has no mechanism to represent a fundamentally new concept without mapping it to existing module types. The system cannot natively understand that a "habit," a "relationship," and a "skill" are the same kind of thing — entities with properties and connections.

3. **Feature-centric reasoning.** The Brain reasons about goals, but it does not reason about the user's world holistically. A calendar event and a health metric are different types handled by different subsystems, even though both are entities in the user's life connected by relationships.

4. **Brittle context management.** Context today is session-scoped metadata. It is not generated dynamically from what the system knows about the user's current situation. The system cannot answer "what is relevant right now?" without being told.

5. **Notifications as timers.** Notifications are scheduled events rather than emergent conclusions from reasoning. The system cannot decide whether to interrupt the user based on its understanding of their world state.

6. **Human-labeled categories.** The system thinks in "Health," "Learning," "Finance," "Projects," "Tasks," "Calendar," "Reminders" — human labels that obscure the underlying unity of the user's experience.

### The Vision

Instead of organizing knowledge into rigid domains, the system should build **one continuously evolving World Model** centered on the user and everything connected to the user. This World Model represents everything as entities and relationships in a unified graph. There are no categories — only nodes and edges with properties.

The system should never ask "what module does this belong to?" It should instead ask "what new understanding about the user's world did I just gain?" Everything updates the same World Model.

### Relationship to Existing Architecture

This ADR does not replace the existing layered architecture (Core, Runtime, Memory, Brain, Perception, Execution, Intelligence, OSAL). Those layers provide essential infrastructure: the EventBus for communication, Service lifecycle for component management, Runtime for scheduling and permissions, OSAL for system abstraction, and the Intelligence layer for model integration.

What changes is **the philosophy and purpose** of each layer:

- **Memory** is no longer a 4-tier store of memory objects. It is the World Model — a persistent graph of entities and relationships that captures the system's understanding of the user's world.
- **Perception** no longer produces observations for the Brain to consume. It extracts entities and relationships from raw input and updates the World Model.
- **Brain** no longer manages goals, plans, and decisions as separate concerns. It reasons over the World Model to generate understanding, make decisions, and drive action.
- **Execution** no longer dispatches tools for the Brain. It carries out actions that the reasoning system determined are needed, without knowing why.
- **Context** is no longer session-scoped metadata. It is a dynamic subgraph of the World Model, generated on demand from relevance to the current situation.

The infrastructure crates (EventBus, Service, LifecycleManager, Scheduler, PermissionChecker, etc.) remain unchanged. What changes is how the higher-layer crates use them.

### Alternatives Considered

1. **Keep the current module-centric architecture.** Rejected because it perpetuates the problem of knowledge silos and rigid categories. The system would remain a collection of features rather than a unified intelligence.

2. **Replace all layers with a single graph store.** Rejected because it discards valuable engineering infrastructure (event bus, lifecycle, permissions, scheduling) and creates a monolithic system. The layering provides essential separation of concerns.

3. **Add a "meta" layer on top that maps between domains.** Rejected because it adds complexity without solving the root problem. The system would still think in categories internally, then translate to a unified view externally.

4. **Incremental adoption without architectural change.** Rejected because the architectural philosophy drives implementation decisions. Without a clear architectural commitment, individual components will continue to be built with category-centric assumptions.

## Decision

We will adopt a **World Model architecture** as the organizing philosophy for AI-native OS, effective immediately for all future design and implementation work.

### Core Decisions

1. **Everything is an entity or a relationship.** The World Model is a property graph with two fundamental types: `Entity` (nodes) and `Relationship` (edges). No other fundamental types exist. Every concept in the user's world — a person, a project, a task, a conversation, a device, a habit, a skill, an idea, a decision, a document, a place — is an Entity. Every connection between entities is a Relationship.

2. **No fixed business-specific categories.** The entity type system is open. Types are strings, not enum variants. The system does not define `Health`, `Finance`, `Work`, or `Project` as distinct categories. It defines generic entities that can be connected in any way the user's world requires.

3. **Memory IS the World Model.** The Memory Platform crates (memory-core, memory-storage, memory-knowledge, etc.) implement the World Model store. The 4-tier memory hierarchy (Working, Episodic, Semantic, Knowledge) becomes the physical storage strategy for the graph. "Memory objects" are entities. "Relationships" connect entities. The knowledge graph IS the primary representation.

4. **Perception updates the World Model.** Every observation from every interface (desktop, voice, WhatsApp, email, browser, GitHub, IoT, sensors, manual input) goes through a pipeline that extracts entities and relationships and updates the World Model. Observations themselves may be stored as entities (ephemeral, with a lifecycle).

5. **Brain reasons over the World Model.** The Brain does not manage separate "goals" and "plans." It queries the World Model to understand the current state, detects changes and patterns, and generates decisions about what actions to take. Goals are entities with a `Goal` type. Plans are entities with a `Plan` type, related to goals via `addresses` relationships.

6. **Context is a dynamic subgraph.** Given the current situation (time, location, recent entities, active entities), the system generates context by traversing the World Model graph outward from relevant entities. Context depth is bounded by relevance scores, not fixed windows.

7. **Notifications emerge from reasoning.** The Brain does not schedule notifications. It continuously evaluates the World Model for situations that warrant the user's attention: blocked projects, relationship changes, routine interruptions, opportunities, approaching deadlines, unfulfilled promises. When a situation exceeds an interruption threshold, a notification is emitted.

8. **Planning is continuous reprioritization.** There is no separate "planning phase." Every reasoning cycle evaluates current state, recent observations, user behavior, new information, and unexpected events to determine what should happen next.

9. **Reasoning asks World Model questions.** Instead of "what goal should I pursue?", the reasoning loop asks: "What changed? What relationships changed? What goals are affected? What opportunities exist? What risks exist? What should be remembered? What should be forgotten? What should be suggested? Should I wait? Should I ask? Should I act?"

### Layer Mapping

The existing layers are repurposed as follows, without changing their infrastructure code:

| Layer | Old Philosophy | New Philosophy |
|---|---|---|
| OSAL | System abstraction for Linux | Unchanged — provides system observation and action primitives |
| Core | EventBus, Service, Lifecycle, Config | Unchanged — infrastructure remains the same |
| Runtime | Task/session/permission management | Unchanged — scheduling, permissions, context propagation remain |
| Memory | 4-tier memory store | World Model graph store (entities + relationships persisted across tiers) |
| Perception | Observation pipeline → Brain | Observation → Entity/Relationship extraction → World Model update |
| Brain | Goal/Plan/Decision management | World Model reasoning engine (query, detect, decide, act) |
| Execution | Tool dispatch | Action executor (receives decisions, executes, reports results) |
| Intelligence | Model provider abstraction | Natural language understanding/generation for entity extraction and reasoning |

### Entity Model

```rust
pub struct Entity {
    pub id: EntityId,
    pub entity_type: String,        // "person", "project", "task", "device", "habit", ...
    pub name: String,
    pub properties: HashMap<String, Value>,
    pub importance: f32,            // 0.0 - 1.0, computed from relationship density and usage
    pub confidence: f32,            // 0.0 - 1.0, how certain the system is this entity exists
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub version: u64,
    pub lifecycle: EntityLifecycle, // Active, Archived, Forgotten
}
```

Every entity supports: creation, evolution, relationships, history, metadata, confidence, importance, lifecycle.

### Relationship Model

```rust
pub struct Relationship {
    pub id: RelationshipId,
    pub relationship_type: String,  // "owns", "depends_on", "contains", "creates", ...
    pub source_id: EntityId,
    pub target_id: EntityId,
    pub properties: HashMap<String, Value>,
    pub confidence: f32,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub version: u64,
}
```

Relationships are directed edges in the property graph. Bidirectional relationships are represented as two directed relationships (or a single relationship with `is_bidirectional: true`).

### Context as Subgraph

Context is generated dynamically by:
1. Identifying the focal entity set (what the user is currently doing, where they are, what time it is)
2. Traversing relationships outward to depth N (default: 3)
3. Scoring each reachable entity by: relationship distance, importance, recency of interaction, confidence
4. Returning the top-K scored entities as the current context

Context has no fixed size. It is always the most relevant connected entities for the current situation.

### Permission Model

The World Model belongs entirely to the user. The user controls:
- What entities and relationships exist
- What is remembered vs forgotten
- What is observed
- What is shared (with which agents/services)
- What can be automated

Permissions are evaluated per entity and per relationship type. Default deny.

## Consequences

### Positive

- **Unified understanding.** Every piece of information about the user's world lives in one graph. The system never has to ask "which module has this data?".
- **Emergent intelligence.** Patterns, connections, and insights emerge naturally from graph structure rather than requiring predefined category-specific logic.
- **No rigid categories.** The system can represent anything the user encounters without needing new types, modules, or schema changes.
- **Dynamic context.** Context adapts to the current situation automatically rather than relying on fixed windows or session boundaries.
- **Natural notifications.** Notifications emerge from reasoning about the world state rather than from timers or triggers.
- **Future-proof.** New types of entities and relationships can be added without architectural changes.
- **All existing infrastructure is preserved.** The EventBus, Service lifecycle, permissions, scheduling, and all other Core/Runtime infrastructure remains unchanged.

### Negative

- **Query complexity.** Graph traversal is more complex than category-filtered queries. Retrieval performance depends on graph topology and index quality.
- **Entity resolution is harder.** Without predefined categories, the system must infer entity types from context. This places more demands on the Perception layer's entity extraction.
- **Reasoning is more open-ended.** Without predefined goals and plans, the reasoning loop has a larger search space. Cognitive budgets must be managed carefully.
- **Migration cost.** Existing module-centric designs (goals, plans, memory tiers) must be reframed as entity/relationship patterns within the World Model.
- **Debugging complexity.** A unified graph can be harder to debug than isolated stores when something goes wrong.
- **Confidence tracking becomes critical.** Without category boundaries, the system must track confidence at the entity and relationship level to avoid spreading misinformation through graph traversal.

## Compliance

1. All new Memory Platform code must operate on entities and relationships, not on domain-specific memory objects.
2. All new Perception Platform code must extract entities and relationships, not produce observations for the Brain.
3. All new Brain Platform code must reason over the World Model graph, not maintain separate goal/plan/decision stores.
4. No new crate or module may introduce a fixed category enum (Health, Finance, Work, etc.).
5. Entity types must be strings, not enum variants.
6. All interfaces must be generic over entity/relationship types.
7. Code review must reject any new struct that represents a domain-specific concept without using the entity/relationship model.

## Notes

- This ADR supersedes the module-centric philosophy of previous designs. Existing implementation crates (core, runtime) are not affected.
- Phase 5+ implementation work (Memory, Brain, Perception, Execution, Intelligence) will be done under this philosophy.
- RFCs 0002-0006 will be updated to reflect this philosophy in follow-up revisions.
- The existing architecture docs (memory.md, brain.md, perception.md) retain their infrastructure descriptions but should be read alongside this ADR for philosophical context.

## References

- [ADR-0001: Project Vision](./0001-project-vision.md)
- [ADR-0002: Clean Architecture](./0002-clean-architecture.md)
- [ADR-0004: Event-Driven Design](./0004-event-driven.md)
- [RFC-0002: Memory Platform](../rfc/RFC-0002-memory-platform.md)
- [RFC-0003: Brain Platform](../rfc/RFC-0003-brain-platform.md)
- [RFC-0004: Perception Platform](../rfc/RFC-0004-perception-platform.md)
- [RFC-0005: Execution Platform](../rfc/RFC-0005-execution-platform.md)
- [RFC-0006: Intelligence Platform](../rfc/RFC-0006-intelligence-platform.md)
- [Architecture: Memory](../architecture/memory.md)
- [Architecture: Brain](../architecture/brain.md)
- [Architecture: Perception](../architecture/perception.md)
- [Architecture: World Model](../architecture/world-model.md)
