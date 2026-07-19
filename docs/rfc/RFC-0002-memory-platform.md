# RFC-0002: Memory Platform

| Field | Value |
|---|---|
| **Status** | Accepted |
| **Author** | Project maintainers |
| **Phase** | Phase 5 |
| **Created** | 2025-02-15 |
| **Updated** | 2025-07-19 |
| **Requires** | RFC-0001 (System Platform provides filesystem abstraction) |
| **Supersedes** | None |

## Abstract

This RFC proposes the Memory Platform for the AI-native OS, organized as Phase 5 of the implementation roadmap. The Memory Platform provides a cognitive memory hierarchy across 12 crates spanning four tiers — Working, Episodic, Semantic, and Knowledge — with backend-agnostic storage (`MemoryStore` trait), hybrid search indexing (embedding + keyword + graph), event-driven consolidation/pruning/pattern discovery (`LearningManager`), hierarchical context management, hot caching, and snapshot/restore. It depends on the Runtime for task scheduling and context propagation and on OSAL for filesystem I/O. The design follows the six-phase memory lifecycle (Create, Store, Index, Retrieve, Consolidate, Prune) and integrates with EventBus, Service, Supervisor, PermissionChecker, and ResourceManager.

## Motivation

An AI-native OS must retain information across conversation turns, sessions, and restarts. Active memory lookups must complete in under 1 microsecond to avoid stalling inference pipelines. Memory must survive process restarts via write-ahead logging and a durable key-value store. Both semantic (embedding-based cosine similarity) and keyword (inverted-index) retrieval must be supported, along with hybrid fusion. Memory entries are scoped to the originating session; cross-session access requires explicit permission. Per-session quotas prevent one session from exhausting shared storage. Entries transition through well-defined states: created in WorkingMemory, consolidated to LongTermStorage, pruned when expired or unimportant. Without a dedicated Memory Platform, each agent would implement its own caching, persistence, and search logic, leading to fragmentation and inconsistent access control.

### Specification Cross-References

The Memory Platform implements the lifecycle defined in `specification.md` section 9 ("Memory Lifecycle"). The architectural overview lives in `architecture/memory.md`. Module-level decomposition is described in `modules/memory.md`.

## Design

### Overview

The Memory Platform is composed of **twelve crates** organized into a cognitive memory hierarchy with 4 tiers:

```
Tier 1 (Working):   memory-working + memory-cache
Tier 2 (Episodic):  memory-episodic
Tier 3 (Semantic):  memory-semantic + memory-knowledge
Tier 4 (Archive):   memory-storage (persistent backends)

Infrastructure:
  memory-core        — shared types, traits, errors, events
  memory-storage     — MemoryStore trait (backend-agnostic CRUD)
  memory-index       — MemoryIndex trait (embedding + keyword)
  memory-context     — hierarchical context scoping
  memory-retrieval   — hybrid fusion engine
  memory-snapshot    — checkpoint/restore
  memory-learning    — consolidation, pruning, pattern discovery
```

Dependencies: Runtime (Scheduler, ContextManager, PermissionChecker, ResourceManager), OSAL filesystem for durable storage paths, Core EventBus for cross-cutting notifications.

### MemoryStore Trait

Both WorkingMemory and LongTermStorage implement a common `MemoryStore` trait for polymorphic use.

```rust
use thiserror::Error;
use async_trait::async_trait;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("entry not found: {key}")]
    NotFound { key: String },
    #[error("quota exceeded for session {session_id}")]
    QuotaExceeded { session_id: String, limit: u64, current: u64 },
    #[error("index operation failed: {detail}")]
    IndexFailure { detail: String },
    #[error("storage backend error: {detail}")]
    StorageFailure { detail: String },
    #[error("operation timed out")]
    Timeout,
    #[error("permission denied: session {session_id} cannot access {key}")]
    PermissionDenied { session_id: String, key: String },
}

pub type MemoryResult<T> = Result<T, MemoryError>;

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub content_type: String,
    pub session_id: String,
    pub timestamp: i64,
    pub ttl: Option<Duration>,
    pub importance: f32,
    pub tags: Vec<String>,
    pub trace_id: String,
    pub access_count: u64,
    pub last_access: i64,
}

#[derive(Debug, Clone)]
pub struct MemoryMetadata {
    pub key: String,
    pub session_id: String,
    pub timestamp: i64,
    pub importance: f32,
    pub tags: Vec<String>,
    pub size_bytes: u64,
    pub access_count: u64,
    pub last_access: i64,
}

#[derive(Debug, Clone)]
pub struct QueryRequest {
    pub query_text: String,
    pub session_id: String,
    pub top_k: usize,
    pub threshold: Option<f32>,
    pub deadline: Option<tokio::time::Instant>,
    pub include_semantic: bool,
    pub include_keyword: bool,
    pub filters: QueryFilters,
}

#[derive(Debug, Default, Clone)]
pub struct QueryFilters {
    pub session_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub importance_min: Option<f32>,
    pub time_range: Option<(i64, i64)>,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub entries: Vec<ScoredEntry>,
    pub query_latency: Duration,
    pub semantic_latency: Option<Duration>,
    pub keyword_latency: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct ScoredEntry {
    pub entry: MemoryEntry,
    pub score: f32,
    pub match_source: MatchSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchSource { Semantic, Keyword, Hybrid, Exact }

#[async_trait]
pub trait MemoryStore: Debug + Send + Sync {
    async fn store(&self, entry: MemoryEntry) -> MemoryResult<()>;
    async fn retrieve(&self, key: &str, session_id: &str) -> MemoryResult<MemoryEntry>;
    async fn delete(&self, key: &str, session_id: &str) -> MemoryResult<()>;
    async fn list_session(&self, session_id: &str) -> MemoryResult<Vec<MemoryMetadata>>;
    async fn session_size(&self, session_id: &str) -> MemoryResult<u64>;
}
```

### 1. WorkingMemory

In-process, bounded, concurrent cache using `lru::LruCache` wrapped in `tokio::sync::RwLock`. Provides O(1) access and LRU eviction. Each entry carries a TTL; expired entries are skipped on retrieval and lazily evicted. When full, evicted entries are handed to the ConsolidationEngine via a `tokio::sync::mpsc` channel.

Key decisions: single `RwLock<LruCache<String, CachedEntry>>` guards all operations. Reads take a read lock; writes take a write lock. Benchmarks show sub-microsecond read latency under moderate contention. Capacity is configured by entry count and byte budget. Eviction handler sends entries to the ConsolidationEngine.

```rust
#[derive(Debug)]
struct CachedEntry {
    inner: MemoryEntry,
    expires_at: Option<tokio::time::Instant>,
    embedding: Option<Vec<f32>>,
}

#[derive(Debug)]
pub struct WorkingMemory {
    cache: RwLock<LruCache<String, CachedEntry>>,
    config: WorkingMemoryConfig,
    eviction_tx: tokio::sync::mpsc::UnboundedSender<MemoryEntry>,
    index_tx: tokio::sync::mpsc::UnboundedSender<IndexOp>,
}

#[async_trait]
impl MemoryStore for WorkingMemory {
    async fn store(&self, entry: MemoryEntry) -> MemoryResult<()> {
        // 1. Check quota via ResourceManager. 2. Compute expires_at from ttl.
        // 3. Write lock, push to LruCache; send evicted entry to eviction_tx.
        // 4. Send IndexOp::Upsert to index_tx. 5. Emit MemoryStored event.
        Ok(())
    }
    async fn retrieve(&self, key: &str, session_id: &str) -> MemoryResult<MemoryEntry> {
        // Read lock, probe cache. If found and not expired, update access_count.
        // If expired, upgrade to write lock, remove, return NotFound.
        Err(MemoryError::NotFound { key: key.to_string() })
    }
}
```

**Lifecycle mapping**: WorkingMemory implements phases 1-3 (Create, Store, Index) synchronously for the hot path. Phase 4 (Retrieve) hits WorkingMemory first. Phase 5 (Consolidate) occurs via the eviction hook. Phase 6 (Prune) is handled by PruningEngine.

### 2. LongTermStorage

Persistent, async-friendly key-value store built on RocksDB (`rust-rocksdb`; `sled` is the secondary candidate). Uses column families:

| Column Family | Key Pattern | Value |
|---|---|---|
| `entries` | `{session_id}:{key}` | Serialized `MemoryEntry` (bincode) |
| `embeddings` | `{session_id}:{key}` | Serialized `Vec<f32>` |
| `metadata` | `{session_id}:{key}` | Serialized `MemoryMetadata` |
| `relationships` | `{session_id}:{key}` | Edge list |
| `wal_seq` | sequence number | Opaque WAL entry |

All I/O dispatched via `tokio::task::spawn_blocking`. Batch writes collect pending operations and flush atomically.

```rust
#[derive(Debug)]
pub struct LongTermStorage {
    db: Arc<DB>,
    config: LongTermStorageConfig,
    batch_tx: tokio::sync::mpsc::UnboundedSender<BatchOp>,
}

impl LongTermStorage {
    pub fn open(config: LongTermStorageConfig) -> MemoryResult<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(config.create_if_missing);
        let db = DB::open_cf(&opts, &config.path, &[
            "entries", "embeddings", "metadata", "relationships", "wal_seq",
        ]).map_err(|e| MemoryError::StorageFailure { detail: e.to_string() })?;
        let (batch_tx, batch_rx) = tokio::sync::mpsc::unbounded_channel();
        let storage = Self { db: Arc::new(db), config, batch_tx };
        storage.spawn_batch_flusher(batch_rx);
        Ok(storage)
    }
}

#[async_trait]
impl MemoryStore for LongTermStorage {
    async fn retrieve(&self, key: &str, session_id: &str) -> MemoryResult<MemoryEntry> {
        let db = self.db.clone();
        let cf = db.cf_handle("entries").unwrap();
        let prefixed_key = format!("{}:{}", session_id, key);
        tokio::task::spawn_blocking(move || {
            db.get_cf(&cf, prefixed_key.as_bytes())
                .map_err(|e| MemoryError::StorageFailure { detail: e.to_string() })?
                .ok_or_else(|| MemoryError::NotFound { key: prefixed_key })
                .and_then(|bytes| bincode::deserialize(&bytes)
                    .map_err(|e| MemoryError::StorageFailure { detail: e.to_string() }))
        }).await.map_err(|e| MemoryError::StorageFailure { detail: e.to_string() })?
    }
}
```

### 3. MemoryIndex

Dual index: semantic (in-memory `HashMap<String, Vec<f32>>` with cosine similarity via `fastembed`) and keyword (`tantivy` with BM25 scoring on body, tags, timestamp, importance, session_id). Hybrid fusion: `final_score = alpha * semantic_score + (1 - alpha) * keyword_score` where `alpha` defaults to 0.6.

```rust
#[derive(Debug)]
pub enum IndexOp {
    Upsert { key: String, entry: MemoryEntry, embedding: Vec<f32> },
    Delete { key: String },
}

#[derive(Debug)]
pub struct MemoryIndex {
    semantic_index: Arc<RwLock<HashMap<String, Vec<f32>>>>,
    tantivy_index: Index,
    model: EmbeddingModel,
    config: MemoryIndexConfig,
}

impl MemoryIndex {
    pub async fn embed(&self, text: &str) -> MemoryResult<Vec<f32>> {
        let model = self.model.clone();
        let text = text.to_string();
        tokio::task::spawn_blocking(move || {
            model.embed(&[text], EmbeddingBatch::Single)
                .map_err(|e| MemoryError::IndexFailure { detail: e.to_string() })
        }).await.map_err(|e| MemoryError::IndexFailure { detail: e.to_string() })?
            .into_iter().next().ok_or_else(|| MemoryError::IndexFailure {
                detail: "empty embedding result".into()
            })
    }

    pub async fn search(&self, request: &QueryRequest) -> MemoryResult<Vec<ScoredEntry>> {
        // Embed query, compute cosine similarity against semantic_index.
        // Run BM25 via tantivy. Fuse with hybrid_alpha. Apply top-K/threshold/deadline.
        todo!()
    }
}
```

### 4. MemoryRetriever

Primary query interface. Checks session permissions, searches WorkingMemory first (fast path), falls through to LongTermStorage on cache miss, performs hybrid search via MemoryIndex, merges and ranks results, enforces deadlines, applies session isolation.

```rust
#[derive(Debug)]
pub struct MemoryRetriever {
    working: Arc<WorkingMemory>,
    long_term: Arc<LongTermStorage>,
    index: Arc<MemoryIndex>,
    permission_checker: Arc<dyn PermissionChecker>,
    scheduler: Arc<dyn Scheduler>,
}

impl MemoryRetriever {
    pub async fn query(&self, request: QueryRequest) -> MemoryResult<QueryResult> {
        self.permission_checker
            .check_access(&request.session_id, "memory:query")
            .map_err(|_| MemoryError::PermissionDenied {
                session_id: request.session_id.clone(), key: "*".into(),
            })?;
        if let Some(deadline) = request.deadline {
            if tokio::time::Instant::now() > deadline {
                return Err(MemoryError::Timeout);
            }
        }
        // Parallel search: WorkingMemory list + MemoryIndex search.
        // Merge, deduplicate, rank, return QueryResult with latency breakdown.
        todo!()
    }

    pub async fn retrieve_exact(&self, key: &str, session_id: &str) -> MemoryResult<MemoryEntry> {
        match self.working.retrieve(key, session_id).await {
            Ok(entry) => Ok(entry),
            Err(MemoryError::NotFound { .. }) => self.long_term.retrieve(key, session_id).await,
            Err(e) => Err(e),
        }
    }
}
```

### 5. ConsolidationEngine

Background task that moves high-value entries from WorkingMemory into LongTermStorage. Two triggers: (a) on eviction — immediate channel delivery, (b) periodic sweep every 30s — entries with `access_count > promotion_threshold` are promoted. Enriches entries during consolidation (re-embeds if model version changed, updates keyword index).

```rust
#[derive(Debug)]
pub struct ConsolidationEngine {
    config: ConsolidationConfig,
    working: Arc<WorkingMemory>,
    long_term: Arc<LongTermStorage>,
    index: Arc<MemoryIndex>,
    eviction_rx: tokio::sync::mpsc::UnboundedReceiver<MemoryEntry>,
}

impl ConsolidationEngine {
    pub async fn run(self) {
        let mut interval = tokio::time::interval(self.config.consolidation_interval);
        loop {
            tokio::select! {
                Some(entry) = self.eviction_rx.recv() => self.consolidate_entry(entry).await,
                _ = interval.tick() => {
                    if let Ok(entries) = self.working.list_all_entries().await {
                        for entry in entries {
                            if entry.access_count >= self.config.promotion_threshold {
                                self.consolidate_entry(entry).await;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn consolidate_entry(&self, entry: MemoryEntry) {
        // Re-embed, store in LongTermStorage, update MemoryIndex, emit MemoryConsolidated.
    }
}
```

### 6. PruningEngine

Background task that removes expired (TTL) and low-importance entries. Runs on configurable interval (default 60s). Targets both stores. Policy: if TTL expired, remove unconditionally. If session exceeds quota, remove lowest-importance entries until under quota. LongTermStorage applies retention policies (`TimeBased`, `ImportanceThreshold`, `KeepAll`).

```rust
#[derive(Debug, Clone)]
pub enum RetentionPolicy { TimeBased(Duration), ImportanceThreshold(f32), KeepAll }

#[derive(Debug)]
pub struct PruningEngine {
    config: PruningConfig,
    working: Arc<WorkingMemory>,
    long_term: Arc<LongTermStorage>,
    resource_manager: Arc<dyn ResourceManager>,
    event_bus: Arc<dyn EventBus>,
}

impl PruningEngine {
    pub async fn run(self) {
        let mut interval = tokio::time::interval(self.config.pruning_interval);
        loop { interval.tick().await; self.prune_cycle().await; }
    }

    async fn prune_cycle(&self) {
        // Prune expired TTL from WorkingMemory.
        // For each over-quota session, remove lowest-importance entries.
        // Prune LongTermStorage by retention policy. Emit MemoryPruned.
    }
}
```

### Integration with Existing Subsystems

**ContextManager**: Every `MemoryEntry` carries a `trace_id` read from `ContextManager::current()`.

**PermissionChecker**: All stores call `check_access(session_id, resource)` where resource is `memory:{operation}:{key}`.

**ResourceManager**: Tracks per-session byte usage. `allocate()` on store, `release()` on delete.

**EventBus events**: `MemoryStored`, `MemoryRetrieved`, `MemoryConsolidated`, `MemoryPruned`, `MemoryError`.

**Scheduler**: ConsolidationEngine and PruningEngine are long-lived tasks registered via `scheduler.register_service()`, managed by Supervisor for restart on failure.

### Error Strategy

All errors use `MemoryError`. Callers match on variants: `NotFound` (fall through), `QuotaExceeded` (notify/prune), `Timeout` (degrade), `StorageFailure`/`IndexFailure` (log + retry). Logging uses the existing `Logger` service at `warn`/`error` severity.

## Drawbacks

1. **RocksDB dependency**: Adds native library build complexity. `sled` is a pure-Rust alternative with weaker guarantees. A feature flag can mitigate but increases test surface.
2. **Embedding model size**: `fastembed` consumes 100-500 MB RAM. Resource-constrained deployments may need a lightweight model or external embedding service.
3. **Cache coherence**: WorkingMemory and LongTermStorage are eventually consistent. Writes to WorkingMemory may not immediately appear in LongTermStorage. Exact-match queries route to WorkingMemory first to minimize staleness.
4. **Batch flush window**: LongTermStorage's batch writer introduces data loss risk on crash (default 50ms). WAL column family mitigates for critical entries.
5. **Index memory**: In-memory semantic index may exceed RAM for millions of entries. Future work will integrate disk-backed HNSW or an external vector DB via the trait abstraction.

## Alternatives Considered

**Single-tier storage (no WorkingMemory)**: Simplifies architecture but misses the 1-microsecond latency target — even buffered RocksDB reads incur 5-50 microseconds.

**External vector database (Qdrant/Pinecone)**: Minimizes network dependencies for core runtime functions. External service introduces latency variance unacceptable for sub-millisecond retrieval. The in-process index can be swapped later via trait.

**Single `TieredMemoryStore`**: Was split for testability — WorkingMemory and LongTermStorage have distinct failure modes, config profiles, and lifecycle requirements. The `MemoryStore` trait allows independent testing and composition.

**No dedicated WAL CF**: RocksDB's built-in WAL is sufficient, but the dedicated CF allows PruningEngine to scan WAL without interfering with main data CFs and enables replay-based recovery without full table scans.

## Open Questions

1. **Pagination**: `list_session` returns all metadata. For sessions with thousands of entries, this is expensive. Initial implementation will warn at 1000 entries and require pagination in a follow-up.
2. **Embedding versioning**: When the model is updated, existing embeddings become incomparable. Store `model_version` alongside each embedding and recompute on access? Or trigger full re-index? The versioned approach is preferred.
3. **Cross-session sharing**: No mechanism for session A to share a memory with session B. A share table or capability-based access is deferred.
4. **Serialization format**: `bincode` for Rust-to-Rust. If non-Rust consumers appear, a self-describing format (messagepack, JSON) may be needed.

## Architecture Decisions

The following decisions were made during the architecture design phase. Full design rationale is in `docs/architecture/memory.md`.

### Decision 1: Cognitive Memory Hierarchy (4 tiers)

**Working, Episodic, Semantic, Knowledge** form a four-tier cognitive hierarchy. Objects flow upward through consolidation: Working → Episodic (event sequences) → Semantic (generalized facts) → Knowledge (structured graph). Each tier has distinct durability, capacity, and latency characteristics.

**Rationale**: A single long-term store conflates fundamentally different memory types. Episodic recall needs temporal indexing. Semantic recall needs fact queries. Knowledge recall needs graph traversal. Separating them at the type level enables specialized indexing and query paths.

### Decision 2: Backend-Agnostic Storage via MemoryStore Trait

All persistent storage goes through the `MemoryStore` trait in `memory-storage`. No memory crate imports a database driver directly. Initial backends: in-memory (testing), SQLite (production). Future: PostgreSQL, RocksDB, object storage.

**Rationale**: Database technology evolves rapidly. The trait boundary isolates the memory subsystem from storage backend changes. Deployments can choose the appropriate backend without code changes.

### Decision 3: 12-Crate Structure Mirroring OSAL Pattern

The 12 memory crates follow the same structure as the 11 OSAL crates: a `memory-core` foundation, specialized trait crates, and a future `memory-linux` for any platform-specific backends. Each crate is independently testable and publishable.

**Rationale**: Consistency with OSAL reduces cognitive load for developers working across layers. Independent crates enable parallel implementation and incremental adoption.

### Decision 4: Event-Driven Learning

The `LearningManager` is not a passive garbage collector. It actively observes access patterns, discovers temporal/sequential/associative/causal patterns, and autonomously manages the memory lifecycle. Pattern discovery runs on a configurable interval (default 300s).

**Rationale**: An AI operating system's memory must be self-managing. Static TTL policies are insufficient — the system must learn what to remember and what to forget based on actual usage patterns.

### Decision 5: Immutable, Versioned Memory Records

`MemoryObject` updates create new versions rather than mutating in place. The version chain provides an audit trail. Objects carry a SHA-256 checksum for integrity verification.

**Rationale**: Auditability and traceability are requirements for an AI system that makes decisions based on historical state. Versioned records enable time-travel queries and rollback.

### Decision 6: Four-Contributor Hybrid Retrieval

Retrieval fuses four signals: embedding similarity (cosine), keyword BM25, knowledge graph traversal score, and recency/importance. Each contributor has configurable weight. The fusion is fully explainable — every result carries a per-contributor score breakdown.

**Rationale**: No single retrieval method is optimal for all query types. Embedding similarity captures semantics, BM25 captures exact keywords, graph traversal captures structured relationships, and recency/importance captures temporal relevance.

### Decision 7: Snapshot/Restore as First-Class Primitive

The `SnapshotManager` is not an afterthought — it is a core trait with scheduled snapshots, export/import, and multiple restore strategies (Replace, Merge, MergeKeepExisting).

**Rationale**: An AI OS that runs continuously for years must support checkpointing, rollback, and migration. Snapshots enable safe experimentation (branch-and-restore), disaster recovery, and state transfer between instances.

## Implementation Plan

### Phase 5A — Architecture & Traits (current phase)
1. Define all trait interfaces in `docs/interfaces/memory.md`
2. Design crate structure and dependency graph in `docs/architecture/memory.md`
3. Finalize RFC-0002 with architecture decisions

### Phase 5B — memory-core + memory-storage + memory-index
1. **memory-core**: `MemoryId`, `MemoryObject`, `MemoryError`, `MemoryEvent`, `QueryFilter`, core enums, default implementations
2. **memory-storage**: `MemoryStore` trait + `InMemoryStore` implementation + unit tests
3. **memory-index**: `MemoryIndex` trait + `InMemoryIndex` (flat cosine) + unit tests
4. Integration: core ↔ storage ↔ index

### Phase 5C — memory-working + memory-cache + memory-context
1. **memory-working**: `WorkingMemory` trait + LRU implementation + TTL + eviction channel + tests
2. **memory-cache**: `MemoryCache` trait + LRU cache + TTL + pattern invalidation + tests
3. **memory-context**: `ContextManager` trait + tree-based context + snapshot/merge + tests

### Phase 5D — memory-episodic + memory-semantic + memory-knowledge
1. **memory-episodic**: `EpisodicMemory` trait + temporal indexing + event storage + tests
2. **memory-semantic**: `SemanticMemory` trait + fact store + confidence tracking + tests
3. **memory-knowledge**: `KnowledgeBase` trait + adjacency list + graph traversal + simple inference + tests

### Phase 5E — memory-retrieval + memory-snapshot
1. **memory-retrieval**: `MemoryRetriever` trait + hybrid fusion + reranking + explanation + tests
2. **memory-snapshot**: `SnapshotManager` trait + JSON snapshot + export/import + restore strategies + tests

### Phase 5F — memory-learning + Integration
1. **memory-learning**: `LearningManager` trait + consolidation engine + pruning engine + pattern discovery + tests
2. **MemoryPlatformService**: wiring all 12 crates into a single `Service` implementation
3. End-to-end lifecycle tests (store → consolidate → retrieve → prune → snapshot → restore)
4. Event integration with Core EventBus and Runtime
5. Criterion benchmarks for all performance-critical paths

### Milestones

| Milestone | Deliverables | Target |
|---|---|---|
| M1 | Architecture docs finalized | Phase 5A |
| M2 | memory-core + storage + index working | Phase 5B |
| M3 | Working memory + cache + context | Phase 5C |
| M4 | Episodic + semantic + knowledge tiers | Phase 5D |
| M5 | Retrieval + snapshot engines | Phase 5E |
| M6 | Learning engine + full integration | Phase 5F |

## Unresolved Topics

- **Memory encryption at rest**: Disk-level encryption (dm-crypt) is assumed. A `MemoryCodec` trait could be inserted for per-entry encryption.
- **Distributed memory**: Single-node design. Future RFC will address raft-based replication or external database backend.
- **Ephemeral memory mode**: A `StorageMode::Ephemeral` flag could skip eviction and drop entries on eviction.
- **Memory diffing**: Delta storage for large entries that change incrementally is deferred.
- **Observability**: Metrics (cache hit rate, index size, consolidation lag, prune count) via HealthMonitor and Prometheus are tracked separately.

## References

1. `specification.md` — Section 9: Memory Lifecycle (Create, Store, Index, Retrieve, Consolidate, Prune).
2. `architecture/memory.md` — Full Memory Platform architecture: crate structure, dependency graph, data model, event model, thread model, data flows.
3. `interfaces/memory.md` — Complete trait definitions for all 11 public interfaces + data model + error model + events.
4. RFC-0001 — System Platform: filesystem abstraction.
5. `crates/core/event-bus/src/lib.rs` — EventBus trait.
6. `crates/runtime/scheduler/src/lib.rs` — Scheduler trait.
7. `crates/runtime/permissions/src/lib.rs` — PermissionChecker trait.
8. `crates/runtime/resource-manager/src/lib.rs` — ResourceManager trait.
