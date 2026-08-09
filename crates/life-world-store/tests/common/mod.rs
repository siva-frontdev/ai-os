//! Shared helpers for the LIFE World Store integration tests.
//!
//! Every test uses an isolated `tempfile` directory, so no test depends on the
//! developer's home directory, existing runtime config, `.env`, the network, or
//! any external service.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use life_world_store::{
    Actor, CreateDocument, CreateEntity, CreateRelationship, Document, DocumentEntityLink,
    Observation, ObservationId, ObservationProcessState, SqliteWorldModelStore, StoredGoal,
    StoredLesson, WorldStore, WorldStoreConfig, WriteContext,
};
use memory_core::Timestamp;
use memory_core::wm::{Entity, EntityId, Relationship, RelationshipId, Value};

/// A per-test sandbox: temp dir + store already `init`'d.
pub struct Sandbox {
    pub _dir: tempfile::TempDir,
    pub store: SqliteWorldModelStore,
    pub db_path: PathBuf,
    pub documents_dir: PathBuf,
}

impl Sandbox {
    /// Create a sandbox with a store initialised at the given base dir.
    pub async fn new(dir: tempfile::TempDir) -> Self {
        let cfg = config_for(dir.path());
        let db_path = cfg.db_path.clone();
        let documents_dir = cfg.documents_dir.clone();
        let store = SqliteWorldModelStore::new(cfg);
        store.init().await.expect("init store");
        Self {
            _dir: dir,
            store,
            db_path,
            documents_dir,
        }
    }

    /// Reopen a brand-new store over the same database (restart durability).
    pub async fn reopen(&self) -> SqliteWorldModelStore {
        let cfg = WorldStoreConfig {
            db_path: self.db_path.clone(),
            documents_dir: self.documents_dir.clone(),
            wal: true,
            backup_count: 3,
            enable_fts: true,
            json_migration_source: None,
        };
        let store = SqliteWorldModelStore::new(cfg);
        store.init().await.expect("reopen init");
        store
    }

    /// A direct connection for inspecting rows the public API does not expose.
    pub fn raw(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.db_path).expect("open raw connection")
    }
}

/// Build a store config isolated to `base`.
pub fn config_for(base: &Path) -> WorldStoreConfig {
    WorldStoreConfig {
        db_path: base.join("world").join("life-world.db"),
        documents_dir: base.join("documents"),
        wal: true,
        backup_count: 3,
        enable_fts: true,
        json_migration_source: None,
    }
}

// ── Write contexts ───────────────────────────────────────────────────────────

/// A write context with the given actor and no provenance.
pub fn ctx(actor: Actor) -> WriteContext {
    WriteContext {
        actor,
        provenance: None,
    }
}

/// AI write context.
pub fn ai_ctx() -> WriteContext {
    ctx(Actor::Ai)
}

/// A `user` write context.
pub fn user_ctx() -> WriteContext {
    ctx(Actor::User {
        user_id: "test-user".into(),
    })
}

/// An `observation` write context carrying a provenance id.
pub fn observation_ctx(provenance: &str) -> WriteContext {
    WriteContext {
        actor: Actor::Observation,
        provenance: Some(provenance.to_string()),
    }
}

/// A `runtime` write context.
pub fn runtime_ctx() -> WriteContext {
    ctx(Actor::Runtime)
}

/// A `migration` write context.
pub fn migration_ctx() -> WriteContext {
    ctx(Actor::Migration)
}

// ── Builders ─────────────────────────────────────────────────────────────────

/// A minimal `CreateEntity` request.
pub fn create_entity(entity_type: &str, name: &str) -> CreateEntity {
    CreateEntity {
        entity_type: entity_type.to_string(),
        name: name.to_string(),
        properties: HashMap::new(),
        importance: 0.5,
        confidence: 0.5,
        metadata: HashMap::new(),
        ctx: ai_ctx(),
    }
}

/// Create an entity through the store and return it.
pub async fn add_entity(store: &SqliteWorldModelStore, entity_type: &str, name: &str) -> Entity {
    store
        .create_entity(create_entity(entity_type, name))
        .await
        .expect("create entity")
}

/// Create a relationship between two entities through the store.
pub async fn add_relationship(
    store: &SqliteWorldModelStore,
    rel_type: &str,
    source: &EntityId,
    target: &EntityId,
) -> Relationship {
    store
        .create_relationship(CreateRelationship {
            relationship_type: rel_type.to_string(),
            source_id: *source,
            target_id: *target,
            properties: HashMap::new(),
            confidence: 0.7,
            weight: 1.0,
            ctx: ai_ctx(),
        })
        .await
        .expect("create relationship")
}

/// A fully-populated observation for storage round-trips.
pub fn observation(kind: &str) -> Observation {
    Observation {
        id: ObservationId::new(),
        kind: kind.to_string(),
        source: Some("test-provider".into()),
        payload: serde_json::json!({
            "message": "hello world",
            "nested": {"n": 42, "ok": true},
            "list": [1, 2, 3],
        }),
        summary: Some("a natural language summary".into()),
        observed_at: Timestamp::from_secs(1_700_000_000),
        confidence: 0.8,
        processed: ObservationProcessState::Raw,
        state_changes: Some(serde_json::json!([{"op": "upsert", "path": "/entities"}] )),
    }
}

/// A simple document create request wrapping `content`.
pub fn create_doc(
    title: &str,
    content: &[u8],
    entity_links: Vec<DocumentEntityLink>,
) -> CreateDocument {
    CreateDocument {
        title: title.to_string(),
        mime_type: Some("text/plain".into()),
        original_filename: Some(format!("{title}.txt")),
        source_uri: Some("test://doc".into()),
        tags: vec!["a".into(), "b".into()],
        content: content.to_vec(),
        extracted_text: Some("extracted text here".into()),
        content_type: Some("plain".into()),
        entity_properties: HashMap::new(),
        entity_links,
        ctx: ai_ctx(),
    }
}

/// Create a document through the store.
pub async fn add_doc(store: &SqliteWorldModelStore, title: &str, content: &[u8]) -> Document {
    store
        .create_document(create_doc(title, content, vec![]))
        .await
        .expect("create document")
}

/// A minimal `StoredGoal` with a fresh UUID id.
pub fn goal() -> StoredGoal {
    StoredGoal {
        goal_id: uuid::Uuid::new_v4().to_string(),
        goal_type: "task".into(),
        description: "Ship the release".into(),
        priority: "normal".into(),
        status: "in_progress".into(),
        parent: None,
        children: Vec::new(),
        dependencies: Vec::new(),
        version: 0,
        retry_count: 0,
        progress_pct: 0.0,
        source: Some("test".into()),
        outcome: None,
        created_at: Timestamp::from_secs(1_700_000_000),
        updated_at: Timestamp::from_secs(1_700_000_000),
        metadata: HashMap::new(),
    }
}

/// A minimal `StoredLesson` with a fresh UUID id.
pub fn lesson() -> StoredLesson {
    StoredLesson {
        lesson_id: uuid::Uuid::new_v4().to_string(),
        category: "general".into(),
        content: "A lesson learned".into(),
        source: Some("test".into()),
        confidence: 0.9,
        created_at: Timestamp::from_secs(1_700_000_000),
    }
}

// ── Raw-row inspection helpers ───────────────────────────────────────────────

/// The `created_by` / `provenance` columns of an entities row.
pub fn entity_provenance(conn: &rusqlite::Connection, id: &EntityId) -> (String, Option<String>) {
    conn.query_row(
        "SELECT created_by, provenance FROM entities WHERE id = ?1",
        [id.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("read entity provenance")
}

/// The `created_by` / `provenance` columns of a relationships row.
pub fn relationship_provenance(
    conn: &rusqlite::Connection,
    id: &RelationshipId,
) -> (String, Option<String>) {
    conn.query_row(
        "SELECT created_by, provenance FROM relationships WHERE id = ?1",
        [id.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("read relationship provenance")
}

/// The `created_by` / `provenance` columns of a documents row.
pub fn document_provenance(
    conn: &rusqlite::Connection,
    id: &life_world_store::DocumentId,
) -> (String, Option<String>) {
    conn.query_row(
        "SELECT created_by, provenance FROM documents WHERE id = ?1",
        [id.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("read document provenance")
}

/// All `(version, operation, changed_by, provenance)` history rows for an entity.
pub fn entity_history_rows(
    conn: &rusqlite::Connection,
    id: &EntityId,
) -> Vec<(i64, String, String, Option<String>)> {
    let mut stmt = conn
        .prepare(
            "SELECT version, operation, changed_by, provenance FROM entity_history \
             WHERE entity_id = ?1 ORDER BY version ASC",
        )
        .expect("prepare history query");
    stmt.query_map([id.to_string()], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })
    .expect("query history")
    .map(|r| r.expect("row"))
    .collect()
}

/// All `(version, operation, changed_by)` history rows for a relationship.
pub fn relationship_history_rows(
    conn: &rusqlite::Connection,
    id: &RelationshipId,
) -> Vec<(i64, String, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT version, operation, changed_by FROM relationship_history \
             WHERE relationship_id = ?1 ORDER BY version ASC",
        )
        .expect("prepare rel history query");
    stmt.query_map([id.to_string()], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })
    .expect("query rel history")
    .map(|r| r.expect("row"))
    .collect()
}

/// Assert two maps of `Value` are equal (helper keeps imports tidy).
pub fn assert_value_map_eq(left: &HashMap<String, Value>, right: &HashMap<String, Value>) {
    assert_eq!(
        left.len(),
        right.len(),
        "map lengths differ: {left:?} vs {right:?}"
    );
    for (k, v) in left {
        match (v, right.get(k)) {
            (Value::String(a), Some(Value::String(b))) => assert_eq!(a, b, "key {k}"),
            (Value::Number(a), Some(Value::Number(b))) => assert_eq!(a, b, "key {k}"),
            (Value::Boolean(a), Some(Value::Boolean(b))) => assert_eq!(a, b, "key {k}"),
            (Value::Null, Some(Value::Null)) => {}
            (Value::Timestamp(a), Some(Value::Timestamp(b))) => assert_eq!(a, b, "key {k}"),
            (Value::EntityId(a), Some(Value::EntityId(b))) => assert_eq!(a, b, "key {k}"),
            (a, b) => panic!("value mismatch for key {k}: {a:?} vs {b:?}"),
        }
    }
}
