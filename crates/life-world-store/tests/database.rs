//! Database initialisation: schema creation, reopen preservation, WAL, FK.

mod common;

use common::{Sandbox, add_entity};

use life_world_store::WorldStore;

/// `init` opens a fresh database and creates the schema tables.
#[tokio::test]
async fn init_creates_schema() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let conn = sb.raw();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for required in [
        "entities",
        "relationships",
        "entity_history",
        "relationship_history",
        "observations",
        "entity_observations",
        "documents",
        "document_content",
        "document_entities",
        "goals",
        "lessons",
        "meta",
    ] {
        assert!(
            tables.contains(&required.to_string()),
            "missing table {required}"
        );
    }
}

/// Reopening the same database path with a brand-new store preserves data.
#[tokio::test]
async fn reopen_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let sb = Sandbox::new(dir).await;
    let e = add_entity(&sb.store, "person", "Alice").await;

    let store = sb.reopen().await;
    let got = store.get_entity(&e.id).await.unwrap();
    assert_eq!(got.map(|x| x.name), Some("Alice".to_string()));
}

/// WAL journal mode is enabled when configured.
#[tokio::test]
async fn wal_mode_is_enabled() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let conn = sb.raw();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal");
}

/// The `foreign_keys` pragma is on, so referential integrity is enforced.
#[tokio::test]
async fn foreign_keys_are_enforced() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let conn = sb.raw();
    let fk: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1, "foreign_keys pragma should be ON");

    // A relationship to a missing entity must be rejected by the engine.
    let target = add_entity(&sb.store, "person", "Bob").await;
    let ghost = memory_core::wm::EntityId::new();
    let res = sb
        .store
        .create_relationship(life_world_store::CreateRelationship {
            relationship_type: "knows".into(),
            source_id: ghost,
            target_id: target.id,
            properties: std::collections::HashMap::new(),
            confidence: 0.5,
            weight: 0.5,
            ctx: common::ai_ctx(),
        })
        .await;
    assert!(
        res.is_err(),
        "FK should reject a relationship to a missing entity"
    );
}

/// `init` twice on the same store is harmless.
#[tokio::test]
async fn init_is_idempotent() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    sb.store.init().await.unwrap();
    sb.store.init().await.unwrap();
}
