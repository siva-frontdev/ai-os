//! Observations: durable record, retrieval, understanding-pipeline state,
//! and entity links.

mod common;

use common::{Sandbox, add_entity, observation};

use life_world_store::{ObservationId, ObservationProcessState, WorldStore, WorldStoreError};

/// A recorded observation round-trips with every field intact.
#[tokio::test]
async fn record_observation_roundtrip() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let obs = observation("email");
    let id = sb.store.record_observation(obs.clone()).await.unwrap();
    assert_eq!(id, obs.id, "the caller-supplied id is preserved");

    let got = sb.store.get_observation(&id).await.unwrap().unwrap();
    assert_eq!(got.id, obs.id);
    assert_eq!(got.kind, "email");
    assert_eq!(got.source.as_deref(), Some("test-provider"));
    assert_eq!(got.payload, obs.payload);
    assert_eq!(got.summary.as_deref(), Some("a natural language summary"));
    assert_eq!(got.observed_at, obs.observed_at);
    assert_eq!(got.confidence, obs.confidence);
    assert_eq!(got.processed, ObservationProcessState::Raw);
    assert_eq!(got.state_changes, obs.state_changes);
}

/// get_observation returns None for an unknown id.
#[tokio::test]
async fn get_missing_observation_returns_none() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let ghost = ObservationId::new();
    assert!(sb.store.get_observation(&ghost).await.unwrap().is_none());
}

/// mark_observation_processed advances the pipeline state and stores the
/// state-changes payload.
#[tokio::test]
async fn mark_processed_advances_state() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let obs = observation("system");
    let id = sb.store.record_observation(obs).await.unwrap();

    let changes = serde_json::json!({"op": "upsert", "path": "/entities/1"});
    sb.store
        .mark_observation_processed(&id, ObservationProcessState::Understood, Some(&changes))
        .await
        .unwrap();

    let got = sb.store.get_observation(&id).await.unwrap().unwrap();
    assert_eq!(got.processed, ObservationProcessState::Understood);
    assert_eq!(got.state_changes, Some(changes));

    sb.store
        .mark_observation_processed(&id, ObservationProcessState::Evolved, None)
        .await
        .unwrap();
    let got = sb.store.get_observation(&id).await.unwrap().unwrap();
    assert_eq!(got.processed, ObservationProcessState::Evolved);
}

/// Marking a missing observation processed returns ObservationNotFound.
#[tokio::test]
async fn mark_processed_missing_observation_errors() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let err = sb
        .store
        .mark_observation_processed(
            &ObservationId::new(),
            ObservationProcessState::Understood,
            None,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, WorldStoreError::ObservationNotFound(_)));
}

/// link_entity_observation creates the join row and is idempotent.
#[tokio::test]
async fn link_entity_observation_records_and_is_idempotent() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Alice").await;
    let obs = observation("user");
    let oid = sb.store.record_observation(obs).await.unwrap();

    sb.store.link_entity_observation(&e.id, &oid).await.unwrap();
    sb.store.link_entity_observation(&e.id, &oid).await.unwrap();

    let conn = sb.raw();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entity_observations WHERE entity_id = ?1 AND observation_id = ?2",
            [e.id.to_string(), oid.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "INSERT OR IGNORE keeps the link a set");
}

/// Observations survive a close/reopen cycle.
#[tokio::test]
async fn observations_survive_restart() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let obs = observation("clock");
    let id = sb.store.record_observation(obs.clone()).await.unwrap();

    let store = sb.reopen().await;
    let got = store.get_observation(&id).await.unwrap().unwrap();
    assert_eq!(got.kind, "clock");
    assert_eq!(got.payload, obs.payload);
}
