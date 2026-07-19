# Memory Platform Architecture

## Purpose

The Memory Platform (Phase 5) provides the AI operating system's cognitive memory subsystem — a unified, hierarchical memory store spanning in-process working memory through persistent long-term storage, with hybrid search indexing, event-driven learning, and snapshot/restore capability. It is the substrate on which the Brain, Perception, and Execution layers store and recall state.

Unlike conventional key-value stores or vector databases, the Memory Platform implements a **cognitive memory hierarchy** modeled on human memory systems: working memory (volatile, bounded, fast), episodic memory (event sequences with temporal context), semantic memory (generalized facts and concepts), and a knowledge graph (structured relationships). Each tier has distinct performance, durability, and capacity characteristics.

The Memory Platform depends on [Runtime](runtime.md) for task scheduling and context propagation and on [OSAL](osal.md) for filesystem I/O. It depends on no other AI-native OS layer beyond Runtime and Core.

---

## Design Principles

1. **Cognitive Hierarchy** — Four memory tiers (Working, Episodic, Semantic, Knowledge) with automatic consolidation pathways. Knowledge is derived from semantic memory; semantic memory is derived from episodic memory; episodic memory is consolidated from working memory.

2. **Backend-Agnostic Storage** — No Memory Platform crate depends on a specific database. All persistence goes through the `MemoryStore` trait. Initial backends: in-memory, SQLite. Future: PostgreSQL, RocksDB, object storage.

3. **Async-First, Lock-Minimal** — All public operations are async. Shared state uses `RwLock` with minimal scopes. Background tasks (consolidation, pruning, pattern discovery) run on Tokio intervals.

4. **Event-Driven Learning** — The LearningManager observes memory access patterns and system events to discover temporal, sequential, associative, and causal patterns autonomously.

5. **Immutable Memory Records** — Memory objects are append-only once created. Updates create new versions. The version chain provides an audit trail.

6. **Session-Isolated by Default** — Memory objects are scoped to their originating context. Cross-context access requires explicit capability grants.

7. **Four-Score Retrieval** — Hybrid retrieval combines embedding similarity, keyword BM25, knowledge graph traversal, and recency/importance scoring. Each contributor is independently weighted and explainable.

---

## Crate Structure

All Memory Platform crates live under `crates/` in the workspace:

```
crates/
├── memory-core/           # Base types, traits, data model
├── memory-storage/        # MemoryStore trait + in-memory backend
├── memory-index/          # MemoryIndex trait + in-memory HNSW
├── memory-working/        # Bounded volatile store with LRU eviction
├── memory-episodic/       # Episodic event sequences
├── memory-semantic/       # Generalized facts and concepts
├── memory-knowledge/      # Structured knowledge graph
├── memory-context/        # Session context management
├── memory-cache/          # Hot cache layer
├── memory-retrieval/      # Hybrid retrieval engine
├── memory-snapshot/       # Snapshot and restore
└── memory-learning/       # Consolidation, pruning, pattern discovery
```

### Dependency Graph

```
memory-core (no deps within memory)
├── memory-storage (core)
├── memory-index (core)
├── memory-context (core)
├── memory-working (core)
│   └── memory-cache (working, core)
├── memory-episodic (core, storage)
├── memory-semantic (core, storage)
├── memory-knowledge (core, storage)
├── memory-retrieval (core, index, storage, cache)
├── memory-snapshot (core, storage, index)
└── memory-learning (core, storage, index, retrieval)
```

Dependencies on other layers:

| Crate | Depends On |
|---|---|
| All memory crates | `ai-os-core` (EventBus, Service, Logger) |
| `memory-storage` | OSAL filesystem (for durable backends) |
| `memory-learning` | Runtime scheduler (background tasks) |
| `memory-context` | Runtime context manager (trace propagation) |
| `memory-retrieval` | Runtime permission checker (access control) |

---

## Data Model

### MemoryObject — universal memory record

Every memory object carries the following fields. The structure follows an append-only, versioned design — updates create new versions rather than mutating in place.

```rust
pub struct MemoryObject {
    // Identity
    pub id: MemoryId,                    // UUID v7 (time-sortable)
    pub version: u64,                    // Monotonic version counter

    // Origin
    pub timestamp: Timestamp,            // UNIX epoch nanoseconds
    pub created_by: String,              // Agent or module identifier
    pub source: MemorySource,            // User, System, Agent, Derived, External

    // Classification
    pub memory_type: MemoryType,         // Working, Episodic, Semantic, Knowledge
    pub tier: MemoryTier,                // Working, Episodic, Semantic, Archive
    pub content_type: String,            // MIME-like type: "text/markdown", "application/json"

    // Value
    pub priority: u8,                    // 0-255 (higher = more important)
    pub importance: f32,                 // 0.0 - 1.0 (computed by learning engine)
    pub confidence: f32,                 // 0.0 - 1.0 (factual certainty)

    // Content
    pub content: Vec<u8>,                // Serialized payload
    pub embedding: Option<Vec<f32>>,     // Pre-computed embedding vector

    // Metadata
    pub tags: Vec<String>,               // Freeform tags for categorization
    pub relationships: Vec<Relationship>, // Links to other memory objects
    pub metadata: HashMap<String, String>, // Extensible key-value metadata

    // Lifecycle
    pub expiration: Option<Timestamp>,   // TTL-based expiration
    pub checksum: [u8; 32],              // SHA-256 of content
}
```

### Relationship — typed link between memory objects

```rust
pub struct Relationship {
    pub target_id: MemoryId,
    pub relation_type: RelationType,
    pub weight: f32,                     // 0.0 - 1.0
}

pub enum RelationType {
    DerivesFrom,    // Object B is derived from object A
    References,     // Object A references object B
    PartOf,         // Object B is part of object A
    Sequence,       // Object B follows object A in time
    Contradicts,    // Object A contradicts object B
    Supports,       // Object A supports object B
    Custom(String), // Extension point
}
```

### MemoryType — the four cognitive tiers

```rust
pub enum MemoryType {
    Working,    // Volatile, bounded, sub-microsecond access
    Episodic,   // Temporal event sequences with context
    Semantic,   // Generalized facts, concepts, abstractions
    Knowledge,  // Structured entities and relations (knowledge graph)
}
```

### MemoryTier — physical storage tier

```rust
pub enum MemoryTier {
    Working,    // In-memory, volatile
    Episodic,   // Fast persistent (SQLite)
    Semantic,   // Durable persistent (SQLite / PostgreSQL)
    Archive,    // Cold storage (compressed / object store)
}
```

### QueryFilter — filtered queries across all stores

```rust
pub struct QueryFilter {
    pub tier: Option<MemoryTier>,
    pub memory_type: Option<MemoryType>,
    pub tags: Option<Vec<String>>,
    pub source: Option<MemorySource>,
    pub created_by: Option<String>,
    pub time_range: Option<Range<Timestamp>>,
    pub priority_min: Option<u8>,
    pub importance_min: Option<f32>,
    pub confidence_min: Option<f32>,
    pub limit: usize,
    pub offset: usize,
    pub sort_by: SortField,
    pub sort_order: SortOrder,
}

pub enum SortField { Timestamp, Priority, Importance, Confidence, Version, AccessCount }
pub enum SortOrder { Ascending, Descending }
```

---

## Crate Responsibilities

### memory-core

Foundation crate. Defines all shared types, traits, errors, and events that other memory crates use. Has zero internal dependencies within the memory workspace.

Public items:
- `MemoryId`, `Timestamp`, `MemorySource`
- `MemoryObject`, `Relationship`, `RelationType`
- `MemoryTier`, `MemoryType`
- `QueryFilter`, `SortField`, `SortOrder`
- `MemoryError` — unified error type with variants for every subsystem
- `MemoryEvent` — unified event enum
- `MemoryResult<T>` — alias for `Result<T, MemoryError>`
- Default implementations for all traits (`DefaultMemoryStore`, `DefaultMemoryIndex`, etc.)

### memory-storage

Persistence abstraction. Defines the `MemoryStore` trait and provides the in-memory reference backend.

Public trait `MemoryStore`:
- CRUD: `insert`, `get`, `update`, `delete`, `exists`
- Batch: `insert_batch`, `get_batch`, `delete_batch`
- Query: `query` (filtered), `count`
- Management: `flush`, `compact`, `stats`, `tier_info`

Reference implementation: `InMemoryStore` — `HashMap<MemoryId, MemoryObject>` behind `RwLock`. Used for testing and lightweight deployments.

### memory-index

Search index abstraction. Defines the `MemoryIndex` trait for embedding-based similarity search and keyword search.

Public trait `MemoryIndex`:
- `index`, `search`, `search_with_filter`, `remove`, `rebuild`
- `stats`, `dimension`, `metric`

Reference implementation: `InMemoryIndex` — flat cosine similarity over a `HashMap<MemoryId, Vec<f32>>`. Linear scan O(n).

### memory-working

Bounded, volatile working memory optimized for speed. Implements LRU eviction with capacity limits. Primary interface for the Brain layer's active context.

Public trait `WorkingMemory`:
- `store`, `recall`, `update`, `forget`
- `search` (keyword), `recent` (last n items)
- `capacity`, `clear`, `stats`

Working memory holds a fixed number of entries (default 1024) with LRU eviction. Evicted entries are emitted as events for the LearningManager to evaluate for consolidation.

### memory-episodic

Stores sequences of events with temporal context. Each episode records what happened, when, for how long, who participated, and where.

Public trait `EpisodicMemory`:
- `record`, `recall`, `recall_by_time`, `recall_by_context`
- `replay` (temporal chain traversal)
- `stats`, `compact`

Episodic events are stored via the `MemoryStore` trait with `MemoryType::Episodic`. The episodic memory provides temporal indexing on top of the base store.

### memory-semantic

Stores generalized facts and concepts that have been abstracted from episodic memories. Each fact is a (subject, predicate, object) triple with confidence and source traceability.

Public trait `SemanticMemory`:
- `store_fact`, `recall_fact`, `query_facts`
- `update_fact`, `retract_fact`
- `confidence` (query fact confidence), `stats`

Semantic facts are stored via `MemoryStore` with `MemoryType::Semantic`. Facts track their originating episodic memory IDs for provenance.

### memory-knowledge

Structured knowledge graph with entities, typed relations, and graph traversal. Supports inference (transitive closure, pattern matching).

Public trait `KnowledgeBase`:
- Entity CRUD: `insert_entity`, `get_entity`, `update_entity`, `delete_entity`
- Relation CRUD: `insert_relation`, `get_relations`, `delete_relation`
- Query: `query` (pattern-based), `traverse` (path traversal)
- Inference: `infer` (rule-based reasoning)
- `stats`

The knowledge graph stores entities and relations via `MemoryStore` with `MemoryType::Knowledge`. Graph traversal is performed in-memory on the loaded adjacency list.

### memory-context

Manages named, hierarchical contexts that scope memory operations. Contexts form a tree (parent-child); child contexts inherit parent values and can override them.

Public trait `ContextManager`:
- `create_context`, `destroy_context`
- `set_value`, `get_value`, `delete_value`
- `snapshot`, `restore`, `merge`
- `active_contexts`, `stats`

Contexts are ephemeral (not persisted) and scoped to sessions. The context manager bridges to Runtime's trace propagation for distributed context.

### memory-cache

Hot cache layer for frequently accessed memory objects. Sits in front of the retrieval path to accelerate repeated access.

Public trait `MemoryCache`:
- `get`, `set`, `remove`
- `invalidate` (pattern-based invalidation)
- `clear`, `stats`

Cache uses a configurable TTL per entry and LRU eviction. Cache misses fall through to the retrieval engine.

### memory-retrieval

Unified retrieval engine combining embedding similarity, keyword search, knowledge graph traversal, and recency/importance scoring.

Public trait `MemoryRetriever`:
- `retrieve`, `retrieve_multi` — primary query interface
- `hybrid_search` — fused embedding + keyword + graph
- `rerank` — reorder results with alternative weights
- `explain` — return retrieval breakdown for interpretability

RetrievalStrategy controls how scores are fused:
- `EmbeddingFirst`: cosine similarity dominates
- `KeywordFirst`: BM25 dominates
- `KnowledgeFirst`: graph traversal dominates
- `Weighted(weights)`: explicit per-contributor weights
- `Semantic`: prefers semantic memory tier
- `Episodic`: prefers episodic memory tier
- `Hybrid`: balanced across all contributors

### memory-snapshot

Checkpoint and restore for the entire memory subsystem. Snapshots capture selected tiers, indices, and context state.

Public trait `SnapshotManager`:
- `create_snapshot`, `restore_snapshot`, `delete_snapshot`
- `list_snapshots`, `snapshot_info`
- `export_snapshot` (to filesystem path), `import_snapshot`
- `schedule_snapshot` (periodic or on-change)

RestoreStrategy determines conflict resolution:
- `Replace`: overwrite current state entirely
- `Merge`: newer timestamps win on conflict
- `MergeKeepExisting`: existing values are preserved

### memory-learning

Background engine for consolidation, pruning, and autonomous pattern discovery. This is the "intelligence" of the memory system — it observes access patterns, discovers regularities, and manages the memory lifecycle autonomously.

Public trait `LearningManager`:
- Consolidation: `consolidate`, `consolidate_batch`, `consolidate_by_policy`, `consolidation_status`
- Pruning: `prune`, `dry_run`, `set_policy`, `get_policy`
- Pattern discovery: `discover_patterns`, `get_pattern`, `apply_pattern`
- Control: `pause`, `resume`, `stats`

**Consolidation** moves objects up the memory hierarchy:
- Working → Episodic: when access_count > threshold or importance > threshold
- Episodic → Semantic: when multiple related episodes support the same fact
- Semantic → Knowledge: when facts form a structured entity-relation pattern

**Pruning** removes objects:
- Expired (TTL reached)
- Low importance / confidence below threshold
- Tier capacity exceeded (lowest-scored entries first)
- Explicit forget requests

**Pattern discovery** identifies:
- Temporal patterns (events that recur at specific times)
- Sequential patterns (event A is frequently followed by event B)
- Associative patterns (events that co-occur)
- Causal patterns (event A causes event B with statistical significance)
- Categorical patterns (events that belong to the same category)

---

## Event Model

All Memory Platform events are dispatched on the Core EventBus as structs implementing the `Event` trait.

### Published Events

| Event | Payload | Source | Trigger |
|-------|---------|--------|---------|
| `memory.object.stored` | `{ id, memory_type, tier, timestamp }` | WorkingMemory/Episodic/Semantic/Knowledge | Object inserted |
| `memory.object.recalled` | `{ id, memory_type, tier, latency_us, cache_hit }` | Retrieval | Object retrieved |
| `memory.object.updated` | `{ id, new_version, timestamp }` | Any store | Object version created |
| `memory.object.deleted` | `{ id, memory_type, tier }` | Any store | Object deleted |
| `memory.consolidated` | `{ id, from_tier, to_tier, new_id }` | LearningManager | Tier promotion completed |
| `memory.consolidation.failed` | `{ id, from_tier, reason }` | LearningManager | Consolidation failed |
| `memory.pruned` | `{ ids_removed, bytes_freed, tier, policy }` | LearningManager | Pruning cycle completed |
| `memory.context.created` | `{ context_id, parent_id, session_id }` | ContextManager | New context created |
| `memory.context.destroyed` | `{ context_id }` | ContextManager | Context destroyed |
| `memory.snapshot.created` | `{ snapshot_id, size_bytes, scope }` | SnapshotManager | Snapshot taken |
| `memory.snapshot.restored` | `{ snapshot_id, timestamp }` | SnapshotManager | Snapshot restored |
| `memory.pattern.discovered` | `{ pattern_id, pattern_type, confidence, support }` | LearningManager | New pattern found |
| `memory.tier.capacity_warning` | `{ tier, usage_pct, current_bytes, max_bytes }` | Storage | Usage exceeds 80% |
| `memory.index.rebuilt` | `{ entries_indexed, elapsed_ms, dimension }` | Index | Index rebuild completed |
| `memory.cache.eviction` | `{ key, reason }` | Cache | Cache entry evicted |

### Consumed Events

| Source | Event | Handler |
|--------|-------|---------|
| Core | `core.config_changed` | Reload memory policies, intervals, tier config |
| Runtime | `runtime.session.destroyed` | Prune session-scoped memory |
| Runtime | `runtime.task.completed` | LearningManager analyzes for pattern discovery |
| Runtime | `runtime.context.changed` | ContextManager syncs context state |
| (future Brain) | `brain.memory.store` | WorkingMemory::store |
| (future Brain) | `brain.memory.recall` | MemoryRetriever::retrieve |
| (future Brain) | `brain.memory.forget` | WorkingMemory::forget |

---

## Thread Model

| Crate | Threading |
|-------|-----------|
| memory-core | Stateless types. No thread-local state. |
| memory-storage | `InMemoryStore`: `std::sync::RwLock<HashMap>`. Future backends: `spawn_blocking` for disk I/O. |
| memory-index | `InMemoryIndex`: `std::sync::RwLock<HashMap>`. Future HNSW: `RwLock` for graph, `spawn_blocking` for build. |
| memory-working | `std::sync::RwLock<LruCache>`. Lock per operation. |
| memory-episodic | Delegates to MemoryStore. Temporal index: `RwLock<BTreeMap>`. |
| memory-semantic | Delegates to MemoryStore. Fact index: `RwLock<HashMap>`. |
| memory-knowledge | Adjacency list: `RwLock<HashMap<EntityId, Vec<Edge>>>`. Graph traversal on caller's task. |
| memory-context | Context tree: `RwLock<HashMap<ContextId, ContextNode>>`. |
| memory-cache | `std::sync::RwLock<LruCache>`. Lock per operation. |
| memory-retrieval | Stateless query executor. No locks. |
| memory-snapshot | Snapshot metadata: `RwLock<HashMap>`. Export/import: `spawn_blocking`. |
| memory-learning | Tokio interval tasks. Consolidation/pruning: `spawn_blocking` for store/index operations. Pattern discovery: CPU-bound on `spawn_blocking`. |

All shared state uses `std::sync::RwLock` (not `tokio::sync::RwLock`) because critical sections are short (hash map lookups) and never held across `.await` points. Background tasks that perform I/O use `tokio::task::spawn_blocking`.

---

## Lifecycle

The Memory Platform is a single `Service` implementation (`MemoryPlatformService`) that wires all 12 crates together.

### Start Order

```
1. MemoryStore          (must be ready first — all stores depend on it)
2. MemoryIndex          (loaded from store metadata)
3. WorkingMemory        (may warm from store)
4. EpisodicMemory       (wraps store)
5. SemanticMemory       (wraps store)
6. KnowledgeBase        (wraps store)
7. ContextManager       (independent, no store dependency)
8. MemoryCache          (warms from working memory)
9. MemoryRetriever      (stateless, depends on index + store + cache)
10. SnapshotManager      (depends on store + index)
11. LearningManager      (depends on all above tasks)
12. Event subscriptions  (registered last)
```

### Running State

- WorkingMemory serves hot-path store/recall at <1µs latency.
- LearningManager runs background tasks: consolidation (default 30s interval), pruning (default 120s), pattern discovery (default 300s).
- SnapshotManager runs scheduled snapshots if configured.

### Stop Order

Reverse of start. LearningManager stops first (completes current consolidation/pruning cycle), then SnapshotManager, then retrieval and stores flush pending writes.

---

## Configuration

The Memory Platform is configured through `Config` (Core). Example configuration structure:

```json
{
  "memory": {
    "store": { "backend": "in-memory", "path": "/var/ai-os/memory" },
    "working": { "max_entries": 1024, "max_bytes": 67108864 },
    "episodic": { "max_events": 100000, "retention_days": 30 },
    "semantic": { "min_confidence": 0.6, "max_facts": 1000000 },
    "knowledge": { "max_entities": 100000, "max_relations": 500000 },
    "cache": { "max_entries": 512, "default_ttl_secs": 60 },
    "index": { "dimension": 384, "metric": "cosine" },
    "retrieval": { "strategy": "hybrid", "weights": { "embedding": 0.4, "keyword": 0.3, "knowledge": 0.2, "recency": 0.05, "importance": 0.05 } },
    "learning": {
      "consolidation_interval_secs": 30,
      "pruning_interval_secs": 120,
      "pattern_discovery_interval_secs": 300,
      "consolidation": { "importance_threshold": 0.7, "access_count_threshold": 5 },
      "pruning": { "importance_threshold": 0.2, "aggressive": false }
    },
    "snapshot": { "auto_snapshot": false, "snapshot_interval_secs": 3600, "max_snapshots": 10 }
  }
}
```

---

## Error Handling

`MemoryError` is the unified error type for all memory crates:

| Error Variant | Source | Recovery |
|---------------|--------|----------|
| `ObjectNotFound(MemoryId)` | All stores | Return None, caller handles |
| `TierFull(MemoryTier, u64, u64)` | WorkingMemory | Evict lowest-priority entries |
| `CapacityExceeded(String, u64, u64)` | All stores | Prune or reject with backpressure |
| `InvalidQuery(String)` | Retrieval | Return empty results |
| `IndexBuildFailed(String)` | Index | Degrade to keyword-only search |
| `SearchFailed(String)` | Index/Retrieval | Retry, degrade gracefully |
| `ConsolidationFailed(MemoryId, String)` | LearningManager | Leave in source tier, retry next cycle |
| `PruningFailed(String)` | LearningManager | Log error, skip problematic entries |
| `SnapshotFailed(String)` | SnapshotManager | Return error, caller retries |
| `PatternDiscoveryFailed(String)` | LearningManager | Log, retry next cycle |
| `ContextNotFound(ContextId)` | ContextManager | Create default context, log warning |
| `SerializationError(String)` | All | Log corruption, skip entry |
| `StorageBackendError(String)` | Store | Reconnect with backoff, buffer writes |
| `PermissionDenied(String, String)` | Retrieval | Return error to caller |
| `Timeout(u64)` | Retrieval | Return partial results |

---

## Data Flows

### Write Path: Store to Working Memory

```
Caller              WorkingMemory        MemoryCache         EventBus
  |                      |                   |                   |
  | store(object)        |                   |                   |
  |--------------------->|                   |                   |
  |                      | 1. Check capacity                    |
  |                      | 2. LRU insert                        |
  |                      | 3. Evict if full                     |
  |                      |                                     |
  |                      | if evicted:                          |
  |                      |  emit(object.evicted)                |
  |                      |------------------------------------->|
  |                      |                                     |
  |                      | if hot:                              |
  |                      |  set(key, object, ttl)               |
  |                      |------------------>|                  |
  |                      |                                     |
  |<---------------------|                   |                  |
  |                      |                                     |
```

### Write Path: Consolidation

```
LearningManager       WorkingMemory       MemoryStore        MemoryIndex
  |                       |                   |                   |
  | consolidate(id,wm)    |                   |                   |
  |---------------------->|                   |                   |
  |                       | recall(id)        |                   |
  |                       |------------------>|                   |
  |                       |   MemoryObject    |                   |
  |                       |<------------------|                   |
  |                       |                   |                   |
  | 1. Check policy       |                   |                   |
  | 2. Enrich object      |                   |                   |
  | 3. Determine target   |                   |                   |
  |    tier               |                   |                   |
  |                       |                   | insert(object)    |
  |                       |                   |------------------>|
  |                       |                   |                   |
  |                       |                   | index(id, emb)    |
  |                       |                   |                  >|
  |                       | forget(id)        |                   |
  |                       |<------------------|                   |
  |                       |                   |                   |
```

### Read Path: Retrieval

```
Caller              MemoryRetriever      MemoryCache     WorkingMemory   MemoryIndex   MemoryStore
  |                      |                   |                |              |              |
  | retrieve(query)      |                   |                |              |              |
  |--------------------->|                   |                |              |              |
  |                      | check permissions |               |              |              |
  |                      |                   |                |              |              |
  |                      | try cache:        |                |              |              |
  |                      |  get(query_hash)  |                |              |              |
  |                      |------------------>|                |              |              |
  |                      |   (miss)          |                |              |              |
  |                      |<------------------|                |              |              |
  |                      |                   |                |              |              |
  |                      | try working:      |                |              |              |
  |                      |  search(query)    |                |              |              |
  |                      |---------------------------------->|              |              |
  |                      |   results         |                |              |              |
  |                      |<----------------------------------|              |              |
  |                      |                   |                |              |              |
  |                      | hybrid_search:    |                |              |              |
  |                      |  embed(query)     |                |              |              |
  |                      |  cosine(top-k)    |                |              |              |
  |                      |  BM25(top-k)      |                |              |              |
  |                      |------------------------------------------------->|              |
  |                      |   scored_ids      |                |              |              |
  |                      |<-------------------------------------------------|              |
  |                      |                   |                |              |              |
  |                      |  fetch objects    |                |              |              |
  |                      |---------------------------------------------------------------->|
  |                      |   objects         |                |              |              |
  |                      |<----------------------------------------------------------------|
  |                      |                   |                |              |              |
  |                      | fusion + rerank   |                |              |              |
  |                      | cache results     |                |              |              |
  |                      |------------------>|                |              |              |
  |                      |                   |                |              |              |
  |  RetrievalResult[]   |                   |                |              |              |
  |<---------------------|                   |                |              |              |
```

---

## Performance Expectations

| Operation | Target Latency (p99) | Throughput |
|-----------|---------------------|------------|
| WorkingMemory store | < 500 ns | 5M+/s |
| WorkingMemory recall (cache hit) | < 500 ns | 5M+/s |
| MemoryIndex search (10K entries) | < 100 µs | 10K+/s |
| MemoryIndex search (1M entries) | < 5 ms | 200+/s |
| EpisodicMemory record | < 10 µs | 100K+/s |
| SemanticMemory store_fact | < 10 µs | 100K+/s |
| KnowledgeBase query (1000 nodes) | < 1 ms | 1K+/s |
| MemoryRetriever hybrid (10K entries) | < 5 ms | 200+/s |
| Consolidation (single entry) | < 10 ms | 100+/s |
| Pruning dry-run (10K entries) | < 50 ms | N/A |
| Snapshot (100K entries) | < 1 s | N/A |
| Pattern discovery (10K entries) | < 100 ms | N/A |

---

## Portability

- **No OS-specific code.** All memory crates use only standard Rust and cross-platform crates. Linux-specific logic (inotify, epoll, dbus) lives exclusively in OSAL.
- **Storage backends are selected at build time** via Cargo features: `mem-in-memory` (default), `mem-sqlite`, `mem-postgres`, `mem-rocksdb`.
- **Index algorithms** are feature-gated: `mem-flat` (default), `mem-hnsw`, `mem-ivf`.
- **Embedding computation** is the caller's responsibility. The Memory Platform stores pre-computed `Vec<f32>` only.
- **All file paths** use `PathBuf`. No hardcoded paths in library code.
- **Serialization** uses `serde` + `bincode` for internal storage. `serde_json` for external export.

---

## Future Extensions

- **Distributed Memory** — Raft-based replication across nodes. Partitioned by memory tier.
- **Multi-Modal Indexing** — Image embeddings (CLIP), audio embeddings, cross-modal retrieval.
- **Memory Compression** — Delta compression for similar objects. Zstd for archive tier.
- **Incremental Indexing** — Replace full rebuilds with incremental HNSW updates.
- **Federated Retrieval** — Query multiple remote Memory Platform instances.
- **Memory Encryption** — `MemoryCodec` trait for per-object encryption at rest.
- **Time-Series Store** — Dedicated column family for temporal data with downsampling.
- **Memory Diffing** — Delta storage for large objects that change incrementally.

---

## References

- [Architecture Overview](overview.md)
- [Runtime Platform](runtime.md)
- [OSAL](osal.md)
- [Core Platform](core.md)
- [Interface Definition: Memory](../interfaces/memory.md)
- [RFC-0002: Memory Platform](../rfc/RFC-0002-memory-platform.md)
- [Design Principles](../principles.md)
- [Specification: Memory Lifecycle](../specification.md#9-memory-lifecycle)
