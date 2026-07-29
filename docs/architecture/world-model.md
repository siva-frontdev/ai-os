# World Model Architecture

## Purpose

The World Model is the central intelligence primitive of AI-native OS. It is a continuously evolving property graph of everything the system understands about the user and the user's world. There are no categories, no modules, no domains. There are only entities, relationships, and the reasoning that emerges from their connections.

This document describes the architecture of the World Model: its data model, how it is built, how it is queried, how it evolves, and how every layer of the platform participates in maintaining and reasoning over it.

---

## Philosophy

### One Model, No Categories

The AI must never organize knowledge into rigid domains: Health, Learning, Finance, Work, Projects, Relationships, Calendar, Tasks, Reminders. These are human labels that obscure the underlying unity of experience.

Instead, the system builds one continuously evolving World Model. Every observation, every interaction, every piece of information updates the same graph. The system never asks "what module does this belong to?" It asks "what new understanding about the user's world did I just gain?"

### Entities and Relationships

Everything is either an entity (a node) or a relationship (an edge). There are no other fundamental types.

- **Entities** are things: people, organizations, projects, goals, tasks, conversations, memories, observations, ideas, decisions, skills, knowledge, interests, habits, routines, events, meetings, documents, websites, applications, devices, places, objects, services, notifications, actions, capabilities, questions, answers, states, contexts.
- **Relationships** are connections: owns, depends_on, contains, creates, produces, belongs_to, supports, affects, participates_in, attends, reads, writes, modifies, uses, manages, leads, contributes_to, blocks, unblocks, precedes, follows, causes, influences, relates_to.

### Everything Updates the Same Model

Every interface — desktop, voice, WhatsApp, email, browser, calendar, GitHub, documents, IoT, sensors, manual user input — contributes observations. Every observation goes through the same pipeline: entity extraction, relationship extraction, graph update. There is one World Model.

---

## Data Model

### Entity

```rust
pub struct Entity {
    pub id: EntityId,                            // UUID v7
    pub entity_type: String,                     // "person", "project", "task", ...
    pub name: String,                            // Human-readable label
    pub properties: HashMap<String, Value>,      // Type-specific attributes
    pub importance: f32,                         // 0.0 - 1.0, computed from graph centrality
    pub confidence: f32,                         // 0.0 - 1.0, certainty of existence/accuracy
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub version: u64,                            // Monotonic version for history
    pub lifecycle: EntityLifecycle,              // Active | Archived | Forgotten
    pub metadata: HashMap<String, String>,       // Extensible key-value metadata
}
```

#### Entity Lifecycle

| State | Meaning | Transitions |
|---|---|---|
| `Active` | Entity is present in the World Model and used in reasoning | → Archived, → Forgotten |
| `Archived` | Entity is preserved but excluded from active reasoning | → Active (if re-observed), → Forgotten |
| `Forgotten` | Entity is removed (user request or low confidence decay) | None (can be re-created if re-observed) |

#### Properties

Properties are typed key-value pairs that capture entity-specific attributes. The type system is open:

```rust
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Timestamp(Timestamp),
    EntityId(EntityId),          // Reference to another entity
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
    Null,
}
```

Examples:
- Person: `{ "email": "...", "phone": "...", "timezone": "..." }`
- Project: `{ "status": "active", "deadline": "...", "repository": "..." }`
- Task: `{ "status": "pending", "priority": 3, "estimated_hours": 5.0 }`
- Device: `{ "os": "macOS", "model": "MacBook Pro", "ip_address": "..." }`

### Relationship

```rust
pub struct Relationship {
    pub id: RelationshipId,
    pub relationship_type: String,       // "owns", "depends_on", "contains", ...
    pub source_id: EntityId,
    pub target_id: EntityId,
    pub properties: HashMap<String, Value>,
    pub confidence: f32,
    pub weight: f32,                     // 0.0 - 1.0, strength of the connection
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub version: u64,
    pub lifecycle: RelationshipLifecycle,
}
```

#### Relationship Direction

Relationships are directed by default. The convention for naming is:
- Active voice: `Person` —`owns`→ `Device`
- Source to target: `Project` —`depends_on`→ `Project`
- Inverse is queried explicitly: "what owns this device?" traverses incoming `owns` edges

#### Common Relationship Types

| Type | Source → Target | Example |
|---|---|---|
| `owns` | Person → Device/ Document/ Application | User owns MacBook Pro |
| `depends_on` | Project → Project/ Task/ Skill | AI-OS depends on Rust |
| `contains` | Project → Task/ Document | Project contains task |
| `creates` | Conversation/ Meeting → Idea/ Decision | Meeting produces decision |
| `produces` | Meeting → Decision | Meeting produces decision |
| `belongs_to` | Document/ Website → Organization/ Person | Website belongs to company |
| `supports` | Routine/ Skill/ Habit → Goal | Exercise supports health goal |
| `affects` | Entity → Entity (state change) | Health affects Energy |
| `participates_in` | Person → Meeting/ Conversation/ Project | User participates in standup |
| `attends` | Person → Event/ Meeting | User attends conference |
| `uses` | Person → Tool/ Application/ Device | User uses VS Code |
| `leads` | Person → Project/ Team/ Organization | Alice leads AI-OS |
| `blocks` | Entity → Entity | Dependency blocks task |
| `unblocks` | Entity → Entity | New hire unblocks project |
| `precedes` | Event → Event | Standup precedes retro |
| `follows` | Event → Event | Retro follows standup |
| `causes` | Event/ Action → Event/ State | Deployment causes outage |
| `influences` | Entity → Decision/ Opinion | Article influences decision |
| `relates_to` | Entity → Entity (generic) | Broadband relates_to ISP |

---

## World Model Store

The World Model is physically stored in the Memory Platform's existing infrastructure. The Memory Platform crates are repurposed as follows:

| Memory Crate | World Model Role |
|---|---|
| `memory-core` | Entity, Relationship, and graph primitive types |
| `memory-storage` | `WorldModelStore` trait — CRUD for entities and relationships |
| `memory-index` | Entity index (type, name, property search), relationship index |
| `memory-working` | Hot cache for active entities (frequently traversed subgraph) |
| `memory-episodic` | Temporal entity index — entities ordered by creation/observation time |
| `memory-semantic` | Fact store — entity properties with confidence and provenance |
| `memory-knowledge` | Graph engine — adjacency list, traversal, path finding, inference |
| `memory-context` | Context generation — dynamic subgraph extraction |
| `memory-cache` | Entity and relationship result cache |
| `memory-retrieval` | Unified retrieval — hybrid entity search (embedding + keyword + graph) |
| `memory-snapshot` | World Model checkpoint and restore |
| `memory-learning` | Entity evolution, importance computation, consolidation, pruning |

### WorldModelStore Trait

```rust
#[async_trait]
pub trait WorldModelStore: Debug + Send + Sync {
    // Entity operations
    async fn insert_entity(&self, entity: Entity) -> Result<EntityId, WorldModelError>;
    async fn get_entity(&self, id: EntityId) -> Result<Option<Entity>, WorldModelError>;
    async fn update_entity(&self, entity: Entity) -> Result<(), WorldModelError>;
    async fn delete_entity(&self, id: EntityId) -> Result<(), WorldModelError>;
    async fn search_entities(&self, query: EntityQuery) -> Result<Vec<Entity>, WorldModelError>;

    // Relationship operations
    async fn insert_relationship(&self, rel: Relationship) -> Result<RelationshipId, WorldModelError>;
    async fn get_relationship(&self, id: RelationshipId) -> Result<Option<Relationship>, WorldModelError>;
    async fn delete_relationship(&self, id: RelationshipId) -> Result<(), WorldModelError>;
    async fn get_relationships(&self, entity_id: EntityId, direction: Direction) -> Result<Vec<Relationship>, WorldModelError>;

    // Graph operations
    async fn traverse(&self, start: EntityId, query: TraversalQuery) -> Result<Subgraph, WorldModelError>;
    async fn shortest_path(&self, from: EntityId, to: EntityId, max_depth: u8) -> Result<Vec<Path>, WorldModelError>;
    async fn subgraph(&self, seed: Vec<EntityId>, depth: u8, filter: Option<SubgraphFilter>) -> Result<Subgraph, WorldModelError>;
}
```

### Graph Engine (memory-knowledge)

The graph engine provides efficient adjacency list storage and traversal:

```rust
#[async_trait]
pub trait GraphEngine: Debug + Send + Sync {
    async fn get_neighbors(&self, entity_id: EntityId, direction: Direction, rel_types: Option<&[&str]>) -> Result<Vec<(Relationship, Entity)>, WorldModelError>;
    async fn get_degree(&self, entity_id: EntityId, direction: Direction) -> Result<usize, WorldModelError>;
    async fn traverse_bfs(&self, start: EntityId, query: BfsQuery) -> Result<Vec<(Entity, Vec<Relationship>, u8)>, WorldModelError>;
    async fn traverse_dfs(&self, start: EntityId, query: DfsQuery) -> Result<Vec<(Entity, Vec<Relationship>)>, WorldModelError>;
    async fn infer_relationships(&self, rules: &[InferenceRule]) -> Result<Vec<Relationship>, WorldModelError>;
}
```

---

## Observation Pipeline

Every observation from any interface goes through the same pipeline:

```
Raw Input → Entity Extraction → Relationship Extraction → Conflict Resolution → World Model Update → Events
```

### 1. Raw Input

Input arrives from any interface: desktop agent, voice, WhatsApp, email, browser extension, calendar, GitHub webhook, IoT sensor, manual user input, etc. The input is packaged as an `Observation` (reusing the existing `perception-core` type).

### 2. Entity Extraction

The Perception layer's entity extractor identifies entities in the observation:

- **Named entities:** People, organizations, places, products — extracted via pattern matching and model-based NER
- **Concept entities:** Projects, goals, ideas, decisions — inferred from context and conversation structure
- **Existing entities:** Resolved against the World Model to detect if this is a new occurrence of a known entity

Output: `Vec<ExtractedEntity>` with confidence scores.

### 3. Relationship Extraction

From the observation content and extracted entities, relationships are inferred:

- Explicit relationships: "I use VS Code" → `User` —`uses`→ `VSCode`
- Implicit relationships: Mention of a project in a calendar event → `Meeting` —`relates_to`→ `Project`
- Temporal relationships: Sequence of observations → `Observation` —`precedes`→ `Observation`

Output: `Vec<ExtractedRelationship>` with confidence scores.

### 4. Conflict Resolution

New information may conflict with existing World Model state:

- **Same entity, different properties:** Merge with confidence-weighted resolution (higher confidence wins, or ask user)
- **Contradictory relationships:** Flag for reasoning system to evaluate
- **Duplicate entities:** Resolve via entity resolution (same name, same context → same entity)

### 5. World Model Update

The resolved entities and relationships are written to the World Model store:

- New entities are inserted
- Existing entities are updated (version incremented, properties merged)
- New relationships are inserted
- Existing relationships are updated or confirmed (confidence increases)

### 6. Events

Update events are published on the EventBus:

| Event | Payload | Trigger |
|---|---|---|
| `wm.entity.created` | `{ entity_id, entity_type, name }` | New entity inserted |
| `wm.entity.updated` | `{ entity_id, changed_properties }` | Entity properties changed |
| `wm.entity.archived` | `{ entity_id }` | Entity lifecycle → Archived |
| `wm.entity.forgotten` | `{ entity_id }` | Entity lifecycle → Forgotten |
| `wm.relationship.created` | `{ rel_id, type, source, target }` | New relationship inserted |
| `wm.relationship.updated` | `{ rel_id, changed_properties }` | Relationship updated |
| `wm.relationship.deleted` | `{ rel_id }` | Relationship removed |
| `wm.subgraph.changed` | `{ seed_entity, affected_entities }` | Significant structural change detected |

These events trigger the reasoning system to evaluate whether the change matters.

---

## Context Generation

Context is never fixed. It is always a dynamic subgraph of the World Model, generated on demand from the current situation.

### Context Seeds

The context generator starts with a set of **seed entities** determined by the current situation:

- **Time-based:** Entities associated with the current time (calendar events, routines, deadlines)
- **Location-based:** Entities associated with the current location (place, nearby devices, local services)
- **Activity-based:** Entities related to what the user is currently doing (active application, open documents, recent commands)
- **Attention-based:** Entities the user has recently interacted with or mentioned

### Traversal

From the seed entities, the generator traverses the graph outward:

```rust
pub struct ContextRequest {
    pub seeds: Vec<EntityId>,
    pub max_depth: u8,                    // Default: 3
    pub max_entities: usize,              // Default: 50
    pub min_importance: f32,              // Default: 0.3
    pub relationship_filter: Option<Vec<String>>,
    pub include_reasoning: bool,          // Include why each entity is relevant
}

pub struct Context {
    pub entities: Vec<ContextEntity>,     // Entity + relevance score + path from seed
    pub relationships: Vec<Relationship>,
    pub generated_at: Timestamp,
    pub seeds: Vec<EntityId>,
}
```

### Scoring

Each reachable entity is scored:

```
relevance = f(importance, inverse_distance, recency, confidence)

where:
  importance = entity's global importance (graph centrality)
  inverse_distance = 1.0 / (path_length + 1)  — closer entities are more relevant
  recency = how recently the entity was observed/interacted with
  confidence = entity's confidence score
```

Entities below `min_importance` or `min_confidence` are excluded. Top `max_entities` are returned.

### Context Evolution

Context is continuously regenerated as the situation changes. The reasoning system subscribes to `wm.*` events and regenerates context when relevant entities change.

---

## Reasoning Loop

The reasoning loop (Brain Platform) operates over the World Model. It has no separate goal manager, plan manager, or decision store. Goals and plans are entities in the graph.

### Loop Structure

```
┌──────────────────────────────────────────────────────┐
│                    Reasoning Loop                     │
│                                                       │
│  1. Update World Model from recent observations       │
│  2. Generate Context (dynamic subgraph)               │
│  3. Detect Changes (what's different since last tick) │
│  4. Evaluate State (what does the current state mean) │
│  5. Identify Opportunities & Risks                   │
│  6. Decide Actions (what to do, ask, or wait)        │
│  7. Execute Actions (via Execution layer)            │
│  8. Observe Results (feedback into World Model)      │
│  9. Update Entity Importance & Confidence            │
│  10. Consolidate & Prune (forgetting, archiving)     │
└──────────────────────────────────────────────────────┘
```

### Questions the Loop Asks

Instead of "what goal should I pursue?", the loop asks:

**What changed?**
- New entities appeared? New relationships? Properties changed?
- What entities were activated, archived, or forgotten?

**What relationships changed?**
- New connections formed? Existing ones strengthened or weakened?
- Are there patterns in relationship changes?

**What goals are affected?**
- Find entities of type `goal` that are connected to changed entities
- Determine if the change advances, blocks, or deprioritizes the goal

**What opportunities exist?**
- Are there entities with high potential value that have low current attention?
- Are there connections that could be formed?

**What risks exist?**
- Are there blocked paths? Overdue deadlines? Unresolved conflicts?
- Declining confidence in important entities?

**What should be remembered?**
- High-importance entities nearing archival threshold
- Patterns observed across multiple observations

**What should be forgotten?**
- Low-confidence, low-importance entities past TTL
- User-requested forgetting

**What should be suggested?**
- Opportunities that match user patterns
- Connections the user might not have made

**Should I wait?**
- Is the current state stable? Are there pending observations?
- Would waiting provide more information?

**Should I ask?**
- Is confidence too low to act? Is user input needed for disambiguation?
- Is the decision high-stakes enough to warrant interruption?

**Should I act?**
- Is there a clear, high-confidence action that improves the user's world state?
- Is the action within automated permission scope?

### Cognitive Budget

Each reasoning cycle has a budget (reusing `brain-core`'s `CognitiveBudget`):

| Dimension | Default Limit | Effect When Exhausted |
|---|---|---|
| Max traversal depth | 1000 nodes | Halve depth, retry |
| Max model calls | 5 per cycle | Degrade to rule-based reasoning |
| Max reasoning time | 500ms | Return best-effort result |
| Max decisions | 3 per cycle | Queue remaining for next cycle |
| Max notifications | 1 per cycle | Suppress, evaluate next cycle |

---

## Notification Philosophy

Notifications are not scheduled events. They emerge naturally from the reasoning loop.

### When to Notify

The reasoning loop evaluates whether the user should be interrupted:

| Situation | Interruption Level | Example |
|---|---|---|
| A project is blocked | High | A dependency failed, blocking a critical-path task |
| A relationship needs attention | Medium | No interaction with a key contact in 30 days |
| A routine was interrupted | Medium | Daily backup failed 3 times |
| An opportunity appeared | Low | A relevant job posting or conference was found |
| A deadline is approaching | Medium | Project deadline in 48 hours, significant work remains |
| A promise has not been fulfilled | Variable | Response was promised but not yet sent |
| Confidence dropped significantly | High | Previously confident entity now uncertain |
| Anomaly detected | High | Unusual pattern in user's behavior or system state |

### Interruption Levels

| Level | Behavior |
|---|---|
| `Suppressed` | Logged for next context generation; no user-facing notification |
| `Low` | Added to next context summary; no immediate interruption |
| `Medium` | Notification displayed but non-blocking (toast, indicator badge) |
| `High` | User is interrupted with full context and suggested action |
| `Critical` | User is interrupted with escalation priority (emergency override) |

### Learning Notification Timing

The system learns when the user prefers to be interrupted:
- Time of day patterns (not during focus hours, okay after lunch)
- Activity patterns (not during deep work, okay during context switches)
- Response patterns (user responded quickly to previous notification → similar situations are appropriate)

---

## Planning as Continuous Reprioritization

There is no "planning phase." Planning is the continuous reprioritization of what the reasoning loop should focus on.

### What Drives Prioritization

| Signal | How It Affects Priority |
|---|---|
| Current world state | Entities with approaching deadlines or blockers get higher priority |
| Recent observations | New information triggers re-evaluation of affected entities |
| User behavior | Entity the user is actively engaged with gets priority |
| New information | Unusual or unexpected observations get immediate evaluation |
| Unexpected events | Anomalies bypass normal prioritization for immediate assessment |
| Relationship decay | Entities with decaying relationship freshness get maintenance priority |

### Goal Entities

Goals are entities like any other. They have:
- `entity_type: "goal"`
- Properties: `status`, `deadline`, `target_state`, `progress`
- Relationships: `addresses` (what problem it solves), `depends_on` (prerequisites), `contains` (sub-goals)

The reasoning loop does not "manage goals." It finds goal entities in the graph, evaluates their status, and determines if action is needed.

### Plan Entities

Plans are also entities:
- `entity_type: "plan"`
- Properties: `status`, `steps`, `estimated_duration`
- Relationships: `addresses` (which entity it addresses), `produces` (expected outcomes)

---

## Execution

Execution is a pure consumer. It receives actions from the reasoning loop and executes them.

### Execution Principle

Execution never knows why something is happening. It only knows:
- Requested action
- Current state
- Constraints
- Verification criteria

### Action Entity

Actions submitted to execution are entities:

```rust
pub struct Action {
    pub id: EntityId,
    pub entity_type: "action",
    pub properties: {
        "action_type": String,         // "send_message", "create_file", "schedule_event"
        "parameters": HashMap<String, Value>,
        "constraints": Vec<Constraint>,
        "verification": VerificationCriteria,
    },
    pub relationships: [
        { type: "produced_by", target: Decision entity },
        { type: "affects", target: affected entity },
    ],
}
```

### Execution Pipeline

```
Decision (from reasoning) → Action entity created in World Model
  → Execution layer detects Action entity
  → Validates against permissions
  → Executes the action (tool dispatch, system call, API call)
  → Result recorded as entity update or new Observation
  → World Model updated with outcome
  → Reasoning loop triggered for outcome evaluation
```

---

## Learning

The system learns only one thing: how the user's world evolves. Learning improves:

| Capability | Improvement Mechanism |
|---|---|
| Predictions | Entity state transition probabilities learned from history |
| Recommendations | Relationship patterns — entities that co-occur or are linked |
| Timing | User responsiveness patterns by time, activity, and topic |
| Communication | Language patterns, preferred notification style |
| Reasoning | Inference rule effectiveness tracked and tuned |
| Prioritization | Which entity types and relationships the user cares about most |
| Trust | Accuracy of past predictions vs outcomes fed as confidence adjustments |

### Entity Importance

Importance is computed from graph structure and usage patterns:

```
importance = f(
    degree_centrality,          // entities with more connections are more important
    recency_of_interaction,     // recently used entities are more important
    frequency_of_access,        // frequently referenced entities are more important
    user_explicit_rating,       // user can explicitly mark importance
    relationship_decay,         // importance decays without interaction
    propagation_from_important_neighbors  // connected to important entities → more important
)
```

### Confidence Decay

Entity and relationship confidence decays over time without re-observation:

```
confidence(t) = initial_confidence * e^(-decay_rate * t)

where:
  decay_rate depends on entity_type and observation frequency
  - Frequently observed types (applications, devices): slow decay
  - Rarely observed types (contacts, projects): faster decay
```

When confidence drops below `forget_threshold` (default: 0.1), the entity is a candidate for forgetting.

---

## Memory as Understanding

The World Model does not store conversations. It stores understanding.

### Example

**Instead of storing:**
> The user said they bought a MacBook Pro.

**The World Model stores:**

```
Entity: MacBook Pro
  entity_type: "device"
  properties:
    owner: "User"
    purpose: "Development"
    purchase_date: "2026-07-25"
    model: "MacBook Pro 16-inch"
  relationships:
    - type: "owned_by" → User
    - type: "used_for" → Project("AI-OS")
    - type: "replaces" → Device("Old Laptop")
  importance: 0.75
  confidence: 0.92
```

### Conversation Entities

Conversations themselves are entities:

```
Entity: Conversation-2026-07-27
  entity_type: "conversation"
  properties:
    channel: "voice"
    duration_seconds: 183
    summary: "Discussed MacBook purchase for AI-OS development"
  relationships:
    - type: "involves" → User
    - type: "involves" → Person("Sales Rep")
    - type: "produced" → Decision("Buy MacBook Pro")
    - type: "references" → Device("MacBook Pro")
    - type: "references" → Project("AI-OS")
  importance: 0.4
  confidence: 0.95
```

The conversation content is not stored verbatim (unless configured). Only the entities, relationships, and understanding extracted from the conversation persist.

---

## Privacy

The World Model belongs entirely to the user.

### User Controls

| Control | Mechanism |
|---|---|
| What exists | User can view all entities and relationships in the graph |
| What is remembered | User can promote entities to persistent or demote to archival |
| What is forgotten | User can delete entities/relationships; forget is absolute |
| What is observed | User controls which observation sources are active |
| What is shared | Per-entity and per-relationship sharing permissions |
| What can be automated | Permission scope for automated actions |

### Permission Model

The existing Runtime PermissionChecker is used, extended with entity-level permissions:

```rust
pub struct EntityPermission {
    pub entity_id: EntityId,
    pub permission: Permission,
    pub grantee: String,           // agent or user identifier
}
```

Default deny. The user explicitly grants:
- Read access to specific entity types
- Write access to specific entity relationships
- Execute access for actions affecting specific entities

---

## Evolution Strategy

The World Model evolves in three dimensions:

### 1. Entity Evolution

Entities change over time:
- **Properties** are added, updated, or removed
- **Importance** is recomputed periodically and on structural changes
- **Confidence** decays and is reinforced by re-observation
- **Lifecycle** transitions from Active → Archived → Forgotten

### 2. Relationship Evolution

Relationships also change:
- **Weight** strengthens with repeated co-occurrence
- **Confidence** increases with evidence, decays without
- **New relationships** are inferred from patterns (transitive closure, similarity)
- **Old relationships** decay when entities drift apart in usage

### 3. Structural Evolution

The graph structure itself evolves:
- **Entity resolution** merges duplicate entities when evidence suggests they are the same
- **Type inference** adjusts entity types as more context is gathered
- **Cluster detection** identifies natural groupings of entities (without predefining categories)
- **Subgraph extraction** archives low-importance disconnected subgraphs

---

## Incremental Implementation Roadmap

### Phase 0: Design (Current)

- [x] ADR-0006: World Model philosophy decision
- [x] Architecture document (this file)
- [ ] Update RFC-0002 (Memory → World Model store)
- [ ] Update RFC-0003 (Brain → World Model reasoning)
- [ ] Update RFC-0004 (Perception → World Model extraction)
- [ ] Update RFC-0005 (Execution → action as entity)
- [ ] Update RFC-0006 (Intelligence → entity extraction & reasoning)

### Phase 1: World Model Store (Replace Memory Knowledge Tier)

- Implement `Entity` and `Relationship` types in `memory-core`
- Implement `WorldModelStore` trait in `memory-storage`
- Implement `GraphEngine` trait in `memory-knowledge`
- Implement entity searching (type, property, name) in `memory-index`
- Implement basic entity CRUD events

### Phase 2: Entity Extraction (Update Perception)

- Implement entity extraction from observations in `perception-entities`
- Implement relationship extraction in `perception-entities` or new submodule
- Implement conflict resolution for World Model updates
- Wire observation pipeline to World Model store

### Phase 3: Context Generation

- Implement context seed determination (time, location, activity, attention)
- Implement subgraph traversal and scoring
- Implement context-as-subgraph API
- Replace session-scoped context with dynamic subgraph context

### Phase 4: Reasoning Over World Model (Update Brain)

- Implement World Model query primitives in reasoning engine
- Implement change detection (diff between ticks)
- Implement state evaluation (what does current subgraph mean)
- Implement opportunity and risk identification
- Implement action decision (act, ask, wait)

### Phase 5: Notification Emergence

- Implement interruption level classification per situation type
- Implement timing learning (when to notify)
- Implement notification delivery via existing Runtime channels
- Remove timer-based notification scheduling

### Phase 6: Continuous Evolution

- Implement entity importance computation (graph centrality + usage)
- Implement confidence decay and reinforcement
- Implement forgetting (archival, deletion)
- Implement entity resolution (merge duplicates)
- Implement relationship inference (transitive closure, co-occurrence)

### Phase 7: Privacy & Permissions

- Implement entity-level permissions
- Implement World Model audit (user can inspect all state)
- Implement sharing controls
- Implement user-facing graph visualization for review

---

## References

- [ADR-0006: World Model Architecture](../adr/0006-world-model.md)
- [ADR-0001: Project Vision](../adr/0001-project-vision.md)
- [ADR-0004: Event-Driven Design](../adr/0004-event-driven.md)
- [Memory Platform Architecture](memory.md) — Physical storage infrastructure
- [Brain Platform Architecture](brain.md) — Reasoning loop infrastructure
- [Perception Platform Architecture](perception.md) — Observation pipeline infrastructure
- [Runtime Platform Architecture](runtime.md) — Scheduling, permissions, execution
- [RFC-0002: Memory Platform](../rfc/RFC-0002-memory-platform.md)
- [RFC-0003: Brain Platform](../rfc/RFC-0003-brain-platform.md)
- [RFC-0004: Perception Platform](../rfc/RFC-0004-perception-platform.md)
- [RFC-0005: Execution Platform](../rfc/RFC-0005-execution-platform.md)
- [RFC-0006: Intelligence Platform](../rfc/RFC-0006-intelligence-platform.md)
