//! Relationship CRUD, endpoint validation, concurrency, and history.

mod common;

use std::collections::HashMap;

use common::{Sandbox, add_entity, add_relationship, ai_ctx};

use life_world_store::{CreateRelationship, UpdateRelationship, WorldStore, WorldStoreError};
use memory_core::wm::{EntityId, RelationshipId, Value};

/// Create a relationship preserves endpoints, type, confidence, weight.
#[tokio::test]
async fn create_relationship_preserves_fields() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "company", "ACME").await;
    let mut props = HashMap::new();
    props.insert("since".to_string(), Value::Number(2020.0));

    let rel = sb
        .store
        .create_relationship(CreateRelationship {
            relationship_type: "works_at".into(),
            source_id: a.id,
            target_id: b.id,
            properties: props.clone(),
            confidence: 0.9,
            weight: 0.5,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();

    assert_eq!(rel.relationship_type, "works_at");
    assert_eq!(rel.source_id, a.id);
    assert_eq!(rel.target_id, b.id);
    common::assert_value_map_eq(&rel.properties, &props);
    assert_eq!(rel.confidence, 0.9);
    assert_eq!(rel.weight, 0.5);
    assert_eq!(rel.version, 1);
    assert!(rel.created_at.as_secs() > 0);
}

/// get_relationship round-trips and returns None for a missing id.
#[tokio::test]
async fn get_relationship_roundtrip() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = add_relationship(&sb.store, "knows", &a.id, &b.id).await;

    let got = sb.store.get_relationship(&rel.id).await.unwrap().unwrap();
    assert_eq!(got.id, rel.id);
    assert_eq!(got.relationship_type, "knows");

    let ghost = RelationshipId::new();
    assert!(sb.store.get_relationship(&ghost).await.unwrap().is_none());
}

/// update_relationship changes only supplied fields and bumps the version.
#[tokio::test]
async fn update_relationship_changes_supplied_fields() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "company", "ACME").await;
    let rel = add_relationship(&sb.store, "works_at", &a.id, &b.id).await;

    let mut props = HashMap::new();
    props.insert("role".to_string(), Value::String("engineer".into()));
    let updated = sb
        .store
        .update_relationship(UpdateRelationship {
            id: rel.id,
            expected_version: 1,
            relationship_type: Some("leads".into()),
            properties: Some(props.clone()),
            confidence: Some(0.8),
            weight: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();

    assert_eq!(updated.version, 2);
    assert_eq!(updated.relationship_type, "leads");
    common::assert_value_map_eq(&updated.properties, &props);
    assert_eq!(updated.confidence, 0.8);
    assert_eq!(updated.weight, 1.0, "un-supplied weight survives");
}

/// A stale relationship version yields VersionConflict without mutation.
#[tokio::test]
async fn update_relationship_version_conflict() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = add_relationship(&sb.store, "knows", &a.id, &b.id).await;

    sb.store
        .update_relationship(UpdateRelationship {
            id: rel.id,
            expected_version: 1,
            relationship_type: Some("friends".into()),
            properties: None,
            confidence: None,
            weight: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();

    let err = sb
        .store
        .update_relationship(UpdateRelationship {
            id: rel.id,
            expected_version: 1,
            relationship_type: Some("rivals".into()),
            properties: None,
            confidence: None,
            weight: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            WorldStoreError::VersionConflict {
                expected: 1,
                actual: 2,
                ..
            }
        ),
        "got {err:?}"
    );
    let now = sb.store.get_relationship(&rel.id).await.unwrap().unwrap();
    assert_eq!(now.relationship_type, "friends");
}

/// Updating or deleting a missing relationship returns RelationshipNotFound.
#[tokio::test]
async fn missing_relationship_errors() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let ghost = RelationshipId::new();
    let upd = sb
        .store
        .update_relationship(UpdateRelationship {
            id: ghost,
            expected_version: 1,
            relationship_type: None,
            properties: None,
            confidence: None,
            weight: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap_err();
    assert!(matches!(upd, WorldStoreError::RelationshipNotFound(_)));

    let del = sb
        .store
        .delete_relationship(&ghost, 1, &ai_ctx())
        .await
        .unwrap_err();
    assert!(matches!(del, WorldStoreError::RelationshipNotFound(_)));
}

/// Creating a relationship with a missing endpoint is rejected.
#[tokio::test]
async fn create_relationship_requires_existing_endpoints() {
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
    assert!(res.is_err(), "endpoint must exist");
}

/// delete_relationship removes the row and keeps the tombstone history.
#[tokio::test]
async fn delete_relationship_removes_row_and_keeps_history() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = add_relationship(&sb.store, "knows", &a.id, &b.id).await;

    sb.store
        .delete_relationship(&rel.id, 1, &ai_ctx())
        .await
        .unwrap();

    assert!(sb.store.get_relationship(&rel.id).await.unwrap().is_none());

    // Create + Delete tombstone retained in relationship_history.
    let rows = common::relationship_history_rows(&sb.raw(), &rel.id);
    assert_eq!(
        rows.len(),
        2,
        "expected create + delete tombstones: {rows:?}"
    );
    assert_eq!(rows[0].1, "create");
    assert_eq!(rows[1].1, "delete");
}

/// A stale delete yields VersionConflict and the row survives.
#[tokio::test]
async fn delete_relationship_version_conflict() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = add_relationship(&sb.store, "knows", &a.id, &b.id).await;

    sb.store
        .update_relationship(UpdateRelationship {
            id: rel.id,
            expected_version: 1,
            relationship_type: Some("friends".into()),
            properties: None,
            confidence: None,
            weight: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();

    let err = sb
        .store
        .delete_relationship(&rel.id, 1, &ai_ctx())
        .await
        .unwrap_err();
    assert!(matches!(err, WorldStoreError::VersionConflict { .. }));
    assert!(sb.store.get_relationship(&rel.id).await.unwrap().is_some());
}

/// Relationship provenance is recorded and survives.
#[tokio::test]
async fn relationship_provenance_recorded() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = sb
        .store
        .create_relationship(CreateRelationship {
            relationship_type: "knows".into(),
            source_id: a.id,
            target_id: b.id,
            properties: HashMap::new(),
            confidence: 0.7,
            weight: 1.0,
            ctx: common::observation_ctx("obs-42"),
        })
        .await
        .unwrap();

    let (created_by, provenance) = common::relationship_provenance(&sb.raw(), &rel.id);
    assert_eq!(created_by, "observation");
    assert_eq!(provenance.as_deref(), Some("obs-42"));
}
