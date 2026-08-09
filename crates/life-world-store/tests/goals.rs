//! Goals: durable upsert, versioning, mirror entity rows, and validation.

mod common;

use common::{Sandbox, goal};

use life_world_store::{WorldStore, WorldStoreError};

/// A first upsert creates the goal at version 1.
#[tokio::test]
async fn upsert_goal_creates_at_version_one() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let stored = sb.store.upsert_goal(goal()).await.unwrap();
    assert_eq!(stored.version, 1);
    assert_eq!(stored.goal_type, "task");

    let got = sb.store.get_goal(&stored.goal_id).await.unwrap().unwrap();
    assert_eq!(got, stored);
}

/// Re-upserting with a matching version bumps to version 2.
#[tokio::test]
async fn upsert_goal_updates_version() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let g = goal();
    let created = sb.store.upsert_goal(g.clone()).await.unwrap();

    let mut updated = created.clone();
    updated.status = "completed".into();
    updated.description = "Ship the release, done".into();
    updated.progress_pct = 100.0;
    let stored = sb.store.upsert_goal(updated.clone()).await.unwrap();

    assert_eq!(stored.version, 2);
    assert_eq!(stored.status, "completed");
    assert_eq!(stored.progress_pct, 100.0);
    let got = sb.store.get_goal(&g.goal_id).await.unwrap().unwrap();
    assert_eq!(got.description, "Ship the release, done");
}

/// A stale goal version yields VersionConflict.
#[tokio::test]
async fn upsert_goal_version_conflict() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let created = sb.store.upsert_goal(goal()).await.unwrap();
    assert_eq!(created.version, 1);

    let mut stale = created.clone();
    stale.version = 1;
    sb.store.upsert_goal(stale.clone()).await.unwrap();

    // Another writer at version 1 now conflicts (stored is 2).
    let err = sb.store.upsert_goal(stale).await.unwrap_err();
    assert!(
        matches!(err, WorldStoreError::VersionConflict { .. }),
        "got {err:?}"
    );
}

/// A goal is mirrored as an `entities` row so it is visible in the property
/// graph.
#[tokio::test]
async fn goal_is_mirrored_as_entity() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let g = goal();
    let stored = sb.store.upsert_goal(g.clone()).await.unwrap();

    let entity = sb
        .store
        .get_entity(&memory_core::wm::EntityId::from_uuid(
            uuid::Uuid::parse_str(&stored.goal_id).unwrap(),
        ))
        .await
        .unwrap()
        .expect("goal entity row must exist");
    assert_eq!(entity.entity_type, "goal");
}

/// A non-UUID goal id is rejected before any row is written.
#[tokio::test]
async fn upsert_goal_rejects_invalid_id() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut g = goal();
    g.goal_id = "not-a-uuid".into();

    let err = sb.store.upsert_goal(g).await.unwrap_err();
    assert!(matches!(err, WorldStoreError::Validation(_)), "got {err:?}");

    // No orphaned goals row.
    let conn = sb.raw();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM goals", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

/// get_goal returns None for an unknown id.
#[tokio::test]
async fn get_missing_goal_returns_none() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let ghost = uuid::Uuid::new_v4().to_string();
    assert!(sb.store.get_goal(&ghost).await.unwrap().is_none());
}

/// all_goals lists every stored goal.
#[tokio::test]
async fn all_goals_lists_every_goal() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let g1 = sb.store.upsert_goal(goal()).await.unwrap();

    let mut g2 = goal();
    g2.description = "Second goal".into();
    let g2 = sb.store.upsert_goal(g2).await.unwrap();

    let all = sb.store.all_goals().await.unwrap();
    let ids: Vec<String> = all.iter().map(|g| g.goal_id.clone()).collect();
    assert_eq!(all.len(), 2);
    assert!(ids.contains(&g1.goal_id));
    assert!(ids.contains(&g2.goal_id));
}

/// Goals survive a close/reopen cycle.
#[tokio::test]
async fn goals_survive_restart() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let stored = sb.store.upsert_goal(goal()).await.unwrap();

    let store = sb.reopen().await;
    let got = store.get_goal(&stored.goal_id).await.unwrap().unwrap();
    assert_eq!(got, stored);
}
