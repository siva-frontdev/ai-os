//! Entity CRUD: create/get/update/archive/restore/delete, field preservation,
//! missing-entity and validation behaviour.

mod common;

use std::collections::HashMap;

use common::{Sandbox, add_entity, ai_ctx, create_entity};

use life_world_store::{
    Actor, CreateEntity, UpdateEntity, WorldStore, WorldStoreError, WriteContext,
};
use memory_core::wm::{EntityId, EntityLifecycle, Value};

/// A freshly created entity carries generated UUID, defaults, and timestamps.
#[tokio::test]
async fn create_entity_preserves_fields() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut props = HashMap::new();
    props.insert("email".to_string(), Value::String("a@b.c".into()));
    props.insert("age".to_string(), Value::Number(42.0));
    let mut meta = HashMap::new();
    meta.insert("source".to_string(), "test".into());

    let e = sb
        .store
        .create_entity(CreateEntity {
            entity_type: "person".into(),
            name: "Alice".into(),
            properties: props.clone(),
            importance: 0.8,
            confidence: 0.9,
            metadata: meta.clone(),
            ctx: common::observation_ctx("obs-1"),
        })
        .await
        .unwrap();

    assert_eq!(e.entity_type, "person");
    assert_eq!(e.name, "Alice");
    assert_eq!(e.version, 1);
    assert_eq!(e.lifecycle, EntityLifecycle::Active);
    assert_eq!(e.importance, 0.8);
    assert_eq!(e.confidence, 0.9);
    common::assert_value_map_eq(&e.properties, &props);
    assert_eq!(e.metadata, meta);
    assert!(e.created_at.as_secs() > 0);
    assert_eq!(e.created_at, e.updated_at);
}

/// `create_entity` returns a fresh UUID each time and preserves it in the store.
#[tokio::test]
async fn create_entity_uuid_is_preserved() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    assert_ne!(a.id, b.id);

    let got = sb.store.get_entity(&a.id).await.unwrap().unwrap();
    assert_eq!(got.id, a.id);
}

/// Free-form entity types are preserved verbatim and tagged in metadata.
#[tokio::test]
async fn create_entity_normalizes_freeform_type() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = sb
        .store
        .create_entity(create_entity("quantum-bridge", "QB1"))
        .await
        .unwrap();
    assert_eq!(e.entity_type, "quantum-bridge");
    assert_eq!(
        e.metadata.get("original_type"),
        Some(&"quantum-bridge".to_string())
    );
}

/// Importance and confidence are clamped into 0..1 at the boundary.
#[tokio::test]
async fn create_entity_clamps_importance_and_confidence() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut req = create_entity("person", "Edge");
    req.importance = 5.0;
    req.confidence = -3.0;
    let e = sb.store.create_entity(req).await.unwrap();
    assert_eq!(e.importance, 1.0);
    assert_eq!(e.confidence, 0.0);
}

/// get_entity returns None for a missing entity.
#[tokio::test]
async fn get_missing_entity_returns_none() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let ghost = EntityId::new();
    assert!(sb.store.get_entity(&ghost).await.unwrap().is_none());
}

/// update_entity changes only the supplied fields and bumps the version.
#[tokio::test]
async fn update_entity_changes_supplied_fields_only() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    let mut props = HashMap::new();
    props.insert("email".to_string(), Value::String("new@x.y".into()));

    let updated = sb
        .store
        .update_entity(UpdateEntity {
            id: e.id,
            expected_version: 1,
            name: Some("Alice Smith".into()),
            properties: Some(props.clone()),
            importance: Some(0.1),
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();

    assert_eq!(updated.version, 2);
    assert_eq!(updated.name, "Alice Smith");
    common::assert_value_map_eq(&updated.properties, &props);
    assert_eq!(updated.importance, 0.1);
    // Un-supplied fields survive.
    assert_eq!(updated.confidence, 0.5);
    assert_eq!(updated.entity_type, "person");
}

/// A stale `expected_version` yields VersionConflict and no mutation.
#[tokio::test]
async fn update_entity_version_conflict() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    sb.store
        .update_entity(UpdateEntity {
            id: e.id,
            expected_version: 1,
            name: Some("A".into()),
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
        .update_entity(UpdateEntity {
            id: e.id,
            expected_version: 1,
            name: Some("B".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
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

    // The failed write must not have mutated anything.
    let now = sb.store.get_entity(&e.id).await.unwrap().unwrap();
    assert_eq!(now.name, "A");
    assert_eq!(now.version, 2);
}

/// Updating a missing entity returns EntityNotFound.
#[tokio::test]
async fn update_missing_entity_not_found() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let err = sb
        .store
        .update_entity(UpdateEntity {
            id: EntityId::new(),
            expected_version: 1,
            name: Some("x".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, WorldStoreError::EntityNotFound(_)),
        "got {err:?}"
    );
}

/// archive_entity flips the lifecycle to Archived and bumps the version.
#[tokio::test]
async fn archive_and_restore_lifecycle() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;

    sb.store
        .archive_entity(&e.id, 1, "cleanup", &ai_ctx())
        .await
        .unwrap();
    let archived = sb.store.get_entity(&e.id).await.unwrap().unwrap();
    assert_eq!(archived.lifecycle, EntityLifecycle::Archived);
    assert_eq!(archived.version, 2);

    sb.store.restore_entity(&e.id, 2, &ai_ctx()).await.unwrap();
    let restored = sb.store.get_entity(&e.id).await.unwrap().unwrap();
    assert_eq!(restored.lifecycle, EntityLifecycle::Active);
    assert_eq!(restored.version, 3);
}

/// archive_entity rejects a stale version without mutating the row.
#[tokio::test]
async fn archive_entity_version_conflict() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    sb.store
        .update_entity(UpdateEntity {
            id: e.id,
            expected_version: 1,
            name: Some("A".into()),
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
        .archive_entity(&e.id, 1, "x", &ai_ctx())
        .await
        .unwrap_err();
    assert!(matches!(err, WorldStoreError::VersionConflict { .. }));
    let now = sb.store.get_entity(&e.id).await.unwrap().unwrap();
    assert_eq!(now.lifecycle, EntityLifecycle::Active);
}

/// Soft delete (hard=false) archives and retains history; the row survives.
#[tokio::test]
async fn soft_delete_archives() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    sb.store
        .delete_entity(&e.id, 1, false, &ai_ctx())
        .await
        .unwrap();
    let got = sb.store.get_entity(&e.id).await.unwrap().unwrap();
    assert_eq!(got.lifecycle, EntityLifecycle::Archived);
}

/// Hard delete removes the row, its history (minus the tombstone), its
/// relationships, and their history.
#[tokio::test]
async fn hard_delete_removes_row_and_history() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = common::add_relationship(&sb.store, "knows", &a.id, &b.id).await;

    sb.store
        .delete_entity(&a.id, 1, true, &ai_ctx())
        .await
        .unwrap();

    assert!(sb.store.get_entity(&a.id).await.unwrap().is_none());
    assert!(sb.store.get_relationship(&rel.id).await.unwrap().is_none());

    // Only the Delete tombstone remains in entity history.
    let rows = common::entity_history_rows(&sb.raw(), &a.id);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "delete");
    // Relationship history was removed entirely.
    assert!(common::relationship_history_rows(&sb.raw(), &rel.id).is_empty());
}

/// Hard deleting a missing entity returns EntityNotFound.
#[tokio::test]
async fn hard_delete_missing_entity_not_found() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let err = sb
        .store
        .delete_entity(&EntityId::new(), 1, true, &ai_ctx())
        .await
        .unwrap_err();
    assert!(matches!(err, WorldStoreError::EntityNotFound(_)));
}

/// Write context actor + provenance survive and are recorded on the row.
#[tokio::test]
async fn created_by_and_provenance_recorded() {
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
            ctx: WriteContext {
                actor: Actor::User {
                    user_id: "u1".into(),
                },
                provenance: Some("user-action-9".into()),
            },
        })
        .await
        .unwrap();

    let (created_by, provenance) = common::entity_provenance(&sb.raw(), &e.id);
    assert_eq!(created_by, "user");
    assert_eq!(provenance.as_deref(), Some("user-action-9"));
}

/// An empty entity_type is rejected at the boundary (RFC-0008 §Security).
#[tokio::test]
async fn create_entity_rejects_empty_entity_type() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut req = create_entity("person", "Alice");
    req.entity_type = "   ".into();
    let err = sb.store.create_entity(req).await.unwrap_err();
    assert!(
        matches!(err, WorldStoreError::InvalidEntityType(_)),
        "got {err:?}"
    );
}

/// An over-length entity_type is rejected (RFC-0008 length limits).
#[tokio::test]
async fn create_entity_rejects_overlong_entity_type() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut req = create_entity("person", "Alice");
    req.entity_type = "x".repeat(200);
    let err = sb.store.create_entity(req).await.unwrap_err();
    assert!(
        matches!(err, WorldStoreError::InvalidEntityType(_)),
        "got {err:?}"
    );
}
