# LIFE World Store — Proposed Schema (v0.1)

> **Status:** Draft — schema ONLY. Not implemented. No migrations will be run.
> **Design rationale:** see `life-world-store-audit.md`.
> The current in-memory World Model types (`memory_core::wm::Entity`,
> `memory_core::wm::Relationship`, `Value`) remain the canonical in-memory model; these
> tables are the durable representation behind the extended `WorldModelStore` trait.

Physical layout: a single SQLite database (`life-world.db`, WAL mode) under
`~/.local/share/life-os/world/`. Document blobs live in `~/.local/share/life-os/documents/`
(checksummed files); the database stores metadata + extracted text.

## 1. Core tables

### 1.1 `entities`

The canonical entity graph (superset of `memory_core::wm::Entity`).

| column | type | notes |
|---|---|---|
| `id` | TEXT PRIMARY KEY | UUID v4 |
| `entity_type` | TEXT NOT NULL | typed catalogue (see §6); normalized at ingestion |
| `name` | TEXT NOT NULL | display name; indexed |
| `importance` | REAL NOT NULL DEFAULT 0.5 | 0..1 |
| `confidence` | REAL NOT NULL DEFAULT 0.5 | 0..1 |
| `lifecycle` | TEXT NOT NULL DEFAULT 'Active' | Active \| Archived \| Forgotten |
| `version` | INTEGER NOT NULL DEFAULT 1 | optimistic concurrency token |
| `properties` | TEXT NOT NULL DEFAULT '{}' | JSON object (matches `Value` enum semantics) |
| `metadata` | TEXT NOT NULL DEFAULT '{}' | JSON object; includes `original_type` when normalized |
| `created_at` | INTEGER NOT NULL | unix seconds |
| `updated_at` | INTEGER NOT NULL | unix seconds |
| `created_by` | TEXT NOT NULL | writer tag: `ai` \| `user` \| `observation` \| `runtime` |
| `provenance` | TEXT | observation id / user action id / source uri (nullable) |
| `checksum` | TEXT | content hash (optional integrity) |

Indexes: `idx_entities_type(entity_type)`, `idx_entities_name(name)`,
`idx_entities_lifecycle(lifecycle)`.

### 1.2 `relationships`

Directed property-graph edges (superset of `memory_core::wm::Relationship`).

| column | type | notes |
|---|---|---|
| `id` | TEXT PRIMARY KEY | UUID v4 |
| `relationship_type` | TEXT NOT NULL | typed catalogue (e.g. `works_at`, `about`, `has`) |
| `source_id` | TEXT NOT NULL REFERENCES entities(id) | ON DELETE RESTRICT |
| `target_id` | TEXT NOT NULL REFERENCES entities(id) | ON DELETE RESTRICT |
| `confidence` | REAL NOT NULL DEFAULT 0.5 | 0..1 |
| `weight` | REAL NOT NULL DEFAULT 1.0 | |
| `version` | INTEGER NOT NULL DEFAULT 1 | optimistic concurrency token |
| `properties` | TEXT NOT NULL DEFAULT '{}' | JSON object |
| `created_at` | INTEGER NOT NULL | unix seconds |
| `updated_at` | INTEGER NOT NULL | unix seconds |
| `created_by` | TEXT NOT NULL | writer tag |
| `provenance` | TEXT | nullable |

Indexes: `idx_relationships_source(source_id)`, `idx_relationships_target(target_id)`,
`idx_relationships_type(relationship_type)`.

### 1.3 `entity_history` / `relationship_history`

Append-only version history.

| column | type | notes |
|---|---|---|
| `entity_id` | TEXT NOT NULL REFERENCES entities(id) | |
| `version` | INTEGER NOT NULL | the version being recorded |
| `operation` | TEXT NOT NULL | create \| update \| archive \| restore \| delete |
| `snapshot_json` | TEXT NOT NULL | full row snapshot at that version |
| `changed_by` | TEXT NOT NULL | writer tag |
| `provenance` | TEXT | nullable |
| `changed_at` | INTEGER NOT NULL | unix seconds |
| `audit_id` | TEXT | link to audit trail (nullable) |

Primary key: `(entity_id, version)`. Same shape for `relationship_history`.

## 2. Observations

### 2.1 `observations`

Retained observations that drive understanding (currently discarded — now durable).

| column | type | notes |
|---|---|---|
| `id` | TEXT PRIMARY KEY | UUID v4 |
| `kind` | TEXT NOT NULL | source kind: time \| system \| desktop \| email \| telegram \| whatsapp \| runtime \| user \| clock |
| `source` | TEXT | provider/source id (e.g. gmail message id) |
| `payload_json` | TEXT NOT NULL | original observation payload |
| `summary` | TEXT | natural-language summary (from understanding) |
| `observed_at` | INTEGER NOT NULL | unix seconds |
| `confidence` | REAL DEFAULT 0.5 | |
| `processed` | INTEGER NOT NULL DEFAULT 0 | 0=raw 1=understood 2=evolved |
| `state_changes_json` | TEXT | from `StructuredWorldUpdate.state_changes` |

Index: `idx_observations_observed_at(observed_at)`, `idx_observations_processed(processed)`.

### 2.2 `entity_observations`

Junction: which observations produced/updated which entities (traceability).

| column | type |
|---|---|
| `entity_id` | TEXT NOT NULL REFERENCES entities(id) |
| `observation_id` | TEXT NOT NULL REFERENCES observations(id) |

Primary key: `(entity_id, observation_id)`.

## 3. Documents

### 3.1 `documents`

First-class entities; also stored as `entities` rows with `entity_type='document'`
linked by `document_id`.

| column | type | notes |
|---|---|---|
| `id` | TEXT PRIMARY KEY | UUID v4 (same as the corresponding entities.id) |
| `title` | TEXT NOT NULL | |
| `mime_type` | TEXT | |
| `size_bytes` | INTEGER | |
| `storage_path` | TEXT | relative path under documents blob dir |
| `checksum` | TEXT | sha256 of original file |
| `original_filename` | TEXT | |
| `source_uri` | TEXT | origin (email id, url, import path) |
| `created_at` | INTEGER NOT NULL | unix seconds |
| `updated_at` | INTEGER NOT NULL | unix seconds |
| `version` | INTEGER NOT NULL DEFAULT 1 | |
| `created_by` | TEXT NOT NULL | writer tag |
| `provenance` | TEXT | nullable |
| `tags_json` | TEXT NOT NULL DEFAULT '[]' | |

### 3.2 `document_content`

Extracted text + type.

| column | type |
|---|---|
| `document_id` | TEXT PRIMARY KEY REFERENCES documents(id) |
| `extracted_text` | TEXT NOT NULL |
| `content_type` | TEXT | e.g. `plain`, `markdown`, `pdf-text` |
| `content_hash` | TEXT | hash of extracted text |

Full-text index: FTS5 virtual table `document_fts(document_id, title, extracted_text)`.

### 3.3 `document_entities`

Junction: entities extracted from / related to a document.

| column | type | notes |
|---|---|---|
| `document_id` | TEXT NOT NULL REFERENCES documents(id) | |
| `entity_id` | TEXT NOT NULL REFERENCES entities(id) | |
| `role` | TEXT | e.g. `about`, `author`, `mentioned` |
| `confidence` | REAL DEFAULT 0.5 | |

Primary key: `(document_id, entity_id, role)`.

## 4. Search

### 4.1 `entities_fts` (FTS5)

`entities_fts(entity_id, name, entity_type, properties_text, metadata_text)` — populated
from the JSON text of `properties`/`metadata`.

### 4.2 `document_fts` (FTS5)

`document_fts(document_id, title, extracted_text)`.

Query pattern (both): `SELECT ... FROM entities_fts WHERE entities_fts MATCH ?` with
entity-type + lifecycle filters applied afterwards.

## 5. Goals / lessons (persisted)

### 5.1 `goals`

Durable backing for `brain-goals` `GoalStore` (currently in-memory). Mirrors
`GoalRecord` fields.

| column | type |
|---|---|
| `goal_id` | TEXT PRIMARY KEY |
| `goal_type` | TEXT NOT NULL |
| `description` | TEXT NOT NULL |
| `priority` | TEXT NOT NULL |
| `status` | TEXT NOT NULL |
| `parent` | TEXT NULL |
| `children_json` | TEXT NOT NULL DEFAULT '[]' |
| `dependencies_json` | TEXT NOT NULL DEFAULT '[]' |
| `version` | INTEGER NOT NULL DEFAULT 1 |
| `retry_count` | INTEGER NOT NULL DEFAULT 0 |
| `progress_pct` | REAL NOT NULL DEFAULT 0 |
| `source` | TEXT |
| `outcome` | TEXT NULL |
| `created_at` / `updated_at` | INTEGER NOT NULL |
| `metadata_json` | TEXT NOT NULL DEFAULT '{}' |

### 5.2 `lessons`

Durable backing for `brain-reflection` `LessonStore`.

| column | type |
|---|---|
| `lesson_id` | TEXT PRIMARY KEY |
| `category` | TEXT NOT NULL |
| `content` | TEXT NOT NULL |
| `source` | TEXT |
| `confidence` | REAL DEFAULT 0.5 |
| `created_at` | INTEGER NOT NULL |

## 6. Typed entity catalogue (v0.1)

Canonical `entity_type` values for new writes; original free-form values preserved in
`metadata.original_type`.

```
user, person, contact, company, organization, project, goal, mission,
job, lead, event, document, knowledge, concept, observation, email,
message, meeting, task, skill, resource, location, product, topic
```

Canonical relationship types (v0.1, open catalogue):

```
works_at, employed_by, has_contact, is_a, member_of, manages, reports_to,
candidate_for, leads, belongs_to, about, references, contributes_to,
created_by, involves, scheduled_on, relates_to, works_on, produced
```

## 7. Notes

- All timestamps are unix seconds (UTC). Existing `Timestamp` type maps to this.
- JSON columns use the existing `Value` enum semantics; parse/validate at the store
  boundary.
- Soft delete = `lifecycle='Archived'` (+ history row); hard delete only via explicit
  operation after archive.
- Migration from `world_model.json` is a one-time, additive import (documented in the
  audit doc §13). No destructive operations.
