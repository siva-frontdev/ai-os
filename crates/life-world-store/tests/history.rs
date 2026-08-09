//! Version history: every operation records a snapshot at the right version.

mod common;

use common::{Sandbox, add_entity, ai_ctx};

use life_world_store::{Operation, UpdateEntity, WorldStore};
use memory_core::wm::Entity;

/// A create → update → update → archive → restore lifecycle records one
/// history entry per version with the correct snapshot.
#[tokio::test]
async fn entity_history_records_full_lifecycle() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    let id = e.id;

    sb.store
        .update_entity(UpdateEntity {
            id,
            expected_version: 1,
            name: Some("Alice A".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();
    sb.store
        .update_entity(UpdateEntity {
            id,
            expected_version: 2,
            name: Some("Alice B".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();
    sb.store
        .archive_entity(&id, 3, "reason", &ai_ctx())
        .await
        .unwrap();
    sb.store.restore_entity(&id, 4, &ai_ctx()).await.unwrap();

    let history = sb.store.get_history(&id).await.unwrap();
    assert_eq!(
        history.len(),
        5,
        "expected one entry per version: {history:?}"
    );
    assert_eq!(history[0].version, 1);
    assert_eq!(history[1].version, 2);
    assert_eq!(history[2].version, 3);
    assert_eq!(history[3].version, 4);
    assert_eq!(history[4].version, 5);

    let ops: Vec<Operation> = history.iter().map(|h| h.operation).collect();
    assert_eq!(
        ops,
        vec![
            Operation::Create,
            Operation::Update,
            Operation::Update,
            Operation::Archive,
            Operation::Restore
        ]
    );

    // Every snapshot is the full entity at that version.
    for (i, entry) in history.iter().enumerate() {
        let snap: Entity = serde_json::from_value(entry.snapshot.clone()).unwrap();
        assert_eq!(snap.version as usize, i + 1, "snapshot {i} wrong version");
        assert_eq!(snap.id, id);
    }
    // Name changes are reflected in the matching snapshots.
    let s2: Entity = serde_json::from_value(history[1].snapshot.clone()).unwrap();
    assert_eq!(s2.name, "Alice A");
    let s3: Entity = serde_json::from_value(history[2].snapshot.clone()).unwrap();
    assert_eq!(s3.name, "Alice B");
    let s4: Entity = serde_json::from_value(history[3].snapshot.clone()).unwrap();
    assert_eq!(s4.lifecycle, memory_core::wm::EntityLifecycle::Archived);
    let s5: Entity = serde_json::from_value(history[4].snapshot.clone()).unwrap();
    assert_eq!(s5.lifecycle, memory_core::wm::EntityLifecycle::Active);
}

/// History carries changed_by / provenance / changed_at for each entry.
#[tokio::test]
async fn history_entries_carry_actor_and_provenance() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = sb
        .store
        .create_entity(common::create_entity("person", "Alice"))
        .await
        .unwrap();
    sb.store
        .update_entity(UpdateEntity {
            id: e.id,
            expected_version: 1,
            name: Some("A".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: common::observation_ctx("obs-7"),
        })
        .await
        .unwrap();

    let history = sb.store.get_history(&e.id).await.unwrap();
    assert_eq!(history[0].changed_by, "ai");
    assert_eq!(history[1].changed_by, "observation");
    assert_eq!(history[1].provenance.as_deref(), Some("obs-7"));
    assert!(history[0].changed_at.as_secs() > 0);
}

/// get_history for an entity that never had history returns an empty list.
#[tokio::test]
async fn history_empty_for_unknown_entity() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let history = sb
        .store
        .get_history(&memory_core::wm::EntityId::new())
        .await
        .unwrap();
    assert!(history.is_empty());
}

/// Relationship history records create + update + delete operations.
#[tokio::test]
async fn relationship_history_records_ops() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;
    let rel = common::add_relationship(&sb.store, "knows", &a.id, &b.id).await;

    sb.store
        .update_relationship(life_world_store::UpdateRelationship {
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
    sb.store
        .delete_relationship(&rel.id, 2, &ai_ctx())
        .await
        .unwrap();

    let rows = common::relationship_history_rows(&sb.raw(), &rel.id);
    let ops: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(ops, vec!["create", "update", "delete"]);
    let versions: Vec<i64> = rows.iter().map(|r| r.0).collect();
    assert_eq!(versions, vec![1, 2, 3], "delete tombstone at version+1");
}
