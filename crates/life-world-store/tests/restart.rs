//! Durability: every data kind survives a close/reopen cycle and a fresh
//! store over the same database.

mod common;

use common::{Sandbox, add_entity, add_relationship, ai_ctx, goal, lesson, observation};

use life_world_store::{Operation, UpdateEntity, WorldStore, WorldStoreConfig};

/// Full-world durability: all data kinds survive a reopen.
#[tokio::test]
async fn all_data_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let sb = Sandbox::new(dir).await;

    // Entity + relationship.
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = add_relationship(&sb.store, "knows", &a.id, &b.id).await;
    // Update + archive history.
    sb.store
        .update_entity(UpdateEntity {
            id: a.id,
            expected_version: 1,
            name: Some("Alice Smith".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();
    // Document + blob.
    let doc = common::add_doc(&sb.store, "memo", b"durable bytes").await;
    // Observation.
    let obs = observation("email");
    let obs_id = sb.store.record_observation(obs.clone()).await.unwrap();
    // Goal + lesson.
    let goal = sb.store.upsert_goal(goal()).await.unwrap();
    let lesson = sb.store.upsert_lesson(lesson()).await.unwrap();

    // Reopen with a brand-new store over the same database.
    let store = sb.reopen().await;

    let got_a = store.get_entity(&a.id).await.unwrap().unwrap();
    assert_eq!(got_a.name, "Alice Smith");
    assert_eq!(got_a.version, 2);

    let got_rel = store.get_relationship(&rel.id).await.unwrap().unwrap();
    assert_eq!(got_rel.relationship_type, "knows");

    let got_doc = store.get_document(&doc.id).await.unwrap().unwrap();
    assert_eq!(got_doc.title, "memo");
    assert_eq!(
        std::fs::read(sb.documents_dir.join(format!("{}.bin", doc.id))).unwrap(),
        b"durable bytes".to_vec()
    );

    let got_obs = store.get_observation(&obs_id).await.unwrap().unwrap();
    assert_eq!(got_obs.kind, "email");
    assert_eq!(got_obs.payload, obs.payload);

    let got_goal = store.get_goal(&goal.goal_id).await.unwrap().unwrap();
    assert_eq!(got_goal, goal);
    let got_lesson = store.get_lesson(&lesson.lesson_id).await.unwrap().unwrap();
    assert_eq!(got_lesson, lesson);

    // History survived too.
    let history = store.get_history(&a.id).await.unwrap();
    let ops: Vec<Operation> = history.iter().map(|h| h.operation).collect();
    assert_eq!(ops, vec![Operation::Create, Operation::Update]);
}

/// `close()` checkpoints and the data remains readable by a fresh store.
#[tokio::test]
async fn data_survives_close_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let sb = Sandbox::new(dir).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    sb.store.close().await.unwrap();

    let store = sb.reopen().await;
    let got = store.get_entity(&e.id).await.unwrap().unwrap();
    assert_eq!(got.name, "Alice");
}

/// A fresh store initialised over an existing database preserves everything
/// (schema is idempotent and WAL sidecars are re-attached).
#[tokio::test]
async fn fresh_store_over_existing_db_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let sb = Sandbox::new(dir).await;
    let e = add_entity(&sb.store, "person", "Alice").await;

    let cfg = WorldStoreConfig {
        db_path: sb.db_path.clone(),
        documents_dir: sb.documents_dir.clone(),
        wal: true,
        backup_count: 3,
        enable_fts: true,
        json_migration_source: None,
    };
    let store = life_world_store::SqliteWorldModelStore::new(cfg);
    store.init().await.unwrap();
    store.init().await.unwrap(); // idempotent re-init

    let got = store.get_entity(&e.id).await.unwrap().unwrap();
    assert_eq!(got.name, "Alice");
}

/// Hard-delete tombstones survive a reopen.
#[tokio::test]
async fn tombstones_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let sb = Sandbox::new(dir).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = add_relationship(&sb.store, "knows", &a.id, &b.id).await;
    sb.store
        .delete_relationship(&rel.id, 1, &ai_ctx())
        .await
        .unwrap();
    sb.store
        .delete_entity(&a.id, 1, true, &ai_ctx())
        .await
        .unwrap();

    let store = sb.reopen().await;
    let rows = common::relationship_history_rows(&sb.raw(), &rel.id);
    assert_eq!(rows.len(), 2, "relationship tombstones preserved");
    let rows = common::entity_history_rows(&sb.raw(), &a.id);
    assert_eq!(rows.len(), 1, "entity tombstone preserved");
    assert_eq!(rows[0].1, "delete");
    assert!(store.get_entity(&a.id).await.unwrap().is_none());
}
