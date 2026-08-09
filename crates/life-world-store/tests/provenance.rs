//! Provenance / WriteContext: every actor tag is recorded, provenance survives
//! persistence and retrieval.

mod common;

use std::collections::HashMap;

use common::{Sandbox, add_entity, migration_ctx, runtime_ctx, user_ctx};

use life_world_store::{Actor, CreateEntity, WorldStore, WriteContext};

/// All five writer categories produce the documented `created_by` tag.
#[tokio::test]
async fn every_actor_tag_is_recorded() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;

    let cases: Vec<(WriteContext, &str)> = vec![
        (common::ctx(Actor::Ai), "ai"),
        (user_ctx(), "user"),
        (common::ctx(Actor::Observation), "observation"),
        (runtime_ctx(), "runtime"),
        (migration_ctx(), "migration"),
    ];

    for (i, (ctx, expected_tag)) in cases.into_iter().enumerate() {
        let e = sb
            .store
            .create_entity(CreateEntity {
                entity_type: "person".into(),
                name: format!("p{i}"),
                properties: HashMap::new(),
                importance: 0.5,
                confidence: 0.5,
                metadata: HashMap::new(),
                ctx,
            })
            .await
            .unwrap();
        let (created_by, _) = common::entity_provenance(&sb.raw(), &e.id);
        assert_eq!(created_by, expected_tag, "entity p{i}");
    }
}

/// Provenance survives persistence and is visible through the public history API.
#[tokio::test]
async fn provenance_survives_persistence_and_retrieval() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = sb
        .store
        .create_entity(CreateEntity {
            entity_type: "person".into(),
            name: "Alice".into(),
            properties: HashMap::new(),
            importance: 0.5,
            confidence: 0.5,
            metadata: HashMap::new(),
            ctx: common::observation_ctx("gmail-msg-123"),
        })
        .await
        .unwrap();

    let history = sb.store.get_history(&e.id).await.unwrap();
    assert_eq!(history[0].changed_by, "observation");
    assert_eq!(history[0].provenance.as_deref(), Some("gmail-msg-123"));
}

/// Provenance survives a close/reopen cycle.
#[tokio::test]
async fn provenance_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let sb = Sandbox::new(dir).await;
    let e = sb
        .store
        .create_entity(CreateEntity {
            entity_type: "person".into(),
            name: "Alice".into(),
            properties: HashMap::new(),
            importance: 0.5,
            confidence: 0.5,
            metadata: HashMap::new(),
            ctx: WriteContext {
                actor: Actor::User {
                    user_id: "u42".into(),
                },
                provenance: Some("user-action-42".into()),
            },
        })
        .await
        .unwrap();

    let store = sb.reopen().await;
    let conn = rusqlite::Connection::open(&sb.db_path).expect("open raw connection");
    let (created_by, provenance) = common::entity_provenance(&conn, &e.id);
    assert_eq!(created_by, "user");
    assert_eq!(provenance.as_deref(), Some("user-action-42"));

    // The reopened store still serves the same data.
    let got = store.get_entity(&e.id).await.unwrap().unwrap();
    assert_eq!(got.name, "Alice");
}

/// Relationship writes carry the writer's tag too.
#[tokio::test]
async fn relationship_provenance_recorded() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = common::add_relationship(&sb.store, "knows", &a.id, &b.id).await;
    let (created_by, _) = common::relationship_provenance(&sb.raw(), &rel.id);
    assert_eq!(created_by, "ai");
}

/// Document writes carry the writer's tag and provenance.
#[tokio::test]
async fn document_provenance_recorded() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(common::create_doc("memo", b"content", vec![]))
        .await
        .unwrap();
    let (created_by, provenance) = common::document_provenance(&sb.raw(), &doc.id);
    assert_eq!(created_by, "ai");
    assert_eq!(provenance, None);
}

/// The `actor` tag matches its documented string form.
#[test]
fn actor_tag_matches_docs() {
    assert_eq!(Actor::Ai.tag(), "ai");
    assert_eq!(
        Actor::User {
            user_id: "x".into()
        }
        .tag(),
        "user"
    );
    assert_eq!(Actor::Observation.tag(), "observation");
    assert_eq!(Actor::Runtime.tag(), "runtime");
    assert_eq!(Actor::Migration.tag(), "migration");
}
