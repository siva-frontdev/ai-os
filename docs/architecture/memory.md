# Memory Platform Blueprint

## Purpose

The Memory Platform (Phase 5, Planned) provides persistent and ephemeral storage for all platform knowledge. It bridges the gap between raw system data and actionable intelligence by implementing a multi-tier memory hierarchy: high-speed short-term working memory for active context, long-term persistent storage for historical records, and an indexing layer that enables semantic and keyword retrieval. Every module above this layer -- Brain, Perception, Execution -- depends on Memory for state persistence and recall.

The Memory Platform builds on the [System Platform](system.md) (filesystem abstraction) and [Core Platform](core.md) (EventBus). It depends on no other AI-native OS crate beyond Core and System.

---

## Responsibilities

- **Short-Term Memory**: Maintain a fixed-capacity in-memory cache of recent items with LRU eviction. Support TTL-based expiration per item. Provide O(1) get/insert/remove operations.
- **Long-Term Storage**: Persist memory items to an embedded database (RocksDB or SQLite). Support key-value and column-family access patterns. Provide batch write and cursor-based iteration.
- **Memory Indexing**: Build and maintain search indices over stored memory items. Support embedding-based vector indexes for semantic search and inverted indexes for keyword search. Store index metadata alongside each item.
- **Memory Retrieval**: Expose a unified query interface supporting semantic similarity search (cosine distance on embeddings), keyword matching (term frequency / inverse document frequency), and hybrid queries that combine both. Support filtering by metadata fields, time ranges, and memory tier.
- **Memory Consolidation**: Implement a promotion policy that moves items from short-term to long-term memory based on access frequency, recency, and explicit "pin" requests. Run consolidation on a configurable interval.
- **Memory Pruning**: Remove expired items based on TTL. Evict items from short-term memory under capacity pressure. Apply long-term retention limits with global or per-category capacity bounds. Publish pruning events for observability.

---

## Public Interfaces

### MemoryStore (`memory::store`)

```rust
pub enum MemoryTier {
    ShortTerm,
    LongTerm,
}

pub struct MemoryItem {
    pub id: Uuid,
    pub key: String,
    pub data: Vec<u8>,
    pub content_type: String,
    pub metadata: HashMap<String, String>,
    pub embedding: Option<Vec<f32>>,
    pub tier: MemoryTier,
    pub ttl: Option<Duration>,
    pub created_at: DateTime<Utc>,
    pub accessed_at: DateTime<Utc>,
    pub access_count: u64,
}

pub trait MemoryStore: Debug + Send + Sync {
    async fn insert(&self, item: MemoryItem) -> Result<(), MemoryError>;
    async fn get(&self, key: &str) -> Result<Option<MemoryItem>, MemoryError>;
    async fn remove(&self, key: &str) -> Result<(), MemoryError>;
    async fn exists(&self, key: &str) -> Result<bool, MemoryError>;
    fn tier(&self) -> MemoryTier;
}
```

### MemoryIndex (`memory::index`)

```rust
pub struct IndexConfig {
    pub embedding_dim: usize,
    pub similarity_metric: SimilarityMetric,
    pub keyword_algorithm: KeywordAlgorithm,
}

pub enum SimilarityMetric { Cosine, Euclidean, DotProduct }
pub enum KeywordAlgorithm { TfIdf, Bm25 }

pub trait MemoryIndex: Debug + Send + Sync {
    async fn index_item(&self, item: &MemoryItem) -> Result<(), MemoryError>;
    async fn remove_from_index(&self, key: &str) -> Result<(), MemoryError>;
    async fn rebuild_index(&self) -> Result<(), MemoryError>;
    fn stats(&self) -> IndexStats;
}
```

### MemoryRetriever (`memory::retrieval`)

```rust
pub struct Query {
    pub text: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub filters: Vec<Filter>,
    pub tier: Option<MemoryTier>,
    pub limit: usize,
    pub min_score: f64,
}

pub struct Filter {
    pub field: String,
    pub op: FilterOp,
    pub value: String,
}

pub enum FilterOp { Eq, Neq, Gt, Lt, Contains, In }

pub struct SearchResult {
    pub item: MemoryItem,
    pub score: f64,
    pub match_type: MatchType,
}

pub enum MatchType { Semantic, Keyword, Hybrid }

pub trait MemoryRetriever: Debug + Send + Sync {
    async fn search(&self, query: Query) -> Result<Vec<SearchResult>, MemoryError>;
    async fn search_semantic(&self, embedding: &[f32], limit: usize) -> Result<Vec<SearchResult>, MemoryError>;
    async fn search_keyword(&self, text: &str, limit: usize) -> Result<Vec<SearchResult>, MemoryError>;
    async fn hybrid_search(&self, query: Query) -> Result<Vec<SearchResult>, MemoryError>;
}
```

### MemoryConsolidator (`memory::consolidation`)

```rust
pub struct ConsolidationPolicy {
    pub min_access_count: u64,
    pub min_age: Duration,
    pub batch_size: usize,
    pub interval: Duration,
}

pub trait MemoryConsolidator: Debug + Send + Sync {
    async fn run_consolidation(&self) -> Result<ConsolidationReport, MemoryError>;
    fn policy(&self) -> &ConsolidationPolicy;
    async fn pin_item(&self, key: &str) -> Result<(), MemoryError>;
    async fn unpin_item(&self, key: &str) -> Result<(), MemoryError>;
}
```

### MemoryPruner (`memory::pruning`)

```rust
pub struct PruningConfig {
    pub short_term_max_items: usize,
    pub long_term_max_items: usize,
    pub short_term_ttl: Duration,
    pub default_ttl: Duration,
    pub pruning_interval: Duration,
}

pub trait MemoryPruner: Debug + Send + Sync {
    async fn prune_expired(&self) -> Result<PruningReport, MemoryError>;
    async fn prune_by_capacity(&self) -> Result<PruningReport, MemoryError>;
    async fn prune_by_policy(&self, policy: PruningPolicy) -> Result<PruningReport, MemoryError>;
    fn config(&self) -> &PruningConfig;
}
```

### MemoryPlatform (`memory::MemoryPlatform`)

```rust
pub struct MemoryPlatform {
    // Short-term (in-memory LRU)
    pub short_term: Arc<dyn MemoryStore>,
    // Long-term (RocksDB/SQLite)
    pub long_term: Arc<dyn MemoryStore>,
    // Indexing layer
    pub index: Arc<dyn MemoryIndex>,
    // Retrieval layer
    pub retriever: Arc<dyn MemoryRetriever>,
    // Consolidation policy engine
    pub consolidator: Arc<dyn MemoryConsolidator>,
    // Pruning engine
    pub pruner: Arc<dyn MemoryPruner>,
}
```

`MemoryPlatform` implements the Core `Service` trait. Its `start()` method initializes the backing store, loads indices, and spawns background consolidation and pruning tasks.

---

## Dependencies

| Crate | Purpose | Subsystem |
|---|---|---|
| `ai-os-core` | EventBus, Service, Logger, LifecycleManager | All subsystems |
| `ai-os-system` | FileSystemProvider for long-term storage paths | LongTermStore |
| `rocksdb` / `rusqlite` | Embedded persistent storage engine | LongTermStore |
| `lru` | LRU cache for short-term memory | ShortTermStore |
| `fastembed` / `ort` | ONNX-based embedding generation | MemoryIndex |
| `tantivy` | Inverted index for full-text keyword search | MemoryIndex, MemoryRetriever |
| `tokenizers` | Text tokenization for embedding pipeline | MemoryIndex |
| `tokio` | Async runtime, periodic tasks | Consolidator, Pruner |
| `serde` / `serde_json` | Memory item serialization | All subsystems |
| `chrono` | Timestamps for TTL and access tracking | All subsystems |
| `uuid` | Memory item identifiers | Store |
| `thiserror` | Error type derivation | All subsystems |

---

## Events Published

| Event | Type String | Subsystem | Trigger |
|---|---|---|---|
| `MemoryItemStored` | `memory.item.stored` | MemoryStore | New item inserted into any tier |
| `MemoryItemRetrieved` | `memory.item.retrieved` | MemoryStore | Existing item accessed |
| `MemoryItemEvicted` | `memory.item.evicted` | ShortTermStore | LRU eviction due to capacity |
| `MemoryItemExpired` | `memory.item.expired` | MemoryPruner | TTL expiration |
| `MemoryConsolidationCompleted` | `memory.consolidation.completed` | MemoryConsolidator | Batch promotion finished |
| `MemoryIndexRebuilt` | `memory.index.rebuilt` | MemoryIndex | Full index rebuild |
| `MemoryPruningCompleted` | `memory.pruning.completed` | MemoryPruner | Pruning cycle finished |
| `MemoryQueryExecuted` | `memory.query.executed` | MemoryRetriever | Search query processed |

---

## Events Consumed

| Event | Source | Consumer | Purpose |
|---|---|---|---|
| `system.file.modified` | FileSystemProvider | MemoryConsolidator | Refresh cached file content in memory |
| `system.file.created` | FileSystemProvider | MemoryConsolidator | Index new file content |
| `system.file.deleted` | FileSystemProvider | MemoryPruner | Remove indexed file content |
| `runtime.session_destroyed` | SessionManager | MemoryPruner | Prune session-scoped memory items |

The Memory Platform subscribes to filesystem events to keep its indices synchronized with on-disk state. Session destruction triggers cleanup of ephemeral memory items tied to that session.

---

## Thread Model

| Subsystem | Threading |
|---|---|
| `ShortTermStore` (LRU) | `RwLock<LruCache>`. All operations under read/write lock. |
| `LongTermStore` (RocksDB) | RocksDB `DB` is `Send + Sync`. Operations are CPU-bound (no async I/O). Wrapped in `spawn_blocking` for long-running operations. |
| `MemoryIndex` (tantivy) | Tantivy `Index` behind `RwLock`. Index writes are `spawn_blocking`. Reads on caller's task. |
| `MemoryRetriever` | Stateless query executor. Embedding comparison on caller's task. HNSW index reads under read lock. |
| `MemoryConsolidator` | Tokio periodic task. Scans short-term store, promotes qualifying items. Runs `spawn_blocking` for index operations. |
| `MemoryPruner` | Tokio periodic task. Scans both tiers, removes expired/evicted items. Runs `spawn_blocking` for database deletes. |

The consolidation and pruning background tasks run at configurable intervals (default: consolidation every 60 seconds, pruning every 120 seconds). Both tasks publish completion events so the HealthMonitor can track their liveness.

---

## Lifecycle

`MemoryPlatform` implements Core's `Service` trait:

1. **Construction**: `MemoryPlatform::new()` creates all subsystem instances with their configurations. The long-term store path is resolved through the [System Platform](system.md) `FileSystemProvider`.
2. **Start**: `start()` opens the RocksDB/SQLite database, loads or builds the index from long-term storage, seeds the short-term cache with recently accessed items, spawns the consolidation and pruning tasks, and registers event subscriptions.
3. **Running**: The platform processes insert, retrieve, and search requests. Background tasks run on their intervals. Index updates are queued asynchronously for write coalescing.
4. **Stop**: `stop()` flushes pending index writes, persists the short-term cache to long-term storage (if configured), stops background tasks, closes the database connection, and unregisters event subscriptions.

Start order within the Memory Platform:

```
1. LongTermStore   (backing database must be ready first)
2. ShortTermStore  (may warm from LongTermStore)
3. MemoryIndex     (loaded from LongTermStore metadata)
4. MemoryRetriever (stateless, depends on index)
5. MemoryConsolidator (depends on both stores)
6. MemoryPruner    (depends on both stores)
7. Event subscriptions registered last
```

---

## Error Handling

`MemoryError` is the unified error type:

| Variant | Subsystem | Condition |
|---|---|---|
| `StoreNotInitialized` | MemoryStore | Operation attempted before `start()` |
| `ItemNotFound(String)` | MemoryStore | Get/remove on non-existent key |
| `TierFull(String)` | ShortTermStore | LRU at capacity and TTL not reached |
| `IndexBuildFailed(String)` | MemoryIndex | Embedding model load or inference failure |
| `SearchExecutionFailed(String)` | MemoryRetriever | Index corruption or invalid query |
| `ConsolidationFailed(String)` | MemoryConsolidator | Batch promotion partial failure |
| `PruningFailed(String)` | MemoryPruner | Database delete error |
| `SerializationError(String)` | All subsystems | Serde encode/decode failure |
| `DatabaseConnectionFailed(String)` | LongTermStore | RocksDB/SQLite open failure |
| `DatabaseOperationFailed(String)` | LongTermStore | Read/write error from storage engine |
| `Core(CoreError)` | All subsystems | Error from Core EventBus or Logger |

**Recovery strategies**:

- Database connection failures during operation are retried with exponential backoff (up to 3 attempts). If all retries fail, the item is queued in a write-behind buffer for later retry.
- Index build failures during startup degrade gracefully: the platform starts with keyword-only search and publishes a warning event. Semantic search is re-enabled when the embedding model loads successfully.
- Partial consolidation failures are logged; successfully promoted items are committed, failed items are left in short-term memory for the next cycle.

---

## Data Flow: Write Path

```
Caller              ShortTermStore         MemoryIndex         LongTermStore
  |                      |                     |                     |
  | insert(item)         |                     |                     |
  |--------------------->|                     |                     |
  |                      | 1. check capacity   |                     |
  |                      | 2. LRU insert       |                     |
  |                      | 3. set accessed_at  |                     |
  |                      |                     |                     |
  |                      | if embedding needed:|                     |
  |                      |   index_item()      |                     |
  |                      |-------------------->|                     |
  |                      |                     | generate embedding  |
  |                      |                     | update inverted idx |
  |                      |<--------------------|                     |
  |                      |                     |                     |
  |                      | if tier == LongTerm:|                     |
  |                      |   put(key, value)   |                     |
  |                      |------------------------------------------>|
  |                      |                     |                     |
  |                      | publish(ItemStored) |                     |
  |<---------------------|                     |                     |
```

## Data Flow: Read Path

```
Caller              MemoryRetriever         MemoryIndex         MemoryStore
  |                      |                     |                     |
  | search(query)        |                     |                     |
  |--------------------->|                     |                     |
  |                      |                     |                     |
  |                      | if semantic:        |                     |
  |                      |   generate emb      |                     |
  |                      |   nearest neighbor  |                     |
  |                      |-------------------->|                     |
  |                      |   candidate keys    |                     |
  |                      |<--------------------|                     |
  |                      |                     |                     |
  |                      | if keyword:         |                     |
  |                      |   tokenize query    |                     |
  |                      |   inverted lookup   |                     |
  |                      |-------------------->|                     |
  |                      |   candidate keys    |                     |
  |                      |<--------------------|                     |
  |                      |                     |                     |
  |                      | merge + deduplicate |                     |
  |                      | apply filters       |                     |
  |                      | score & rank        |                     |
  |                      |                     |                     |
  |                      | for each candidate: |                     |
  |                      |   get(key)          |                     |
  |                      |------------------------------------------>|
  |                      |   MemoryItem        |                     |
  |                      |<------------------------------------------|
  |                      |                     |                     |
  | SearchResult[]       |                     |                     |
  |<---------------------|                     |                     |
```

---

## Future Extensions

- **Distributed Memory**: Shard the long-term store across multiple nodes using consistent hashing. Use Raft for metadata consensus.
- **Memory Graph**: Add entity-relation storage on top of long-term memory, enabling graph traversal queries (pathfinding, connectivity) for the Brain Platform's knowledge graph.
- **Multi-Modal Indexing**: Extend the index to support image embeddings (CLIP), audio embeddings, and cross-modal retrieval.
- **Time-Series Store**: Add a dedicated column family or separate database for time-series data with downsampling and retention policies.
- **Federated Retrieval**: Query multiple remote Memory Platform instances via the EventBus bridge, enabling multi-instance knowledge sharing.
- **Incremental Indexing**: Replace full index rebuilds with incremental index updates for semantic embeddings, reducing consolidation latency.
- **Memory Compression**: Apply delta compression for similar memory items and LZ4/Zstd compression for infrequently accessed long-term entries.
- **Read-Through / Write-Through Cache**: Support optional read-through (fetch from long-term on short-term miss) and write-through (immediate long-term persistence) modes per item.

---

## References

- [Architecture Overview](overview.md)
- [Core Platform Deep Dive](core.md)
- [System Platform Blueprint](system.md)
- [Brain Platform Blueprint](brain.md)
- [Perception Platform Blueprint](perception.md)
- [Design Principles](../principles.md)
- [Roadmap](../roadmap.md)
- RocksDB: https://rocksdb.org/
- Tantivy: https://github.com/quickwit-oss/tantivy
- FastEmbed: https://github.com/Anush008/fastembed-rs
