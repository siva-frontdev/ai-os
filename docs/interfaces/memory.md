# Memory Platform Interface — `memory` *(Phase 5)*

## Purpose

The Memory layer provides the AI's cognitive memory subsystem. It implements a four-tier memory hierarchy — working, episodic, semantic, and knowledge — with backend-agnostic storage, hybrid search indexing, event-driven learning, and snapshot/restore. All 12 memory crates communicate exclusively through the traits defined here.

---

## Data Model

### Core Types

```rust
use std::ops::Range;
use std::collections::HashMap;

pub type MemoryId = uuid::Uuid;
pub type Timestamp = i64; // UNIX epoch nanoseconds
pub type MemoryResult<T> = Result<T, MemoryError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemorySource {
    User,
    System,
    Agent,
    Derived,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryTier {
    Working,   // volatile, in-memory
    Episodic,  // fast persistent
    Semantic,  // durable persistent
    Archive,   // cold storage
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryType {
    Working,
    Episodic,
    Semantic,
    Knowledge,
}
```

### MemoryObject

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryObject {
    pub id: MemoryId,
    pub version: u64,
    pub timestamp: Timestamp,
    pub created_by: String,
    pub source: MemorySource,
    pub memory_type: MemoryType,
    pub tier: MemoryTier,
    pub content_type: String,
    pub priority: u8,
    pub importance: f32,
    pub confidence: f32,
    pub content: Vec<u8>,
    pub embedding: Option<Vec<f32>>,
    pub tags: Vec<String>,
    pub relationships: Vec<Relationship>,
    pub metadata: HashMap<String, String>,
    pub expiration: Option<Timestamp>,
    pub checksum: [u8; 32],
}
```

### Relationship

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub target_id: MemoryId,
    pub relation_type: RelationType,
    pub weight: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    DerivesFrom,
    References,
    PartOf,
    Sequence,
    Contradicts,
    Supports,
    Custom(String),
}
```

### Query Types

```rust
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortField {
    Timestamp,
    Priority,
    Importance,
    Confidence,
    Version,
    AccessCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_objects: u64,
    pub total_bytes: u64,
    pub tier_counts: HashMap<MemoryTier, u64>,
    pub type_counts: HashMap<MemoryType, u64>,
    pub average_object_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityInfo {
    pub max_entries: usize,
    pub current_entries: usize,
    pub max_bytes: u64,
    pub current_bytes: u64,
}
```

---

## 1. MemoryStore — Storage Abstraction

`crates/memory-storage/` — `ai-os-memory-storage`

```rust
#[async_trait]
pub trait MemoryStore: Debug + Send + Sync {
    // CRUD
    async fn insert(&self, object: MemoryObject) -> MemoryResult<()>;
    async fn get(&self, id: &MemoryId) -> MemoryResult<Option<MemoryObject>>;
    async fn update(&self, object: MemoryObject) -> MemoryResult<()>;
    async fn delete(&self, id: &MemoryId) -> MemoryResult<()>;
    async fn exists(&self, id: &MemoryId) -> MemoryResult<bool>;

    // Batch
    async fn insert_batch(&self, objects: &[MemoryObject]) -> MemoryResult<()>;
    async fn get_batch(&self, ids: &[MemoryId]) -> MemoryResult<Vec<Option<MemoryObject>>>;
    async fn delete_batch(&self, ids: &[MemoryId]) -> MemoryResult<()>;

    // Query
    async fn query(&self, filter: &QueryFilter) -> MemoryResult<Vec<MemoryObject>>;
    async fn count(&self, filter: &QueryFilter) -> MemoryResult<u64>;

    // Management
    async fn flush(&self) -> MemoryResult<()>;
    async fn compact(&self) -> MemoryResult<()>;
    async fn stats(&self) -> MemoryResult<StorageStats>;
}
```

---

## 2. MemoryIndex — Search Index

`crates/memory-index/` — `ai-os-memory-index`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredMemoryId {
    pub id: MemoryId,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_indexed: u64,
    pub dimension: usize,
    pub metric: DistanceMetric,
    pub memory_bytes: u64,
}

#[async_trait]
pub trait MemoryIndex: Debug + Send + Sync {
    async fn index(&self, id: &MemoryId, embedding: &[f32]) -> MemoryResult<()>;
    async fn search(&self, query: &[f32], k: usize) -> MemoryResult<Vec<ScoredMemoryId>>;
    async fn search_with_filter(
        &self,
        query: &[f32],
        filter: &QueryFilter,
        k: usize,
    ) -> MemoryResult<Vec<ScoredMemoryId>>;
    async fn remove(&self, id: &MemoryId) -> MemoryResult<()>;
    async fn rebuild(&self) -> MemoryResult<()>;
    async fn stats(&self) -> MemoryResult<IndexStats>;
    fn dimension(&self) -> usize;
    fn metric(&self) -> DistanceMetric;
}
```

---

## 3. WorkingMemory — Bounded Volatile Store

`crates/memory-working/` — `ai-os-memory-working`

```rust
#[async_trait]
pub trait WorkingMemory: Debug + Send + Sync {
    async fn store(&self, object: MemoryObject) -> MemoryResult<()>;
    async fn recall(&self, id: &MemoryId) -> MemoryResult<Option<MemoryObject>>;
    async fn update(&self, object: MemoryObject) -> MemoryResult<()>;
    async fn forget(&self, id: &MemoryId) -> MemoryResult<()>;
    async fn search(&self, query: &str, k: usize) -> MemoryResult<Vec<MemoryObject>>;
    async fn recent(&self, n: usize) -> MemoryResult<Vec<MemoryObject>>;
    async fn capacity(&self) -> MemoryResult<CapacityInfo>;
    async fn clear(&self) -> MemoryResult<()>;
    async fn stats(&self) -> MemoryResult<StorageStats>;
}
```

---

## 4. EpisodicMemory — Event Sequences

`crates/memory-episodic/` — `ai-os-memory-episodic`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicEvent {
    pub id: MemoryId,
    pub timestamp: Timestamp,
    pub duration: Option<i64>,
    pub event_type: String,
    pub participants: Vec<String>,
    pub location: Option<String>,
    pub sequence_id: Option<MemoryId>,
    pub content: Vec<u8>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicContext {
    pub time_range: Option<Range<Timestamp>>,
    pub event_types: Option<Vec<String>>,
    pub participants: Option<Vec<String>>,
    pub sequence_id: Option<MemoryId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicStats {
    pub total_events: u64,
    pub unique_sequences: u64,
    pub time_span: Option<Range<Timestamp>>,
}

#[async_trait]
pub trait EpisodicMemory: Debug + Send + Sync {
    async fn record(&self, event: EpisodicEvent) -> MemoryResult<MemoryId>;
    async fn recall(&self, id: &MemoryId) -> MemoryResult<Option<EpisodicEvent>>;
    async fn recall_by_time(&self, range: Range<Timestamp>) -> MemoryResult<Vec<EpisodicEvent>>;
    async fn recall_by_context(&self, context: &EpisodicContext) -> MemoryResult<Vec<EpisodicEvent>>;
    async fn replay(&self, id: &MemoryId) -> MemoryResult<Vec<EpisodicEvent>>;
    async fn stats(&self) -> MemoryResult<EpisodicStats>;
}
```

---

## 5. SemanticMemory — Facts and Concepts

`crates/memory-semantic/` — `ai-os-memory-semantic`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub id: MemoryId,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f32,
    pub source_ids: Vec<MemoryId>,
    pub derived_from: Option<MemoryId>,
    pub timestamp: Timestamp,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct FactQuery {
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object: Option<String>,
    pub confidence_min: Option<f32>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticStats {
    pub total_facts: u64,
    pub unique_subjects: u64,
    pub unique_predicates: u64,
    pub average_confidence: f32,
}

#[async_trait]
pub trait SemanticMemory: Debug + Send + Sync {
    async fn store_fact(&self, fact: Fact) -> MemoryResult<MemoryId>;
    async fn recall_fact(&self, id: &MemoryId) -> MemoryResult<Option<Fact>>;
    async fn query_facts(&self, query: &FactQuery) -> MemoryResult<Vec<Fact>>;
    async fn update_fact(&self, fact: Fact) -> MemoryResult<()>;
    async fn retract_fact(&self, id: &MemoryId) -> MemoryResult<()>;
    async fn confidence(&self, id: &MemoryId) -> MemoryResult<f32>;
    async fn stats(&self) -> MemoryResult<SemanticStats>;
}
```

---

## 6. KnowledgeBase — Knowledge Graph

`crates/memory-knowledge/` — `ai-os-memory-knowledge`

```rust
pub type EntityId = MemoryId;
pub type RelationId = MemoryId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub entity_type: String,
    pub properties: HashMap<String, String>,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub id: RelationId,
    pub source_id: EntityId,
    pub target_id: EntityId,
    pub relation_type: String,
    pub weight: f32,
    pub properties: HashMap<String, String>,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeQuery {
    pub source_entity: Option<EntityId>,
    pub relation_type: Option<String>,
    pub target_entity: Option<EntityId>,
    pub max_depth: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeResult {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub paths: Vec<Vec<EntityId>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathSpec {
    pub max_depth: usize,
    pub allowed_relation_types: Option<Vec<String>>,
    pub direction: TraversalDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraversalDirection {
    Forward,
    Backward,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceQuery {
    pub premise_entities: Vec<EntityId>,
    pub target_relation_type: Option<String>,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferredFact {
    pub subject_id: EntityId,
    pub relation_type: String,
    pub object_id: EntityId,
    pub confidence: f32,
    pub derivation_path: Vec<RelationId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeStats {
    pub total_entities: u64,
    pub total_relations: u64,
    pub unique_relation_types: Vec<String>,
    pub unique_entity_types: Vec<String>,
    pub average_degree: f64,
}

#[async_trait]
pub trait KnowledgeBase: Debug + Send + Sync {
    // Entities
    async fn insert_entity(&self, entity: Entity) -> MemoryResult<EntityId>;
    async fn get_entity(&self, id: &EntityId) -> MemoryResult<Option<Entity>>;
    async fn update_entity(&self, entity: Entity) -> MemoryResult<()>;
    async fn delete_entity(&self, id: &EntityId) -> MemoryResult<()>;

    // Relations
    async fn insert_relation(&self, relation: Relation) -> MemoryResult<RelationId>;
    async fn get_relations(&self, entity_id: &EntityId) -> MemoryResult<Vec<Relation>>;
    async fn delete_relation(&self, id: &RelationId) -> MemoryResult<()>;

    // Query
    async fn query(&self, query: &KnowledgeQuery) -> MemoryResult<KnowledgeResult>;
    async fn traverse(&self, start: &EntityId, spec: &PathSpec) -> MemoryResult<Vec<Entity>>;

    // Inference
    async fn infer(&self, query: &InferenceQuery) -> MemoryResult<Vec<InferredFact>>;

    // Management
    async fn stats(&self) -> MemoryResult<KnowledgeStats>;
}
```

---

## 7. ContextManager — Scope and Hierarchy

`crates/memory-context/` — `ai-os-memory-context`

```rust
pub type ContextId = uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub context_id: ContextId,
    pub parent_id: Option<ContextId>,
    pub values: HashMap<String, Vec<u8>>,
    pub created: Timestamp,
}

#[async_trait]
pub trait ContextManager: Debug + Send + Sync {
    async fn create_context(&self, parent: Option<ContextId>) -> MemoryResult<ContextId>;
    async fn destroy_context(&self, id: &ContextId) -> MemoryResult<()>;
    async fn set_value(&self, ctx: &ContextId, key: &str, value: Vec<u8>) -> MemoryResult<()>;
    async fn get_value(&self, ctx: &ContextId, key: &str) -> MemoryResult<Option<Vec<u8>>>;
    async fn delete_value(&self, ctx: &ContextId, key: &str) -> MemoryResult<()>;
    async fn snapshot(&self, ctx: &ContextId) -> MemoryResult<ContextSnapshot>;
    async fn restore(&self, snapshot: &ContextSnapshot) -> MemoryResult<ContextId>;
    async fn merge(&self, target: &ContextId, source: &ContextId) -> MemoryResult<()>;
    async fn active_contexts(&self) -> MemoryResult<Vec<ContextId>>;
    async fn stats(&self) -> MemoryResult<ContextManagerStats>;
}
```

---

## 8. MemoryCache — Hot Cache Layer

`crates/memory-cache/` — `ai-os-memory-cache`

```rust
pub type CacheKey = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEntry {
    pub object: MemoryObject,
    pub cached_at: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub access_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_entries: usize,
    pub max_entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub hit_rate: f64,
}

#[async_trait]
pub trait MemoryCache: Debug + Send + Sync {
    async fn get(&self, key: &CacheKey) -> MemoryResult<Option<CachedEntry>>;
    async fn set(&self, key: CacheKey, object: MemoryObject, ttl: Option<std::time::Duration>) -> MemoryResult<()>;
    async fn remove(&self, key: &CacheKey) -> MemoryResult<()>;
    async fn invalidate(&self, pattern: &str) -> MemoryResult<()>;
    async fn clear(&self) -> MemoryResult<()>;
    async fn stats(&self) -> MemoryResult<CacheStats>;
}
```

---

## 9. MemoryRetriever — Hybrid Retrieval Engine

`crates/memory-retrieval/` — `ai-os-memory-retrieval`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalQuery {
    pub text: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub filters: QueryFilter,
    pub strategy: RetrievalStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrievalStrategy {
    EmbeddingFirst,
    KeywordFirst,
    KnowledgeFirst,
    Semantic,
    Episodic,
    Hybrid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RetrievalWeights {
    pub embedding: f32,
    pub keyword: f32,
    pub knowledge: f32,
    pub recency: f32,
    pub importance: f32,
}

impl Default for RetrievalWeights {
    fn default() -> Self {
        Self {
            embedding: 0.4,
            keyword: 0.3,
            knowledge: 0.2,
            recency: 0.05,
            importance: 0.05,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    pub object: MemoryObject,
    pub score: f32,
    pub contributors: Vec<RetrievalContributor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalContributor {
    pub source: ContributorSource,
    pub score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContributorSource {
    Embedding,
    Keyword,
    Knowledge,
    Recency,
    Importance,
    Context,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalExplanation {
    pub query: RetrievalQuery,
    pub weights: RetrievalWeights,
    pub steps: Vec<RetrievalStep>,
    pub total_elapsed: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalStep {
    pub description: String,
    pub elapsed: std::time::Duration,
    pub candidates_before: usize,
    pub candidates_after: usize,
}

#[async_trait]
pub trait MemoryRetriever: Debug + Send + Sync {
    async fn retrieve(&self, query: &RetrievalQuery) -> MemoryResult<RetrievalResult>;
    async fn retrieve_multi(
        &self,
        query: &RetrievalQuery,
        n: usize,
    ) -> MemoryResult<Vec<RetrievalResult>>;
    async fn hybrid_search(
        &self,
        text: &str,
        embedding: &[f32],
        k: usize,
        filters: &QueryFilter,
    ) -> MemoryResult<Vec<ScoredMemoryId>>;
    async fn rerank(
        &self,
        results: &[RetrievalResult],
        weights: &RetrievalWeights,
    ) -> MemoryResult<Vec<RetrievalResult>>;
    async fn explain(
        &self,
        result: &RetrievalResult,
    ) -> MemoryResult<RetrievalExplanation>;
}
```

---

## 10. SnapshotManager — Checkpoint and Restore

`crates/memory-snapshot/` — `ai-os-memory-snapshot`

```rust
pub type SnapshotId = uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotScope {
    pub include_working: bool,
    pub include_episodic: bool,
    pub include_semantic: bool,
    pub include_knowledge: bool,
    pub include_index: bool,
    pub include_context: bool,
    pub time_range: Option<Range<Timestamp>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestoreStrategy {
    Replace,
    Merge,
    MergeKeepExisting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub id: SnapshotId,
    pub created: Timestamp,
    pub size_bytes: u64,
    pub scope: SnapshotScope,
    pub object_count: u64,
    pub checksum: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPolicy {
    pub interval: Option<std::time::Duration>,
    pub scope: SnapshotScope,
    pub max_snapshots: u32,
    pub on_change: bool,
}

#[async_trait]
pub trait SnapshotManager: Debug + Send + Sync {
    async fn create_snapshot(&self, scope: &SnapshotScope) -> MemoryResult<SnapshotId>;
    async fn restore_snapshot(
        &self,
        id: &SnapshotId,
        strategy: RestoreStrategy,
    ) -> MemoryResult<()>;
    async fn delete_snapshot(&self, id: &SnapshotId) -> MemoryResult<()>;
    async fn list_snapshots(&self) -> MemoryResult<Vec<SnapshotMetadata>>;
    async fn snapshot_info(&self, id: &SnapshotId) -> MemoryResult<SnapshotMetadata>;
    async fn export_snapshot(&self, id: &SnapshotId, path: &std::path::Path) -> MemoryResult<()>;
    async fn import_snapshot(&self, path: &std::path::Path) -> MemoryResult<SnapshotId>;
    async fn schedule_snapshot(&self, policy: SnapshotPolicy) -> MemoryResult<()>;
}
```

---

## 11. LearningManager — Consolidation, Pruning, Discovery

`crates/memory-learning/` — `ai-os-memory-learning`

```rust
pub type PatternId = uuid::Uuid;

// -- Consolidation --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationPolicy {
    pub target_tier: MemoryTier,
    pub importance_threshold: f32,
    pub confidence_threshold: f32,
    pub access_count_threshold: u64,
    pub age_threshold: std::time::Duration,
    pub batch_size: usize,
    pub interval: Option<std::time::Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationReport {
    pub total_attempted: usize,
    pub total_succeeded: usize,
    pub total_failed: usize,
    pub entries: Vec<ConsolidationEntry>,
    pub elapsed: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationEntry {
    pub source_id: MemoryId,
    pub target_id: Option<MemoryId>,
    pub from_tier: MemoryTier,
    pub to_tier: MemoryTier,
    pub status: ConsolidationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsolidationStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Skipped(String),
}

// -- Pruning --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningPolicy {
    pub target_tier: MemoryTier,
    pub max_objects: Option<u64>,
    pub max_bytes: Option<u64>,
    pub importance_threshold: f32,
    pub age_threshold: std::time::Duration,
    pub confidence_threshold: f32,
    pub excluded_types: Vec<MemoryType>,
    pub excluded_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningReport {
    pub total_removed: usize,
    pub bytes_freed: u64,
    pub tier: MemoryTier,
    pub policy: PruningPolicy,
    pub entries: Vec<PrunedEntry>,
    pub elapsed: std::time::Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrunedEntry {
    pub id: MemoryId,
    pub reason: PruneReason,
    pub importance: f32,
    pub age: std::time::Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PruneReason {
    Expired,
    LowImportance,
    LowConfidence,
    CapacityExceeded,
    ExplicitForget,
}

// -- Pattern Discovery --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub pattern_types: Vec<PatternType>,
    pub min_support: u64,
    pub min_confidence: f32,
    pub time_window: std::time::Duration,
    pub batch_size: usize,
    pub interval: Option<std::time::Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternType {
    Temporal,
    Sequential,
    Associative,
    Causal,
    Categorical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternCondition {
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAction {
    pub action_type: String,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub id: PatternId,
    pub pattern_type: PatternType,
    pub confidence: f32,
    pub support: u64,
    pub description: String,
    pub conditions: Vec<PatternCondition>,
    pub actions: Vec<PatternAction>,
    pub created: Timestamp,
    pub last_matched: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStats {
    pub consolidation: ConsolidationStats,
    pub pruning: PruningStats,
    pub patterns: PatternStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationStats {
    pub total_consolidated: u64,
    pub last_run: Option<Timestamp>,
    pub average_batch_size: f64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningStats {
    pub total_pruned: u64,
    pub last_run: Option<Timestamp>,
    pub bytes_freed_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStats {
    pub total_patterns: u64,
    pub by_type: HashMap<PatternType, u64>,
    pub last_discovery: Option<Timestamp>,
}

#[async_trait]
pub trait LearningManager: Debug + Send + Sync {
    // Consolidation
    async fn consolidate(&self, id: &MemoryId, target_tier: MemoryTier) -> MemoryResult<ConsolidationStatus>;
    async fn consolidate_batch(
        &self,
        ids: &[MemoryId],
        target_tier: MemoryTier,
    ) -> MemoryResult<Vec<ConsolidationEntry>>;
    async fn consolidate_by_policy(
        &self,
        policy: &ConsolidationPolicy,
    ) -> MemoryResult<ConsolidationReport>;
    async fn consolidation_status(&self, id: &MemoryId) -> MemoryResult<ConsolidationStatus>;

    // Pruning
    async fn prune(&self, policy: &PruningPolicy) -> MemoryResult<PruningReport>;
    async fn dry_run(&self, policy: &PruningPolicy) -> MemoryResult<PruningReport>;
    async fn set_pruning_policy(&self, policy: PruningPolicy) -> MemoryResult<()>;
    async fn get_pruning_policy(&self) -> MemoryResult<PruningPolicy>;

    // Pattern discovery
    async fn discover_patterns(&self, config: &DiscoveryConfig) -> MemoryResult<Vec<Pattern>>;
    async fn get_pattern(&self, id: &PatternId) -> MemoryResult<Option<Pattern>>;
    async fn apply_pattern(&self, pattern: &Pattern) -> MemoryResult<()>;

    // Control
    async fn pause(&self) -> MemoryResult<()>;
    async fn resume(&self) -> MemoryResult<()>;
    async fn stats(&self) -> MemoryResult<LearningStats>;
}
```

---

## Error Model

`MemoryError` is the unified error type for all Memory Platform operations:

```rust
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("object not found: {0}")]
    ObjectNotFound(MemoryId),

    #[error("tier {0} is full: {1} bytes used of {2}")]
    TierFull(MemoryTier, u64, u64),

    #[error("capacity exceeded for {0}: {1} used of {2}")]
    CapacityExceeded(String, u64, u64),

    #[error("invalid query: {0}")]
    InvalidQuery(String),

    #[error("index build failed: {0}")]
    IndexBuildFailed(String),

    #[error("search failed: {0}")]
    SearchFailed(String),

    #[error("consolidation failed for {0}: {1}")]
    ConsolidationFailed(MemoryId, String),

    #[error("pruning failed: {0}")]
    PruningFailed(String),

    #[error("snapshot failed: {0}")]
    SnapshotFailed(String),

    #[error("pattern discovery failed: {0}")]
    PatternDiscoveryFailed(String),

    #[error("context not found: {0}")]
    ContextNotFound(ContextId),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("storage backend error: {0}")]
    StorageBackendError(String),

    #[error("permission denied: {session} cannot access {resource}")]
    PermissionDenied { session: String, resource: String },

    #[error("operation timed out after {0}ms")]
    Timeout(u64),
}
```

---

## Events

### Published Events

| Event Type | Payload | Source |
|---|---|---|
| `memory.object.stored` | `{ id, memory_type, tier, timestamp }` | WorkingMemory |
| `memory.object.recalled` | `{ id, memory_type, tier, latency_us, cache_hit }` | Retrieval |
| `memory.object.updated` | `{ id, new_version, timestamp }` | Any store |
| `memory.object.deleted` | `{ id, memory_type, tier }` | Any store |
| `memory.consolidated` | `{ id, from_tier, to_tier, new_id }` | LearningManager |
| `memory.consolidation.failed` | `{ id, from_tier, reason }` | LearningManager |
| `memory.pruned` | `{ ids_removed, bytes_freed, tier, policy }` | LearningManager |
| `memory.context.created` | `{ context_id, parent_id, session_id }` | ContextManager |
| `memory.context.destroyed` | `{ context_id }` | ContextManager |
| `memory.snapshot.created` | `{ snapshot_id, size_bytes, scope }` | SnapshotManager |
| `memory.snapshot.restored` | `{ snapshot_id, timestamp }` | SnapshotManager |
| `memory.pattern.discovered` | `{ pattern_id, pattern_type, confidence, support }` | LearningManager |
| `memory.tier.capacity_warning` | `{ tier, usage_pct, current_bytes, max_bytes }` | Storage |
| `memory.index.rebuilt` | `{ entries_indexed, elapsed_ms, dimension }` | Index |
| `memory.cache.eviction` | `{ key, reason }` | Cache |

### Consumed Events

| Source | Event | Handling |
|---|---|---|
| Core | `core.config_changed` | Reload memory policies |
| Runtime | `runtime.session.destroyed` | Prune session-scoped memory |
| Runtime | `runtime.task.completed` | Analyze for pattern discovery |

---

## Performance Targets

| Operation | Target Latency (p99) | Throughput |
|-----------|---------------------|------------|
| WorkingMemory store/recall | < 1 µs | 1M+/s |
| MemoryStore insert (persistent) | < 100 µs | 50K+/s |
| MemoryIndex search (10K entries) | < 200 µs | 5K+/s |
| MemoryIndex search (1M entries) | < 5 ms | 200+/s |
| MemoryRetriever hybrid query | < 10 ms | 100+/s |
| Consolidation (single entry) | < 10 ms | 100+/s |
| Pruning dry-run (10K entries) | < 50 ms | N/A |

---

## Dependencies

| Crate | Depends On |
|---|---|
| `ai-os-memory-core` | `ai-os-core` |
| `ai-os-memory-storage` | `memory-core`, `ai-os-core` |
| `ai-os-memory-index` | `memory-core`, `ai-os-core` |
| `ai-os-memory-working` | `memory-core`, `ai-os-core` |
| `ai-os-memory-episodic` | `memory-core`, `memory-storage`, `ai-os-core` |
| `ai-os-memory-semantic` | `memory-core`, `memory-storage`, `ai-os-core` |
| `ai-os-memory-knowledge` | `memory-core`, `memory-storage`, `ai-os-core` |
| `ai-os-memory-context` | `memory-core`, `ai-os-core`, `ai-os-runtime` |
| `ai-os-memory-cache` | `memory-core`, `memory-working`, `ai-os-core` |
| `ai-os-memory-retrieval` | `memory-core`, `memory-index`, `memory-storage`, `memory-cache`, `ai-os-core`, `ai-os-runtime` |
| `ai-os-memory-snapshot` | `memory-core`, `memory-storage`, `memory-index`, `ai-os-core` |
| `ai-os-memory-learning` | `memory-core`, `memory-storage`, `memory-index`, `memory-retrieval`, `ai-os-core`, `ai-os-runtime` |
