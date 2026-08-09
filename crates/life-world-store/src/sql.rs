//! SQLite DDL for the LIFE World Store (schema v0.1).
//!
//! Mirrors `docs/architecture/life-world-store-schema.md`. All timestamps are
//! unix seconds; JSON columns use the `memory_core::wm::Value` semantics.

/// Full schema, idempotent (`IF NOT EXISTS`).
pub const SCHEMA: &str = r#"
-- 1.1 entities ---------------------------------------------------------------
CREATE TABLE IF NOT EXISTS entities (
    id            TEXT PRIMARY KEY,
    entity_type   TEXT NOT NULL,
    name          TEXT NOT NULL,
    importance    REAL NOT NULL DEFAULT 0.5,
    confidence    REAL NOT NULL DEFAULT 0.5,
    lifecycle     TEXT NOT NULL DEFAULT 'Active',
    version       INTEGER NOT NULL DEFAULT 1,
    properties    TEXT NOT NULL DEFAULT '{}',
    metadata      TEXT NOT NULL DEFAULT '{}',
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    created_by    TEXT NOT NULL,
    provenance    TEXT,
    checksum      TEXT
);
CREATE INDEX IF NOT EXISTS idx_entities_type      ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_name      ON entities(name);
CREATE INDEX IF NOT EXISTS idx_entities_lifecycle ON entities(lifecycle);

-- 1.2 relationships -----------------------------------------------------------
CREATE TABLE IF NOT EXISTS relationships (
    id                TEXT PRIMARY KEY,
    relationship_type TEXT NOT NULL,
    source_id         TEXT NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    target_id         TEXT NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    confidence        REAL NOT NULL DEFAULT 0.5,
    weight            REAL NOT NULL DEFAULT 1.0,
    version           INTEGER NOT NULL DEFAULT 1,
    properties        TEXT NOT NULL DEFAULT '{}',
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    created_by        TEXT NOT NULL,
    provenance        TEXT
);
CREATE INDEX IF NOT EXISTS idx_relationships_source ON relationships(source_id);
CREATE INDEX IF NOT EXISTS idx_relationships_target ON relationships(target_id);
CREATE INDEX IF NOT EXISTS idx_relationships_type   ON relationships(relationship_type);

-- 1.3 history -----------------------------------------------------------------
-- History rows intentionally survive row deletion (no ON DELETE CASCADE, and no
-- FK constraint): a deleted relationship keeps its audit trail, and a hard
-- entity delete removes history explicitly then records a final Delete
-- tombstone. The schema doc (life-world-store-schema.md §1.3) lists these
-- tables without foreign keys; an FK here would block the parent deletion.
CREATE TABLE IF NOT EXISTS entity_history (
    entity_id     TEXT NOT NULL,
    version       INTEGER NOT NULL,
    operation     TEXT NOT NULL,
    snapshot_json TEXT NOT NULL,
    changed_by    TEXT NOT NULL,
    provenance    TEXT,
    changed_at    INTEGER NOT NULL,
    audit_id      TEXT,
    PRIMARY KEY (entity_id, version)
);

CREATE TABLE IF NOT EXISTS relationship_history (
    relationship_id TEXT NOT NULL,
    version         INTEGER NOT NULL,
    operation       TEXT NOT NULL,
    snapshot_json   TEXT NOT NULL,
    changed_by      TEXT NOT NULL,
    provenance      TEXT,
    changed_at      INTEGER NOT NULL,
    audit_id        TEXT,
    PRIMARY KEY (relationship_id, version)
);

-- 2.1 observations -------------------------------------------------------------
CREATE TABLE IF NOT EXISTS observations (
    id                TEXT PRIMARY KEY,
    kind              TEXT NOT NULL,
    source            TEXT,
    payload_json      TEXT NOT NULL,
    summary           TEXT,
    observed_at       INTEGER NOT NULL,
    confidence        REAL DEFAULT 0.5,
    processed         INTEGER NOT NULL DEFAULT 0,
    state_changes_json TEXT
);
CREATE INDEX IF NOT EXISTS idx_observations_observed_at ON observations(observed_at);
CREATE INDEX IF NOT EXISTS idx_observations_processed   ON observations(processed);

-- 2.2 entity <-> observation links ---------------------------------------------
CREATE TABLE IF NOT EXISTS entity_observations (
    entity_id      TEXT NOT NULL REFERENCES entities(id)      ON DELETE CASCADE,
    observation_id TEXT NOT NULL REFERENCES observations(id)  ON DELETE CASCADE,
    PRIMARY KEY (entity_id, observation_id)
);

-- 3.1 documents ----------------------------------------------------------------
CREATE TABLE IF NOT EXISTS documents (
    id                TEXT PRIMARY KEY,
    title             TEXT NOT NULL,
    mime_type         TEXT,
    size_bytes        INTEGER,
    storage_path      TEXT,
    checksum          TEXT,
    original_filename TEXT,
    source_uri        TEXT,
    lifecycle         TEXT NOT NULL DEFAULT 'Active',
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    version           INTEGER NOT NULL DEFAULT 1,
    created_by        TEXT NOT NULL,
    provenance        TEXT,
    tags_json         TEXT NOT NULL DEFAULT '[]'
);

-- 3.2 extracted text ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS document_content (
    document_id   TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    extracted_text TEXT NOT NULL,
    content_type  TEXT,
    content_hash  TEXT
);

-- 3.3 document <-> entity links ---------------------------------------------------
CREATE TABLE IF NOT EXISTS document_entities (
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    entity_id   TEXT NOT NULL REFERENCES entities(id)  ON DELETE RESTRICT,
    role        TEXT,
    confidence  REAL DEFAULT 0.5,
    PRIMARY KEY (document_id, entity_id, role)
);

-- 5.1 goals ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS goals (
    goal_id           TEXT PRIMARY KEY,
    goal_type         TEXT NOT NULL,
    description       TEXT NOT NULL,
    priority          TEXT NOT NULL,
    status            TEXT NOT NULL,
    parent            TEXT,
    children_json     TEXT NOT NULL DEFAULT '[]',
    dependencies_json TEXT NOT NULL DEFAULT '[]',
    version           INTEGER NOT NULL DEFAULT 1,
    retry_count       INTEGER NOT NULL DEFAULT 0,
    progress_pct      REAL NOT NULL DEFAULT 0,
    source            TEXT,
    outcome           TEXT,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    metadata_json     TEXT NOT NULL DEFAULT '{}'
);

-- 5.2 lessons --------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS lessons (
    lesson_id   TEXT PRIMARY KEY,
    category    TEXT NOT NULL,
    content     TEXT NOT NULL,
    source      TEXT,
    confidence  REAL DEFAULT 0.5,
    created_at  INTEGER NOT NULL
);

-- 4.1 entities FTS5 ---------------------------------------------------------------
CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(
    entity_id, name, entity_type, properties_text, metadata_text
);

-- 4.2 documents FTS5 ---------------------------------------------------------------
CREATE VIRTUAL TABLE IF NOT EXISTS document_fts USING fts5(
    document_id, title, extracted_text
);

-- internal metadata (e.g. migration state) ----------------------------------------
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// Key used to record that the `world_model.json` migration has completed.
pub const META_MIGRATION_DONE: &str = "world_model_json_migrated";
