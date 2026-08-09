//! Atomicity: failed operations leave no partial rows, and multi-statement
//! operations commit or roll back as one unit.

mod common;

use std::collections::HashMap;

use common::{Sandbox, add_entity, ai_ctx};

use life_world_store::{
    CreateRelationship, DocumentEntityLink, UpdateDocument, UpdateEntity, WorldStore,
    WorldStoreError,
};
use memory_core::wm::EntityId;

/// A failed document create (missing link entity) leaves no DB rows behind.
#[tokio::test]
async fn failed_document_create_leaves_no_rows() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let ghost = EntityId::new();
    let req = common::create_doc(
        "memo",
        b"data",
        vec![DocumentEntityLink {
            entity_id: ghost,
            role: "about".into(),
            confidence: 0.9,
        }],
    );
    assert!(sb.store.create_document(req).await.is_err());

    let conn = sb.raw();
    let docs: i64 = conn
        .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
        .unwrap();
    let content: i64 = conn
        .query_row("SELECT COUNT(*) FROM document_content", [], |r| r.get(0))
        .unwrap();
    let ents: i64 = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(docs, 0, "no document row after a failed create");
    assert_eq!(content, 0, "no content row after a failed create");
    assert_eq!(ents, 0, "no entity row after a failed create");
}

/// A stale document update does not mutate the stored row.
#[tokio::test]
async fn conflicting_document_update_keeps_row() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = common::add_doc(&sb.store, "memo", b"v1").await;

    let err = sb
        .store
        .update_document(UpdateDocument {
            id: doc.id,
            expected_version: 99,
            title: Some("should not land".into()),
            tags: None,
            content: None,
            extracted_text: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap_err();
    assert!(matches!(err, WorldStoreError::VersionConflict { .. }));

    let now = sb.store.get_document(&doc.id).await.unwrap().unwrap();
    assert_eq!(now.title, "memo");
    assert_eq!(now.version, 1);
}

/// A failed relationship create (missing endpoint) leaves no relationship row.
#[tokio::test]
async fn failed_relationship_create_leaves_no_row() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let ghost = EntityId::new();

    let res = sb
        .store
        .create_relationship(CreateRelationship {
            relationship_type: "knows".into(),
            source_id: ghost,
            target_id: b.id,
            properties: HashMap::new(),
            confidence: 0.5,
            weight: 0.5,
            ctx: ai_ctx(),
        })
        .await;
    assert!(
        res.is_err(),
        "a relationship to a missing endpoint must be rejected"
    );

    let conn = sb.raw();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

/// A failed hard delete (version conflict) changes nothing: the entity, its
/// relationships, and its history all survive.
#[tokio::test]
async fn conflicting_hard_delete_changes_nothing() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = common::add_relationship(&sb.store, "knows", &a.id, &b.id).await;

    sb.store
        .update_entity(UpdateEntity {
            id: a.id,
            expected_version: 1,
            name: Some("Alice v2".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();

    let err = sb
        .store
        .delete_entity(&a.id, 1, true, &ai_ctx())
        .await
        .unwrap_err();
    assert!(matches!(err, WorldStoreError::VersionConflict { .. }));

    assert!(sb.store.get_entity(&a.id).await.unwrap().is_some());
    assert!(sb.store.get_relationship(&rel.id).await.unwrap().is_some());

    // History still has create + update (nothing was pruned).
    let rows = common::entity_history_rows(&sb.raw(), &a.id);
    let ops: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(ops, vec!["create", "update"]);
}

/// Relationship delete commits the row removal and the tombstone together.
#[tokio::test]
async fn relationship_delete_is_atomic() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = common::add_relationship(&sb.store, "knows", &a.id, &b.id).await;

    sb.store
        .delete_relationship(&rel.id, 1, &ai_ctx())
        .await
        .unwrap();

    let conn = sb.raw();
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
    let rows = common::relationship_history_rows(&conn, &rel.id);
    assert_eq!(rows.len(), 2, "create + delete tombstones");
}

/// Repeated conflicting writes never append history.
#[tokio::test]
async fn stale_writes_never_append_history() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;

    // Advance to version 2 legitimately, then attack with stale version-1 writes.
    sb.store
        .update_entity(UpdateEntity {
            id: e.id,
            expected_version: 1,
            name: Some("Alice v2".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();

    for _ in 0..3 {
        let res = sb
            .store
            .update_entity(UpdateEntity {
                id: e.id,
                expected_version: 1,
                name: Some("stale".into()),
                properties: None,
                importance: None,
                confidence: None,
                metadata: None,
                ctx: ai_ctx(),
            })
            .await;
        assert!(matches!(res, Err(WorldStoreError::VersionConflict { .. })));
    }

    let rows = common::entity_history_rows(&sb.raw(), &e.id);
    assert_eq!(rows.len(), 2, "only create + the one successful update");
}
