# ADR-0008: Canonical World Store

## Status

Accepted

## Date

2026-08-08

## Context

LIFE Open Space must let the user and the AI operate on **the same underlying world data** —
personal information, companies, contacts, documents, projects, goals, missions, jobs,
leads, knowledge, relationships, events, preferences — with full CRUD, search, filter,
relate, view, and visualize.

The audit (`docs/architecture/life-world-store-audit.md`) established the current state:

- The canonical in-memory World Model is the property graph
  `memory_core::wm::{Entity, Relationship, Value}` with the `WorldModelStore` trait,
  implemented by `InMemoryWorldModelStore`.
- The only durable world data is `world_model.json`, written by
  `PersistenceManager` (full-graph pretty JSON, atomic replace, auto-save every 5 min and
  on shutdown).
- Storage abstractions (`StorageBackend`, `Transaction`, `BatchOperation`, `memory-index`
  traits) are intentionally incomplete no-op stubs; their doc comments anticipate a SQL
  backend (SQLite/PostgreSQL/RocksDB).
- There is no SQLite, no document storage, no observation persistence, and no provenance/
  history/versioning/concurrency.
- Goals, lessons, and learning state are in-memory and **lost on restart**.
- A second, competing fixed-schema model exists: `brain_core::model::WorldModel`
  (`WorldModelService`), used by the `BrainOrchestrator` path — in-memory, never persisted.
- The cognitive/runtime architecture (EventBus, Service lifecycle, scheduler, permissions,
  CognitiveLoopService, CompanionHost, providers, MCP) must remain intact.

This ADR records the architectural decisions that make the World Model the durable,
canonical **World Store** that both the AI and the user operate on — without rewriting the
cognitive architecture and without creating a second competing persistence system.

## Decision

### Decision 1: `memory_core::wm` is the canonical World Model

We will keep `memory_core::wm::{Entity, Relationship, Value}` as the **single canonical
World Model** — a property graph with typed nodes and edges, open to new concepts without
domain-specific structs. No new canonical model will be introduced. A typed `entity_type`
catalogue will govern new writes, with original free-form strings preserved in
`metadata.original_type`.

### Decision 2: SQLite is the first durable backend

We will implement the durable World Store on **SQLite** (WAL mode, single file at
`~/.local/share/life-os/world/life-world.db`), behind the existing storage abstractions.
Rationale: ACID transactions, optimistic concurrency support, built-in FTS5 full-text
search, JSON columns for `properties`/`metadata`, single-file portability, and zero-daemon
embedded operation. Other backends (PostgreSQL, RocksDB) remain possible behind the same
traits.

### Decision 3: World Store is accessed through existing storage abstractions

We will extend the existing `WorldModelStore` (and `MemoryStore`/`StorageBackend`) traits
rather than inventing a parallel storage interface. The cognitive loop, evolution engine,
companion host, and (new) Open Space all read/write through these traits. The
`InMemoryWorldModelStore` remains the hot working set; SQLite is the durable canonical
substrate it loads from and flushes to.

### Decision 4: Documents use filesystem/blob storage with metadata in SQLite

We will store document **original files as checksummed blobs** under
`~/.local/share/life-os/documents/`, with **metadata, extracted text, and entity links in
SQLite** (`documents`, `document_content`, `document_entities`). Every document is also an
entity (`entity_type='document'`, same UUID) so it participates in the graph, search, and
relationships. Deletion follows the global soft-delete lifecycle; the blob is retained until
hard delete.

### Decision 5: Observations become durable

We will persist observations (`record_observation`/`get_observation`) and link each
observation to the entities it produced (`entity_observations`). The understanding pipeline
tracks process state (raw → understood → evolved). This provides end-to-end provenance:
every AI-initiated change is traceable to the observation that justified it. (Today,
`StructuredWorldUpdate` content is discarded after evolution.)

### Decision 6: World entities support provenance and version history

Every entity/relationship/document row carries `created_by` (actor tag) and `provenance`
(observation id / user action / source URI). Append-only `entity_history` /
`relationship_history` tables record a snapshot and operation per `version`. `get_history`
returns the ordered change log with who/what/when/why. `Entity.version` becomes a real
concurrency token and history index.

### Decision 7: Soft delete is the normal deletion mechanism

We will implement deletion as **soft delete** by default: `archive_entity` /
`archive_document` set `lifecycle='Archived'` and retain history and relationships;
`restore_entity` returns to `Active`. Hard delete is an explicit, privileged operation that
also removes history — never the default path. This preserves auditability and provenance.

### Decision 8: AI and user operate through the same World Store APIs

We will expose a single `WorldStore` API surface used by **both** the AI (cognitive loop)
and the user (Open Space + existing companion memory controls). The only difference is the
`WriteContext` (actor + provenance) and authorization path (AI: `capability-policy` risk
gate; user: session role). This guarantees that "AI maintains the world" and "user edits the
same world" cannot drift apart.

### Decision 9: `brain_core::model::WorldModel` remains temporarily for compatibility but is not the canonical long-term model

We will **not** retire or rework `brain_core::model::WorldModel` (the fixed-schema model
behind `WorldModelService`/`BrainOrchestrator`) during the World Store work. It remains for
compatibility with the existing `BrainOrchestrator` path. No new development extends it;
new world data flows through `memory_core::wm` and the World Store. Its retirement is a
separate future RFC.

### Decision 10: No Open Space UI accesses SQLite directly

We will build the Open Space UI to talk exclusively to the **World Store API** (via the
service/lifecycle layer). The UI never opens the database, never issues SQL, and never
writes blobs directly. This keeps the UI a pure view/manipulation layer and preserves a
single enforcement point for authorization, provenance, concurrency, and audit. (Applies
equally to the existing companion web UI once Open Space lands.)

## Consequences

### Positive

- Single durable source of truth for the user's world; AI and user edits converge on one
  store with no drift.
- Provenance, history, and versioning make every change auditable and explainable
  (`get_history`), aligned with the "observe everything" tenet.
- Optimistic concurrency prevents lost updates between the autonomous AI loop and user edits.
- Soft delete preserves auditability; nothing is silently destroyed.
- FTS5 gives real search without a second system.
- Observations retained → end-to-end traceability (observation → understanding → entity).
- Existing cognitive/runtime architecture is untouched; risk is contained to the storage
  layer.
- The property-graph entity model supports future server + multi-device sync without
  redesign (immutable UUIDs, versions, tombstones).
- No second competing persistence system (the audit's core warning is avoided).

### Negative

- New external dependency (`rusqlite` bundled) enters the workspace and must pass the
  dependency-approval process (AGENTS.md §16).
- Transition period has two write paths (SQLite canonical + JSON export fallback), a small
  operational surface until Open Space v1 is verified.
- Goals require a discipline rule to keep the operational `goals` row and the `goal` entity
  row in sync (same UUID, single write path).
- SQLite is a personal-scale store; if LIFE grows to server/CLI-scale, the same traits must
  host a different backend (accepted trade-off, interfaces isolate it).
- `brain_core::model::WorldModel` remains in the codebase during transition, which can
  mislead contributors until its retirement RFC lands.

## Compliance

- All world data writes must go through the `WorldStore`/`WorldModelStore` trait APIs.
  Code review rejects direct SQL/rusqlite usage outside `crates/life-world-store`.
- `crates/life-world-store` is the **only** crate allowed to depend on `rusqlite`; enforced
  by a dependency-graph check in CI (deny `rusqlite` in any other `Cargo.toml`).
- No Open Space/UI crate may import `rusqlite` or open a database connection; enforced by
  the same dependency check plus review.
- Every new `entity_type` write must come from the typed catalogue; free-form types are
  only ever stored in `metadata.original_type`. Review/CI lint on `entity_type` sources.
- No code may call `delete_entity(hard=true)` outside an explicit, privileged, reviewed
  path (soft delete is the default).
- `brain_core::model::WorldModel` must not gain new persisted-storage code; new world
  storage is the World Store. Review enforces this.
- Migration from `world_model.json` must always first copy the source file to a timestamped
  backup and tag migrated rows `created_by='migration'`.

## Notes

- This ADR is the companion decision record to RFC-0008 (LIFE World Store). It records the
  decisions at architecture-finalization time; implementation is milestone M3.
- Supersedes nothing; builds on ADR-0006 (World Model) and ADR-0007 (Continuous Cognitive
  Loop).
- Follow-ups: vector search RFC; synchronization RFC; retirement RFC for
  `brain_core::model::WorldModel`; at-rest encryption follow-up.

## References

- [RFC-0008: LIFE World Store](../rfc/RFC-0008-life-world-store.md)
- [LIFE World Store schema](../architecture/life-world-store-schema.md)
- [LIFE World Store audit](../architecture/life-world-store-audit.md)
- [ADR-0006: World Model](./0006-world-model.md)
- [ADR-0007: Continuous Cognitive Loop](./0007-continuous-cognitive-loop.md)
- [RFC-0002: Memory Platform](../rfc/RFC-0002-memory-platform.md)
- [memory_core::wm](../../crates/memory-core/src/wm.rs)
- [InMemoryWorldModelStore](../../crates/memory-storage/src/wm_store.rs)
- [PersistenceManager](../../brain/brain-coordinator/src/companion_host/persistence.rs)
