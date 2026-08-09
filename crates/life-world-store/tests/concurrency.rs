//! Optimistic concurrency: stale writes conflict, versions advance exactly once,
//! and no partial mutation occurs.

mod common;

use common::{Sandbox, add_entity, ai_ctx};

use life_world_store::{UpdateEntity, WorldStore, WorldStoreError};

/// Classic lost-update scenario: client A bumps to v2, client B's stale v1
/// write conflicts and changes nothing.
#[tokio::test]
async fn stale_update_conflicts() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;

    // Client A reads v1, writes, lands on v2.
    let updated = sb
        .store
        .update_entity(UpdateEntity {
            id: e.id,
            expected_version: 1,
            name: Some("Alice (A)".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();
    assert_eq!(updated.version, 2);

    // Client B still holds v1 → conflict.
    let err = sb
        .store
        .update_entity(UpdateEntity {
            id: e.id,
            expected_version: 1,
            name: Some("Alice (B)".into()),
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

    // No partial mutation: B's name never landed.
    let now = sb.store.get_entity(&e.id).await.unwrap().unwrap();
    assert_eq!(now.name, "Alice (A)");
    assert_eq!(now.version, 2);
}

/// Two writers racing from the same base version: exactly one wins.
#[tokio::test]
async fn concurrent_updates_exactly_one_wins() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    let id = e.id;

    let (r1, r2) = tokio::join!(
        sb.store.update_entity(UpdateEntity {
            id,
            expected_version: 1,
            name: Some("From writer 1".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        }),
        sb.store.update_entity(UpdateEntity {
            id,
            expected_version: 1,
            name: Some("From writer 2".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        }),
    );

    let ok = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
    let conflicts = [&r1, &r2]
        .iter()
        .filter(|r| matches!(r, Err(WorldStoreError::VersionConflict { .. })))
        .count();
    assert_eq!(ok, 1, "exactly one writer must win: {r1:?} / {r2:?}");
    assert_eq!(conflicts, 1, "the loser must conflict: {r1:?} / {r2:?}");

    // Final row is at version 2 with exactly one of the two names.
    let now = sb.store.get_entity(&id).await.unwrap().unwrap();
    assert_eq!(now.version, 2);
    assert!(now.name == "From writer 1" || now.name == "From writer 2");
}

/// Version increments exactly once per successful write.
#[tokio::test]
async fn version_increments_exactly_once_per_write() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    let mut expected = 1u64;

    for i in 0..5u64 {
        let updated = sb
            .store
            .update_entity(UpdateEntity {
                id: e.id,
                expected_version: expected,
                name: Some(format!("gen {i}")),
                properties: None,
                importance: None,
                confidence: None,
                metadata: None,
                ctx: ai_ctx(),
            })
            .await
            .unwrap();
        expected += 1;
        assert_eq!(updated.version, expected);
    }
    assert_eq!(expected, 6);
    let history = sb.store.get_history(&e.id).await.unwrap();
    assert_eq!(history.len(), 6, "six versions recorded");
}

/// A failed concurrent write leaves no residue: history only contains the
/// successful writes.
#[tokio::test]
async fn conflicting_writes_do_not_append_history() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    let id = e.id;

    sb.store
        .update_entity(UpdateEntity {
            id,
            expected_version: 1,
            name: Some("first".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();

    // Stale write fails...
    let _ = sb
        .store
        .update_entity(UpdateEntity {
            id,
            expected_version: 1,
            name: Some("stale".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap_err();

    // ...and leaves history untouched.
    let history = sb.store.get_history(&id).await.unwrap();
    assert_eq!(history.len(), 2, "only create + the one successful update");
}
