# RFC-0008: LIFE World Store

| Field | Value |
|---|---|
| **Status** | Accepted |
| **Author** | LIFE-OS Architecture Team |
| **Phase** | LIFE (World Store) |
| **Created** | 2026-08-08 |
| **Updated** | 2026-08-08 |
| **Requires** | RFC-0002 (Memory Platform), ADR-0006 (World Model), ADR-0007 (Continuous Cognitive Loop) |
| **Supersedes** | None |

## Abstract

This RFC freezes the architecture of the **LIFE World Store** — the single durable substrate
that both the user (via LIFE Open Space) and the AI (via the existing cognitive loop) operate
on. It makes the existing in-memory World Model (`memory_core::wm`) durable by implementing a
SQLite backend **behind the existing storage abstractions**, without changing the cognitive or
runtime architecture. It defines the canonical entity graph, document storage, observation
persistence, provenance, versioning, optimistic concurrency, soft deletion, search, migration,
backup/restore, and the future path to multi-device synchronization.

Source material: `docs/architecture/life-world-store-audit.md` and
`docs/architecture/life-world-store-schema.md`.

## Motivation

### Problem

1. **The only durable world data today is `world_model.json`** — a full-graph pretty-JSON
   rewrite, auto-saved every 5 minutes and at shutdown. A crash can lose up to 5 minutes of
   state; there is no WAL, journal, or incremental persistence.
2. **Observations disappear.** The LLM `StructuredWorldUpdate` is consumed transiently;
   `new_observations`, `state_changes`, `open_questions` are never stored. Provenance cannot
   be reconstructed later.
3. **Goals and lessons disappear on restart.** `GoalStore`, `LessonStore`, and learning state
   are in-memory only, despite being serde-ready and despite a `RecoveryManager` designed to
   resume interrupted goals.
4. **No document storage exists.** No originals, no metadata, no extracted text, no
   document↔entity links.
5. **No provenance, no history, no versioning.** `Entity.version` exists but updates overwrite
   in place; there is no change history and no record of *who* changed *what* and *why*.
6. **No concurrency control.** The AI loop and user edits share one in-memory store; there is
   no conflict detection.
7. **LIFE Open Space needs CRUD, search, filter, relate, view, visualize** on the same data the
   AI maintains. Today the data is a JSON file that cannot serve those needs.

### What Happens If We Do Nothing

LIFE Open Space would be forced to create a **second, competing persistence system** (e.g. a
database with its own schema) that drifts from the AI's world model. This violates the
platform's core tenet — *"AI maintains the user's world + User can directly inspect/edit/
manage the same world"* — and duplicates every data model identified in the audit
(people, companies, goals, knowledge, observations).

## Design

### Overview

```
                    ┌──────────────────────────────────────────────┐
                    │           LIFE WORLD STORE (SQLite)          │
                    │                                              │
   AI (cognitive    │  entities ◄─ relationships                   │
   loop / evolution)│  observations ── entity_observations         │
        │           │  documents + document_content + links        │
        │           │  goals / lessons  (operational stores)       │
        │           │  entity_history / relationship_history       │
        ▼           │  FTS5: entities_fts / document_fts           │
  WorldStore trait  │                                              │
        ▲           └──────────────────────────────────────────────┘
        │                           │
   User (Open Space /              │
   companion UI)                   ▼
                      World Model (in-memory view
                      maintained by the cognitive loop)
```

The **World Store** is a durable service implementing the extended `WorldModelStore` /
`MemoryStore` abstractions. Both the AI (cognitive loop) and the user (Open Space) speak to
it through the **same `WorldStore` API** (see §Interfaces). The in-memory
`InMemoryWorldModelStore` remains the hot working set used by the cognitive loop; the SQLite
store is the durable canonical substrate that it loads from and flushes to.

### Problem / Goals / Non-goals

**Goals**
- Make the World Model durable behind existing abstractions (no cognitive rewrite).
- Single canonical store for entities, relationships, observations, documents, goals.
- Provenance, version history, optimistic concurrency, soft deletion on every write.
- Full-text search + filtered search.
- Safe, additive, idempotent migration from `world_model.json`.
- Backup/restore/verification.
- An entity model that supports future server + multi-device synchronization **without
  redesign**.
- A contractor can implement the whole store from this RFC + schema without further
  architectural decisions.

**Non-goals** (explicitly out of scope for this RFC)
- Implementing the store (that is milestone M3).
- Building Open Space UI (M4+).
- Retiring `brain_core::model::WorldModel` (deferred; kept for compatibility).
- Vector/embedding search (FTS5 first; vector is a later RFC).
- Implementing synchronization (design-compatible only, §Future Synchronization).
- Migrating data at RFC time (migration procedure is specified; execution is M3).
- Encrypting the world store at rest (backup encryption is covered; full at-rest
  encryption is a follow-up).

### Current Architecture

- Canonical in-memory model: `memory_core::wm::Entity`, `Relationship`, `Value` (property
  graph; no fixed categories; open `entity_type` strings).
- Canonical in-memory store contract: `WorldModelStore` (18 methods) implemented by
  `InMemoryWorldModelStore` (4 `RwLock<HashMap>` + adjacency indexes).
- Only durable world data: `PersistenceManager` writing `world_model.json`
  (`{entities, relationships, saved_at}`), atomic temp+rename, 5-min auto-save + on stop.
- Cognitive loop writes via `EvolutionEngine`; user writes via companion UI
  (`/api/forget|correct|merge`).
- Goals: `GoalStore` trait → `InMemoryGoalStore` (in-memory, lost on restart).
- Lessons: `LessonStore` trait → in-memory.
- Documents: none. Search: name/type/neighbor only. Indexes/transactions: no-op stubs.
- SQLite: not present anywhere in the workspace.

### Target Architecture

See Overview diagram. The World Store is a new crate/service in the Memory layer
(`crates/life-world-store`), depending on `memory-core` types and implementing the
extended `WorldModelStore`/`MemoryStore` traits. All existing cognitive/runtime/companion
code keeps its current shape and call patterns; only the durable backend changes beneath
them, plus the new World Store API surface used by Open Space.

### Canonical World Model

- **`memory_core::wm` is the canonical model** (ADR-0008, Decision 1).
- Entities and relationships are property-graph nodes/edges; `Value` covers
  String/Number/Boolean/Timestamp/EntityId/Array/Map/Null.
- **Typed entity catalogue** replaces free-form `entity_type` strings for new writes
  (person, company, project, goal, mission, job, lead, contact, document, knowledge,
  concept, observation, event, user, …). Original free-form values are preserved in
  `metadata.original_type` (lossless normalization).
- `brain_core::model::WorldModel` (fixed schema) remains temporarily for compatibility and
  is **not** the canonical long-term model (ADR-0008, Decision 9).

### World Store Responsibilities

- Canonical durable home for: entities, relationships, observations, documents, goals,
  lessons, history, search indexes.
- Single write path with provenance, versioning, concurrency, and audit hooks.
- Single read path: `get`, `search`, `list`, `history`, graph traversal.
- Migration import/export and backup/restore.

### SQLite Responsibilities

- Durable storage engine behind the `WorldStore` API (not a schema owner).
- ACID transactions (replaces the no-op `Transaction` trait).
- `FTS5` full-text indexes for entities and documents.
- JSON columns (`properties`, `metadata`) using existing `Value` semantics.
- WAL mode; single-file database at `~/.local/share/life-os/world/life-world.db`.

### Document Storage Responsibilities

- Original file → checksummed blob under `~/.local/share/life-os/documents/`.
- Metadata + extracted text → SQLite (`documents`, `document_content`, `document_entities`).
- Documents are **also entities** (`entity_type='document'`, same UUID), so they are
  searchable/relatable/visualizable like any other world node.
- Soft deletion follows the global lifecycle; original file is retained until hard delete.

### AI Access

The AI (cognitive loop) writes to the World Store through the same `WorldStore` API,
carrying provenance `actor=ai` + the observation that justified the write. The loop's
`EvolutionEngine` continues to merge by name, adjust confidence/importance, and create
entities/relationships — now durably and transactionally.

### User Access

The user writes through Open Space (and the existing companion memory controls), carrying
provenance `actor=user`. Every user write goes through the same API with the same
authorization (`capability-policy` gate + session role), concurrency, and audit rules as AI
writes.

### Observation Persistence

- `record_observation` stores raw observations durably (`observations` table).
- The understanding pipeline marks them `processed` (raw → understood → evolved) and links
  them to the entities they produced (`entity_observations`).
- This gives end-to-end provenance: *which observation led to which entity change*.

### Goal/Mission Persistence

- **Goals**: dedicated `goals` table mirrors `GoalRecord` (operational data for the
  cognitive engine: hierarchy, objectives, dependencies, retry, progress). Each goal is also
  a graph node (`entity_type='goal'`, same UUID) so goals participate in relationships and
  appear in Open Space. `GoalStore` trait is implemented over the durable table.
- **Missions**: entities (`entity_type='mission'`) with operational detail in properties. A
  dedicated `missions` table is deferred until a MissionManager engine exists.

### Provenance

Every entity/relationship/document row carries `created_by` (actor tag: `ai`/`user`/
`observation`/`runtime`/`migration`) and `provenance` (observation id, user action id,
source URI). Every AI write is linked to its justifying observation.

### Versioning

- Monotonic `version` per row, incremented on every successful write.
- Append-only `entity_history` / `relationship_history` store a snapshot + operation per
  version (`create|update|archive|restore|delete`).
- `get_history(id)` returns the ordered change log with who/when/why.

### Audit/History

- The existing companion `AuditLog` is retained and extended with new categories
  (`WorldStoreWrite`, `DocumentUpload`, `Restore`, `Migration`).
- Every World Store mutation appends an audit entry referencing the same `audit_id` recorded
  in the history table.

### Optimistic Concurrency

- `update`/`archive`/`restore`/`delete` require `expected_version` (`if-match`).
- A mismatch returns `VersionConflict { id, expected, actual }`; the writer re-reads and
  re-merges (AI) or reloads/forces (user). No lost updates.

### Soft Deletion

- `archive_entity`/`archive_document` → `lifecycle='Archived'` (normal deletion).
- `restore_entity` → back to `Active`.
- `delete_entity` is soft by default (archive + retain history); hard delete is an explicit
  privileged operation that also removes history (never the default path).

### Search

- `search_entities(query)` → FTS5 over name/type/properties-text with entity-type and
  lifecycle filters, pagination, sort.
- `search_documents(query)` → FTS5 over title + extracted text.
- Graph traversal remains via `get_neighbors`-style queries on the relationship table.

### Full-text Search

- FTS5 virtual tables `entities_fts` and `document_fts`, kept in sync transactionally with
  writes. Chinese/other-language tokenization is a build-time FTS5 configuration concern.

### Relationships

- Directed edges with type, confidence, weight, version, provenance.
- Enforced referential integrity to entities (RESTRICT on hard delete of referenced
  entities; soft delete does not break edges).
- Traversal indexes on `source_id` / `target_id`.

### Migration Strategy

From `world_model.json` to SQLite. Requirements (mandated):

1. **Additive** — never deletes or rewrites the source file; old file preserved.
2. **Idempotent** — re-running the migration is safe (skip/upsert by existing UUID).
3. **No data loss** — every entity and relationship is copied.
4. **Preserve UUIDs** — entity/relationship IDs are copied verbatim.
5. **Preserve properties, confidence, importance, lifecycle, timestamps** — copied verbatim.
6. **Provenance** — migrated rows carry `created_by='migration'`,
   `provenance='migration:world_model.json'`.
7. **Retain original JSON** — the source file is copied to a timestamped backup before
   import.
8. **Type normalization** — `entity_type` mapped to the catalogue where a mapping exists;
   `metadata.original_type` preserves the original string.
9. Run once at first boot of the new store; result logged to audit.

### Backup/Restore

- **Backup**: SQLite `VACUUM INTO` (consistent snapshot) → `life-world-<ts>.db`, plus a
  manifest with SHA-256 checksums of the DB and the document blob directory.
- **Restore**: verify checksum → `integrity_check` → replace DB → verify counts.
- **Recovery**: WAL recovery on open; periodic snapshots; `world_model.json` export remains
  as an additional fallback.
- Implemented behind the existing `PrivacyManager` backup/restore surface (extended to the
  world store).

### Future Multi-Device / Server Synchronization

Design must not require a later entity-model redesign:

- Immutable UUIDs, monotonic `version`, `updated_at`, `lifecycle` (with tombstones for
  deletes) — all already present in the schema.
- The World Store API is the single integration point; a sync layer (push/pull of deltas,
  or a server-side SQLite + remote API) can be added on top.
- **Explicitly not implemented in this RFC.** No sync, no server, no conflict-merge policy
  (future RFC).

### Security

- Authorization: all World Store writes are gated by the `capability-policy` gate (AI) and
  session roles (user). Read of sensitive entity types is permission-checked.
- Input validation: `entity_type` (catalogue), length limits, `Value` JSON parse, path
  canonicalization for document blobs (no traversal).
- Secrets never enter the world store.
- Backup files are checksummed; optional AES-GCM encryption via existing `PrivacyManager`
  (key stays out of the DB).
- Audit of every write (who/what/when/why).

### Failure Recovery

- SQLite WAL: crash-safe; auto-recovery on open.
- Atomic writes: single transactions; partial failures roll back.
- History is append-only: a corrupt write can be inspected/rolled back via history.
- Backup/restore path for catastrophic loss.
- The companion keeps its crash-safe 5-min save pattern for the in-memory hot set during
  transition.

### Performance Expectations

| Operation | Target (personal scale) |
|---|---|
| `get_entity` | < 1 ms p50 |
| `update_entity` (single tx) | < 2 ms p50 |
| `search_entities` (FTS5, filtered) | < 10 ms p50 |
| `insert_batch` (1000 entities) | < 100 ms |
| Graph traversal (1-hop neighbors) | < 1 ms p50 |
| Full-graph load (startup) | < 200 ms |
| Write amplification | history append is O(1) per write; no full-graph rewrites |
| Disk footprint | target < 50 MB per 10k entities + relationships + history |

Criterion benchmarks will be added for the hot paths (see §Testing Strategy).

### Interfaces

```rust
// crate: crates/life-world-store (new), depends on ai-os-memory-core types.

/// Who is performing the write and what justifies it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteContext {
    pub actor: Actor,            // Ai | User { user_id } | Observation | Runtime | Migration
    pub provenance: Option<String>, // observation id / user action id / source uri
}

pub struct CreateEntity {
    pub entity_type: String,       // typed catalogue
    pub name: String,
    pub properties: HashMap<String, Value>,
    pub importance: f32,
    pub confidence: f32,
    pub metadata: HashMap<String, String>,
    pub ctx: WriteContext,
}

pub struct UpdateEntity {
    pub id: EntityId,
    pub expected_version: u64,     // optimistic concurrency token
    pub name: Option<String>,
    pub properties: Option<HashMap<String, Value>>,
    pub importance: Option<f32>,
    pub confidence: Option<f32>,
    pub metadata: Option<HashMap<String, String>>,
    pub ctx: WriteContext,
}

pub trait WorldStore: Send + Sync + Debug {
    // ---- Entities ----
    async fn create_entity(&self, req: CreateEntity) -> Result<Entity, WorldStoreError>;
    async fn get_entity(&self, id: &EntityId) -> Result<Option<Entity>, WorldStoreError>;
    async fn update_entity(&self, req: UpdateEntity) -> Result<Entity, WorldStoreError>;
    async fn archive_entity(&self, id: &EntityId, expected_version: u64, reason: &str, ctx: &WriteContext) -> Result<(), WorldStoreError>;
    async fn restore_entity(&self, id: &EntityId, expected_version: u64, ctx: &WriteContext) -> Result<(), WorldStoreError>;
    async fn delete_entity(&self, id: &EntityId, expected_version: u64, hard: bool, ctx: &WriteContext) -> Result<(), WorldStoreError>;

    // ---- Relationships ----
    async fn create_relationship(&self, req: CreateRelationship) -> Result<Relationship, WorldStoreError>;
    async fn get_relationship(&self, id: &RelationshipId) -> Result<Option<Relationship>, WorldStoreError>;
    async fn update_relationship(&self, req: UpdateRelationship) -> Result<Relationship, WorldStoreError>;
    async fn delete_relationship(&self, id: &RelationshipId, expected_version: u64, ctx: &WriteContext) -> Result<(), WorldStoreError>;

    // ---- Search ----
    async fn search_entities(&self, query: &SearchQuery) -> Result<Vec<Entity>, WorldStoreError>;
    async fn search_documents(&self, query: &SearchQuery) -> Result<Vec<Document>, WorldStoreError>;

    // ---- Observations ----
    async fn record_observation(&self, obs: Observation) -> Result<ObservationId, WorldStoreError>;
    async fn get_observation(&self, id: &ObservationId) -> Result<Option<Observation>, WorldStoreError>;

    // ---- History ----
    async fn get_history(&self, id: &EntityId) -> Result<Vec<HistoryEntry>, WorldStoreError>;

    // ---- Documents ----
    async fn create_document(&self, req: CreateDocument) -> Result<Document, WorldStoreError>;
    async fn get_document(&self, id: &DocumentId) -> Result<Option<Document>, WorldStoreError>;
    async fn update_document(&self, req: UpdateDocument) -> Result<Document, WorldStoreError>;
    async fn archive_document(&self, id: &DocumentId, expected_version: u64, ctx: &WriteContext) -> Result<(), WorldStoreError>;
}
```

`SearchQuery { text: Option<String>, entity_type: Option<&str>, lifecycle: Option<LifecycleFilter>,
tags: Option<Vec<&str>>, limit: u32, offset: u32, sort: SortBy }`.

`WorldStoreError` variants include `VersionConflict { id, expected, actual }`,
`EntityNotFound`, `InvalidEntityType`, `Validation(String)`, `Storage(String)`,
`NotAuthorized`, `Conflict`, `Io(String)`.

#### Events

The World Store publishes lifecycle/change events on the EventBus for downstream subscribers
(Open Space live views, notifications):

| Direction | Event Type | Payload | Description |
|---|---|---|---|
| Published | `worldstore.entity_created` | `EntityCreated { id, entity_type }` | Emitted after a successful create |
| Published | `worldstore.entity_updated` | `EntityUpdated { id, version }` | Emitted after a successful update |
| Published | `worldstore.entity_archived` | `EntityArchived { id, reason }` | Soft delete |
| Published | `worldstore.entity_restored` | `EntityRestored { id }` | Restore |
| Published | `worldstore.relationship_created` | `RelationshipCreated { id, source, target }` | Edge added |
| Published | `worldstore.observation_recorded` | `ObservationRecorded { id, kind }` | Observation ingested |
| Published | `worldstore.document_created` | `DocumentCreated { id }` | Document added |

#### Dependencies

| Dependency | Layer | Purpose |
|---|---|---|
| `ai-os-memory-core` | Memory | `Entity`, `Relationship`, `Value`, `EntityId`, timestamps |
| `ai-os-memory-storage` | Memory | Extended `WorldModelStore` trait, `StorageBackend` |
| `ai-os-core` | Core | EventBus, error patterns |
| `rusqlite` (bundled) | External | SQLite driver — **new dependency, to be approved per AGENTS.md §16** |
| `chrono` / `uuid` | External | timestamps / IDs (already in workspace) |
| `sha2` / `ring` | External | checksums, backup encryption (ring already used by PrivacyManager) |

#### Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `world_store.path` | PathBuf | `~/.local/share/life-os/world/life-world.db` | SQLite database location |
| `world_store.documents_dir` | PathBuf | `~/.local/share/life-os/documents/` | Document blob root |
| `world_store.wal` | bool | true | WAL mode |
| `world_store.backup_count` | u32 | 3 | Rotating snapshots kept |
| `world_store.enable_fts` | bool | true | Build/maintain FTS5 indexes |

#### Thread Model

- The store is `Send + Sync`; SQLite accessed from Tokio via `spawn_blocking` for
  synchronous rusqlite calls (never block the async runtime).
- A single write lock serializes writers (SQLite single-writer); reads use WAL snapshot
  isolation. Keep lock scopes minimal.
- Event publishing is async/non-blocking.

#### Lifecycle

- `init`: open DB, run `PRAGMA journal_mode=WAL`, ensure schema, run pending one-time
  migration if `world_model.json` present and DB empty.
- `start`: load hot entities into `InMemoryWorldModelStore` for the cognitive loop; register
  event subscriptions.
- `stop`: flush in-memory hot set, checkpoint WAL, write export (`world_model.json`) as
  fallback, close DB cleanly.

#### Error Handling

- `VersionConflict` → caller re-reads and re-merges (AI) or is surfaced to UI (user).
- Storage/Io errors → fail-fast, logged, audit-recorded; companion falls back to its existing
  JSON save path during transition.
- Corruption at open → `integrity_check`, restore from latest verified backup, log recovery.

### Security Considerations

- Authorization on every API call (AI: capability-policy risk gate; user: session role).
- Input validation and length limits; `entity_type` catalogue enforcement.
- Document blob path canonicalization; no symlink/traversal writes.
- No secrets in the store; backup encryption via `PrivacyManager` (AES-GCM).
- Full audit trail of all writes; `audit_id` linkage to history.

### Performance Considerations

- Benchmarks (Criterion) for `get`, `update`, `search`, `insert_batch`, traversal.
- Targets in §Performance Expectations; FTS5 indexes kept in the same transaction as writes
  to avoid drift.
- History append is the only write amplification; no full-graph rewrites.

### Testing Strategy

- Unit: every `WorldStore` method happy + error paths (incl. `VersionConflict`, invalid
  type, not-found).
- Property-based: serialization round-trips of `Value`/`properties`; history reconstruction.
- Integration: full cognitive-loop flow against the SQLite store (existing
  `cognitive-integration`/`phase6-autonomous` tests re-targeted); restart durability test
  (write → reopen → verify); migration idempotency (run twice → identical); concurrent
  update conflict tests (`tokio::time::pause()`-free, deterministic).
- Backup/restore: round-trip with checksum verification; corrupt-file recovery test.

## Drawbacks

- A new external dependency (`rusqlite` bundled) requires approval (AGENTS.md §16) — small,
  widely used, audit-clean, C-bundled (no system dependency).
- Two write paths during transition (SQLite canonical + JSON export fallback) add a small
  operational surface until Open Space v1 is verified.
- Goals' dual representation (operational `goals` table + `goal` entity row) requires a
  discipline rule to keep them in sync (same UUID, single write path).

## Alternatives Considered

| Alternative | Reason for Rejection |
|---|---|
| Keep `world_model.json` as the world store | No CRUD/search/concurrency; 5-min data-loss window; cannot serve Open Space |
| New custom database (RocksDB/sled) | SQLite's FTS5 + JSON1 + ACID + single-file + portability are the better fit; existing traits already anticipate a SQL backend |
| No store; Open Space owns its own DB | Creates the second competing persistence system the audit explicitly warns against |
| Store entities as rows only (normalized relational design) | Reduces extensibility; the property graph is the canonical model per ADR-0006 |
| Vector search now | Embeddings are stubs today; FTS5 first, vector search as a later RFC |
| Rewrite `brain_core::model::WorldModel` as canonical | Out of scope; kept for compatibility (ADR-0008 Decision 9) |

## Open Questions

1. `rusqlite` bundled vs system `sqlite` package — resolved to bundled in M3 unless the
   audit fails.
2. Whether goals get their own entity row automatically for *every* goal or on-demand —
   default: every goal is also an entity (uniform graph).
3. FTS5 tokenizer for mixed-language content — build-time configuration; default `unicode61`.
4. Migration triggers — automatic at first boot vs explicit `life migrate` command; default
   automatic with `life migrate --dry-run` for verification.

## Implementation Plan

Milestone **M3** (next after this RFC):

1. Approve `rusqlite` dependency (AGENTS.md §16) — small.
2. Implement `crates/life-world-store` with `SqliteWorldModelStore` behind the extended
   `WorldModelStore` trait (+ `MemoryStore`/`StorageBackend` impls) — large.
3. Implement schema DDL from `life-world-store-schema.md` (entities, relationships,
   history, observations, documents, goals, lessons, FTS5) — medium.
4. Implement `WorldStore` API trait + optimistic concurrency + provenance + events —
   large.
5. Implement `world_model.json` migration (additive, idempotent, backup-first) — medium.
6. Implement document store (blob + metadata + FTS) — medium.
7. Implement goal/lesson durable persistence behind existing traits — medium.
8. Implement backup/restore/verification behind `PrivacyManager` — medium.
9. Re-target existing cognitive-loop/phase-6 tests at the new store; add durability,
   migration-idempotency, and conflict tests — large.
10. Criterion benchmarks for hot paths — small.

## Acceptance Criteria

A contractor must be able to implement the following **without making additional
architectural decisions**, using this RFC + `life-world-store-schema.md`:

1. SQLite World Store behind the existing storage abstractions.
2. Migration from `world_model.json` (additive, idempotent, no data loss, provenance
   tagged, original backup retained).
3. Document metadata storage (blob + metadata + extracted text + entity links).
4. Observation persistence (record, get, process-state, entity linkage).
5. CRUD (`create/get/update/archive/restore/delete_entity`,
   `create/get/update/delete_relationship`).
6. Search (`search_entities`, `search_documents` via FTS5 + filters).
7. History (`get_history`, append-only version snapshots).
8. Provenance (`WriteContext`, `created_by`, observation linkage).
9. Optimistic concurrency (`expected_version`, `VersionConflict`).
10. Backup/restore/verification (checksummed snapshots, recovery).

## Unresolved Topics

- Vector/embedding search (future RFC).
- Multi-device/server synchronization and merge policy (future RFC).
- At-rest encryption of the world store beyond backup encryption (follow-up).
- Open Space UI (separate RFC, milestone M4).
- Retiring `brain_core::model::WorldModel` (separate RFC).

## References

- [LIFE World Store audit](../architecture/life-world-store-audit.md)
- [LIFE World Store schema](../architecture/life-world-store-schema.md)
- [LIFE-OS system spec](../architecture/life-os-system-spec.md)
- [ADR-0008: Canonical World Store](../adr/0008-canonical-world-store.md)
- [ADR-0006: World Model](../adr/0006-world-model.md)
- [ADR-0007: Continuous Cognitive Loop](../adr/0007-continuous-cognitive-loop.md)
- [RFC-0002: Memory Platform](./RFC-0002-memory-platform.md)
