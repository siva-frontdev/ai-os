# Memory Platform Module

**Module:** `ai_os_memory`
**Status:** Planned (Phase 5)
**Crate:** Not yet created

## Purpose

The Memory Platform module provides the AI-native OS with a dual-store memory architecture inspired by cognitive science. A fast, bounded short-term store handles recent and frequently accessed items, while a persistent long-term store retains structured knowledge across sessions. An indexing and retrieval layer supports semantic and keyword search, and background engines consolidate and prune data to keep the stores within operational limits.

## Status Overview

| Subsystem | Design | Implementation | Tests |
|---|---|---|---|
| ShortTermMemory | Draft | Not started | Not started |
| LongTermStorage | Draft | Not started | Not started |
| MemoryIndex | Draft | Not started | Not started |
| MemoryRetriever | Draft | Not started | Not started |
| ConsolidationEngine | Draft | Not started | Not started |
| PruningEngine | Draft | Not started | Not started |

## Public Interfaces

### `ShortTermMemory`

Planned struct for an in-memory, bounded, TTL-bearing LRU cache.

```rust
/// Planned for Phase 5
pub struct ShortTermMemory<K, V> {
    inner: LruCache<K, Timestamped<V>>,
    ttl: Duration,
    capacity: usize,
}

/// Planned for Phase 5
impl<K: Eq + Hash + Send + Sync, V: Clone + Send + Sync> ShortTermMemory<K, V> {
    pub fn new(capacity: usize, ttl: Duration) -> Self;

    /// Insert an item. Evicts LRU item if at capacity.
    pub fn insert(&mut self, key: K, value: V) -> Option<(K, V)>;

    /// Get an item. Returns None if missing or expired.
    pub fn get(&mut self, key: &K) -> Option<V>;

    /// Remove an item.
    pub fn remove(&mut self, key: &K) -> Option<V>;

    /// Check if an item exists and is fresh (renews TTL probe without access).
    pub fn contains(&mut self, key: &K) -> bool;

    /// Number of live (non-expired) entries.
    pub fn len(&self) -> usize;
}
```

Backed by the `lru` crate. Items are wrapped in `Timestamped<V>` which records `Instant::now()` on insertion. `get` checks `Timestamped.timestamp + TTL > Instant::now()`, returning `None` and evicting silently on expiry.

### `LongTermStorage`

Planned struct for persistent key-value storage with column-family support.

```rust
/// Planned for Phase 5
pub struct LongTermStorage {
    db: Arc<Database>,  // sled or rocksdb handle
}

/// Planned for Phase 5
pub enum ColumnFamily {
    Episodic,
    Semantic,
    Procedural,
    Embeddings,
    Metadata,
}

/// Planned for Phase 5
impl LongTermStorage {
    pub fn open(path: &Path) -> Result<Self, MemoryError>;
    pub fn put(&self, cf: ColumnFamily, key: &[u8], value: &[u8]) -> Result<(), MemoryError>;
    pub fn get(&self, cf: ColumnFamily, key: &[u8]) -> Result<Option<Vec<u8>>, MemoryError>;
    pub fn delete(&self, cf: ColumnFamily, key: &[u8]) -> Result<(), MemoryError>;
    pub fn scan_prefix(&self, cf: ColumnFamily, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, MemoryError>;
    pub fn flush(&self) -> Result<(), MemoryError>;
}
```

Planned impl uses `sled` (pure Rust, lower maintenance burden) or `rusty-level` (LevelDB bindings, higher throughput). Column families are implemented as key-prefix namespaces if the backend lacks native CF support.

### `MemoryIndex`

Planned struct that combines embedding vectors with metadata for retrieval.

```rust
/// Planned for Phase 5
pub struct MemoryIndex {
    embedder: Arc<dyn EmbeddingModel>,
    metadata_store: Arc<LongTermStorage>,
}

/// Planned for Phase 5
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, MemoryError>;
    fn dimension(&self) -> usize;
}

/// Planned for Phase 5
pub struct IndexedItem {
    pub id: u64,
    pub text: String,
    pub embedding: Vec<f32>,
    pub metadata: HashMap<String, String>,
    pub timestamp: SystemTime,
}

impl MemoryIndex {
    pub async fn index(&self, item: IndexedItem) -> Result<(), MemoryError>;
    pub async fn update_metadata(&self, id: u64, metadata: HashMap<String, String>) -> Result<(), MemoryError>;
    pub async fn delete(&self, id: u64) -> Result<(), MemoryError>;
}
```

Planned impl uses `fastembed` (ONNX-based, no Python dependency) for embedding generation. Metadata is stored alongside the embedding in the `Embeddings` column family.

### `MemoryRetriever`

Planned struct for multi-modal search: semantic via cosine similarity, keyword via Tantivy, with hybrid fusion.

```rust
/// Planned for Phase 5
pub struct MemoryRetriever {
    index: Arc<MemoryIndex>,
    keyword_index: Arc<KeywordIndex>,
}

/// Planned for Phase 5
pub enum SearchMode {
    Semantic,
    Keyword,
    Hybrid { alpha: f64 }, // 0.0 = pure keyword, 1.0 = pure semantic
}

/// Planned for Phase 5
pub struct SearchResult {
    pub id: u64,
    pub text: String,
    pub score: f64,
    pub metadata: HashMap<String, String>,
}

impl MemoryRetriever {
    pub async fn search(&self, query: &str, mode: SearchMode, top_k: usize) -> Result<Vec<SearchResult>, MemoryError>;
}
```

Semantic search computes cosine similarity between the query embedding and all indexed embeddings (brute-force for now; ANN via HNSW is a planned optimization). Keyword search delegates to `tantivy` with a RAM-backed index. Hybrid fusion uses reciprocal rank fusion (RRF) to merge results.

### `ConsolidationEngine`

Planned background task that promotes items from short-term to long-term memory.

```rust
/// Planned for Phase 5
pub struct ConsolidationEngine {
    stm: Arc<Mutex<ShortTermMemory<String, Vec<u8>>>>,
    ltm: Arc<LongTermStorage>,
    config: ConsolidationConfig,
}

/// Planned for Phase 5
pub struct ConsolidationConfig {
    pub access_threshold: usize,       // promote after N accesses
    pub recency_bonus: Duration,       // boost items touched within this window
    pub batch_size: usize,
    pub interval: Duration,
}

impl ConsolidationEngine {
    pub fn new(stm: Arc<...>, ltm: Arc<...>, config: ConsolidationConfig) -> Self;
    pub async fn run(&mut self) -> Result<(), MemoryError>;
}
```

The engine runs as a Tokio task. On each tick it scans the STM, computes a promotion score (`access_freq * recency_factor`), and writes items above the threshold into the LTM `Episodic` column family.

### `PruningEngine`

Planned background task that evicts stale or low-importance items from both stores.

```rust
/// Planned for Phase 5
pub struct PruningEngine {
    stm: Arc<Mutex<ShortTermMemory<String, Vec<u8>>>>,
    ltm: Arc<LongTermStorage>,
    config: PruningConfig,
}

/// Planned for Phase 5
pub struct PruningConfig {
    pub stm_ttl: Duration,
    pub ltm_max_items: usize,
    pub ltm_ttl: Duration,
    pub importance_threshold: f64,
    pub interval: Duration,
}

impl PruningEngine {
    pub fn new(stm: Arc<...>, ltm: Arc<...>, config: PruningConfig) -> Self;
    pub async fn run(&mut self) -> Result<(), MemoryError>;
}
```

Pruning strategy:
- **TTL-based**: Remove items whose age exceeds `ltm_ttl` in the LTM.
- **Capacity-based**: When LTM exceeds `ltm_max_items`, remove the oldest 10%.
- **Importance-based**: Items with importance scores below `importance_threshold` (derived from access frequency and consolidation score) are evicted first.

## Dependencies

| Dependency | Scope | Purpose |
|---|---|---|
| `ai_os_core` | internal | EventBus, Service trait, logging |
| `lru` | external | Bounded LRU cache for ShortTermMemory |
| `sled` / `rusty-level` | external | Persistent embedded database |
| `fastembed` | external | ONNX-based text embeddings |
| `tantivy` | external | Full-text keyword indexing and search |
| `serde` | external | Serialization of stored items |
| `bincode` | external | Binary encoding for KV storage |
| `tokio` | runtime | Async background tasks |

## Events Published

| Event | Trigger | Payload |
|---|---|---|
| `memory::ItemPromoted` | `ConsolidationEngine` promotes item to LTM | `(u64, ColumnFamily)` -- id, target CF |
| `memory::ItemPruned` | `PruningEngine` removes an item | `(u64, ColumnFamily, String)` -- id, source, reason |
| `memory::ItemExpired` | STM TTL expiry on access | `(String, String)` -- key, reason |

## Events Consumed

| Event | Consumer | Action |
|---|---|---|
| `core::ServiceStateChanged` | `ConsolidationEngine` | Pause/resume promotion |
| `perception::InputEvent` | `MemoryIndex` | Index incoming textual input |
| `runtime::TaskCompleted` | `ConsolidationEngine` | Consolidate task output into episodic memory |

## Thread Model

- `ShortTermMemory` is wrapped in `Mutex` (tokio or std) because `LruCache` requires `&mut` for both `get` and `insert`. Contention is expected to be low because STM operations are O(1).
- `LongTermStorage` uses `Arc` around a backend handle (sled DB is `Send + Sync`; RocksDB handles are `Send + Sync`). Reads and writes go through the shared reference.
- `MemoryIndex` serializes the embedder behind a `Semaphore` (embedding models are typically single-threaded). Metadata updates are channeled through an `mpsc` to avoid blocking the embedder.
- `ConsolidationEngine` and `PruningEngine` each run on a single Tokio task. No shared mutable state between them.

## Lifecycle

```
Initialization order:
  1. LongTermStorage::open (persistent backend startup)
  2. MemoryIndex::new (load or build index)
  3. ShortTermMemory::new (in-memory only, no persistence)
  4. MemoryRetriever::new (reference index + keyword index)
  5. ConsolidationEngine::run (background task, await on shutdown signal)
  6. PruningEngine::run (background task, await on shutdown signal)
```

Shutdown proceeds in reverse: stop pruning, stop consolidation, flush LTM, close index.

## Error Handling

Error type: `ai_os_memory::MemoryError` with variants:

- `StorageError(String)` -- LTM backend failure (disk full, corruption).
- `SerializationError(String)` -- encode/decode failure.
- `EmbeddingError(String)` -- model load or inference failure.
- `SearchError(String)` -- index corruption or query parse failure.
- `CapacityError(String)` -- both stores saturated and pruning is falling behind.

Recovery: LTM backend errors trigger retry with exponential backoff (up to 3 retries). If the embedder fails, the system degrades to keyword-only search and publishes a `core::HealthStatusChanged` event with `Degraded`. Pruning engine back-pressure is published as a metric but does not block insertions.

## Configuration

```toml
[short_term]
capacity = 10_000
ttl_secs = 300

[long_term]
backend = "sled"                    # "sled" | "rocksdb"
path = "/var/lib/ai-os/memory"
flush_interval_secs = 60

[consolidation]
access_threshold = 3
recency_bonus_secs = 3600
interval_secs = 30

[pruning]
stm_ttl_secs = 300
ltm_max_items = 1_000_000
ltm_ttl_secs = 2_592_000            # 30 days
importance_threshold = 0.1
interval_secs = 120
```

Configuration file: `aios-memory.toml`.

## Testing Strategy

- **Unit tests**: ShortTermMemory TTL expiry, capacity eviction, LRU ordering. LongTermStorage CRUD round-trip with binary payloads. MemoryRetriever hybrid fusion scoring.
- **Integration tests**: ConsolidationEngine end-to-end: insert N items into STM, trigger consolidation, verify items appear in LTM. PruningEngine: fill STM past capacity, verify oldest items evicted.
- **Benchmarks**: STM throughput (ops/sec) at capacity. LTM write throughput with 1 KB values. Embedding latency at batch size 1 vs. 32. Tantivy indexing throughput.
- **Property-based tests**: LongTermStorage key-value round-trip with arbitrary byte vectors. Search result monotonicity (higher `top_k` returns superset of lower `top_k`).

## Future Extensions

- Hierarchical memory: working -> short-term -> long-term with graduated consolidation.
- Emotional valence tagging alongside importance scoring.
- Cross-modal memory (associate embeddings from audio, image, and text).
- Distributed long-term storage via a Raft-backed key-value store.
- Online learning: update embeddings incrementally without full re-index.
- Graph memory: store relations as edges in a persistent graph database (see [Brain](brain.md)).
