//! Backup and restore: manifest, snapshot integrity, round-trip, and
//! tamper detection.

mod common;

use std::path::PathBuf;

use common::{Sandbox, add_entity, ai_ctx};

use life_world_store::{RestoreReport, WorldStore, WorldStoreError};

/// backup writes a snapshot DB and a manifest into the destination directory.
#[tokio::test]
async fn backup_writes_manifest_and_snapshot() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    add_entity(&sb.store, "person", "Alice").await;
    let dest = tempfile::tempdir().unwrap();

    let manifest = sb.store.backup(dest.path()).await.unwrap();
    assert!(manifest.created_at > 0);
    assert!(!manifest.db_sha256.is_empty());
    assert!(manifest.db_file.ends_with(".db"));

    let snapshot = dest.path().join(&manifest.db_file);
    assert!(snapshot.exists(), "snapshot DB must exist");
    assert!(
        dest.path().join("backup.json").exists(),
        "manifest must exist"
    );
}

/// restore swaps the snapshot back in: data deleted after the backup returns.
#[tokio::test]
async fn restore_roundtrip_recovers_deleted_data() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let alice = add_entity(&sb.store, "person", "Alice").await;
    let dest = tempfile::tempdir().unwrap();
    sb.store.backup(dest.path()).await.unwrap();

    // Mutate the live store after the backup.
    sb.store
        .delete_entity(&alice.id, 1, true, &ai_ctx())
        .await
        .unwrap();
    assert!(sb.store.get_entity(&alice.id).await.unwrap().is_none());

    let report: RestoreReport = sb.store.restore(dest.path()).await.unwrap();
    assert_eq!(report.integrity, "ok");
    assert_eq!(report.entities, 1);
    assert!(report.documents_match);

    let got = sb.store.get_entity(&alice.id).await.unwrap().unwrap();
    assert_eq!(got.name, "Alice");
}

/// A snapshot whose bytes no longer match the manifest checksum is rejected.
#[tokio::test]
async fn restore_rejects_tampered_snapshot() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    add_entity(&sb.store, "person", "Alice").await;
    let dest = tempfile::tempdir().unwrap();
    let manifest = sb.store.backup(dest.path()).await.unwrap();

    // Corrupt the snapshot on disk.
    let snapshot = dest.path().join(&manifest.db_file);
    std::fs::write(&snapshot, b"garbage that is not a sqlite file").unwrap();

    let err = sb.store.restore(dest.path()).await.unwrap_err();
    assert!(matches!(err, WorldStoreError::Migration(_)), "got {err:?}");
}

/// Restore from a directory without a manifest is an I/O error.
#[tokio::test]
async fn restore_missing_manifest_errors() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let empty = tempfile::tempdir().unwrap();
    let err = sb.store.restore(empty.path()).await.unwrap_err();
    assert!(matches!(err, WorldStoreError::Io(_)), "got {err:?}");
}

/// Rotation keeps at most `backup_count` snapshots in the destination.
#[tokio::test]
async fn backup_rotates_snapshots() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    add_entity(&sb.store, "person", "Alice").await;
    let dest = tempfile::tempdir().unwrap();

    for _ in 0..5 {
        sb.store.backup(dest.path()).await.unwrap();
    }

    let db_snaps: Vec<PathBuf> = std::fs::read_dir(dest.path())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "db").unwrap_or(false))
        .collect();
    assert!(
        db_snaps.len() <= 3,
        "kept {} snapshots (backup_count=3)",
        db_snaps.len()
    );
}

/// Restore survives a store restart: the restored file is the new durable
/// database.
#[tokio::test]
async fn restored_state_survives_restart() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let alice = add_entity(&sb.store, "person", "Alice").await;
    let dest = tempfile::tempdir().unwrap();
    sb.store.backup(dest.path()).await.unwrap();

    sb.store
        .delete_entity(&alice.id, 1, true, &ai_ctx())
        .await
        .unwrap();
    sb.store.restore(dest.path()).await.unwrap();

    let store = sb.reopen().await;
    let got = store.get_entity(&alice.id).await.unwrap().unwrap();
    assert_eq!(got.name, "Alice");
}
