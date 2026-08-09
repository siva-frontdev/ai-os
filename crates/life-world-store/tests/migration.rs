//! `world_model.json` migration: import, idempotency, and failure modes.

mod common;

use std::path::PathBuf;

use common::Sandbox;

use life_world_store::{JsonWorldModel, WorldStore};

use memory_core::wm::{Entity, Relationship};

fn sample_model() -> JsonWorldModel {
    let alice = Entity::new("person", "Alice");
    let bob = Entity::new("person", "Bob");
    let rel = Relationship::new("knows", alice.id, bob.id);
    JsonWorldModel {
        entities: vec![alice, bob],
        relationships: vec![rel],
        saved_at: 1_700_000_000,
    }
}

fn write_model(dir: &std::path::Path, model: &JsonWorldModel) -> PathBuf {
    let path = dir.join("world_model.json");
    std::fs::write(&path, serde_json::to_string_pretty(model).unwrap()).unwrap();
    path
}

/// A fresh store imports entities and relationships from the JSON file.
#[tokio::test]
async fn migrate_imports_entities_and_relationships() {
    let dir = tempfile::tempdir().unwrap();
    let model = sample_model();
    let source = write_model(dir.path(), &model);
    let sb = Sandbox::new(dir).await;

    let report = sb.store.migrate_from_json(&source).await.unwrap();
    assert!(!report.skipped);
    assert_eq!(report.entities_imported, 2);
    assert_eq!(report.relationships_imported, 1);
    assert_eq!(report.source_file, Some(source.clone()));

    // Entities are now retrievable.
    let all = sb
        .store
        .search_entities(&life_world_store::SearchQuery {
            entity_type: Some("person".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    let alice = all
        .iter()
        .find(|e| e.name == "Alice")
        .expect("alice imported");
    assert_eq!(alice.lifecycle, memory_core::wm::EntityLifecycle::Active);
}

/// Re-running the migration is a no-op (marker set on the first import).
#[tokio::test]
async fn migrate_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let source = write_model(dir.path(), &sample_model());
    let sb = Sandbox::new(dir).await;

    let first = sb.store.migrate_from_json(&source).await.unwrap();
    assert_eq!(first.entities_imported, 2);

    let second = sb.store.migrate_from_json(&source).await.unwrap();
    assert!(second.skipped, "second migration must be skipped");
    assert_eq!(second.entities_imported, 0);
}

/// A missing source file yields a `skipped` report, not an error.
#[tokio::test]
async fn migrate_missing_source_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist.json");
    let sb = Sandbox::new(dir).await;

    let report = sb.store.migrate_from_json(&missing).await.unwrap();
    assert!(report.skipped);
    assert_eq!(report.entities_imported, 0);
    assert_eq!(report.relationships_imported, 0);
}

/// Malformed JSON is a Migration error and leaves no rows behind.
#[tokio::test]
async fn migrate_bad_json_errors_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("bad.json");
    std::fs::write(&source, "{ this is not json").unwrap();
    let sb = Sandbox::new(dir).await;

    let err = sb.store.migrate_from_json(&source).await.unwrap_err();
    assert!(matches!(
        err,
        life_world_store::WorldStoreError::Migration(_)
    ));

    let conn = sb.raw();
    let entities: i64 = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
        .unwrap();
    let rels: i64 = conn
        .query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))
        .unwrap();
    assert_eq!(entities, 0);
    assert_eq!(rels, 0);
}

/// The source file is backed up to a `.bak-<secs>` before import.
#[tokio::test]
async fn migrate_backs_up_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = write_model(dir.path(), &sample_model());
    let original = std::fs::read(&source).unwrap();
    let sb = Sandbox::new(dir).await;

    let report = sb.store.migrate_from_json(&source).await.unwrap();
    let backup = report.source_backup.expect("a backup must exist");
    assert!(backup.exists(), "backup file must exist on disk");
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    assert_ne!(backup.file_name(), source.file_name());
}

/// Migrated entities are provenance-tagged as `migration`.
#[tokio::test]
async fn migrate_tags_entities_as_migration() {
    let dir = tempfile::tempdir().unwrap();
    let source = write_model(dir.path(), &sample_model());
    let sb = Sandbox::new(dir).await;
    sb.store.migrate_from_json(&source).await.unwrap();

    let conn = sb.raw();
    let tags: Vec<String> = conn
        .prepare("SELECT created_by FROM entities ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(tags, vec!["migration".to_string(), "migration".to_string()]);
}
