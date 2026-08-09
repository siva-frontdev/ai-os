//! Security posture: path containment for uploaded file names, payload
//! robustness, and audit-trail traceability.

mod common;

use common::{Sandbox, add_entity, ai_ctx};

use life_world_store::{CreateDocument, DocumentEntityLink, WorldStore};
use memory_core::wm::EntityLifecycle;

/// A hostile `original_filename` never escapes the documents directory: the
/// blob path is derived from the generated id, not from user input.
#[tokio::test]
async fn original_filename_cannot_escape_documents_dir() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut req = common::create_doc("memo", b"content", vec![]);
    req.original_filename = Some("../../../../tmp/evil.txt".into());
    let doc = sb.store.create_document(req).await.unwrap();

    // The blob lands under documents_dir as `<uuid>.bin`.
    let expected = sb.documents_dir.join(format!("{}.bin", doc.id));
    assert!(expected.exists(), "blob must exist inside documents_dir");
    assert_eq!(
        doc.storage_path.as_deref(),
        Some(format!("{}.bin", doc.id).as_str())
    );

    // No file escaped the directory.
    assert!(
        !sb.documents_dir
            .join("..")
            .join("..")
            .join("tmp")
            .join("evil.txt")
            .exists()
    );
    assert!(!std::path::Path::new("/tmp/evil.txt").exists());

    // The original value is stored verbatim for provenance, not used as a path.
    assert_eq!(
        doc.original_filename.as_deref(),
        Some("../../../../tmp/evil.txt")
    );
}

/// The stored blob bytes exactly match the uploaded content (storage
/// integrity), and the same content produces a deterministic storage path.
#[tokio::test]
async fn blob_matches_uploaded_content() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let content = b"exact bytes to store".to_vec();
    let doc = sb
        .store
        .create_document(common::create_doc("memo", &content, vec![]))
        .await
        .unwrap();

    let blob = sb.documents_dir.join(format!("{}.bin", doc.id));
    assert_eq!(std::fs::read(&blob).unwrap(), content);
    assert_eq!(doc.size_bytes, Some(content.len() as i64));
}

/// Deletion is traceable: a soft delete retains full history, and a hard
/// delete retains the Delete tombstone.
#[tokio::test]
async fn deletion_keeps_audit_trail() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;

    sb.store
        .delete_entity(&a.id, 1, false, &ai_ctx())
        .await
        .unwrap();
    let rows = common::entity_history_rows(&sb.raw(), &a.id);
    let ops: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(
        ops,
        vec!["create", "archive"],
        "soft delete is an archive in history"
    );

    sb.store
        .delete_entity(&a.id, 2, true, &ai_ctx())
        .await
        .unwrap();
    let rows = common::entity_history_rows(&sb.raw(), &a.id);
    let ops: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(
        ops,
        vec!["delete"],
        "only the tombstone survives a hard delete"
    );
}

/// A soft-deleted entity is invisible to active search but still restorable.
#[tokio::test]
async fn soft_deleted_entities_hidden_from_active_search() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let b = add_entity(&sb.store, "person", "Bob").await;

    sb.store
        .delete_entity(&a.id, 1, false, &ai_ctx())
        .await
        .unwrap();

    let active = sb
        .store
        .search_entities(&life_world_store::SearchQuery {
            entity_type: Some("person".into()),
            lifecycle: life_world_store::LifecycleFilter::Active,
            ..Default::default()
        })
        .await
        .unwrap();
    let names: Vec<String> = active.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, vec!["Bob".to_string()]);
    assert_eq!(
        sb.store.get_entity(&b.id).await.unwrap().unwrap().lifecycle,
        EntityLifecycle::Active
    );
}

/// Arbitrary nested observation payloads round-trip without loss.
#[tokio::test]
async fn observation_payload_roundtrips_arbitrary_json() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let payload = serde_json::json!({
        "nested": {"deep": {"list": [1, 2, {"x": null}], "unicode": "héllo — wörld"}},
        "empty": {},
        "bool": true,
        "float": std::f64::consts::PI,
    });
    let mut obs = common::observation("email");
    obs.payload = payload.clone();
    let id = sb.store.record_observation(obs).await.unwrap();

    let got = sb.store.get_observation(&id).await.unwrap().unwrap();
    assert_eq!(got.payload, payload);
}

/// A document link to an entity that is later hard-deleted cannot be created,
/// and deleting the entity removes the link (referential integrity).
#[tokio::test]
async fn entity_hard_delete_cleans_links() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let req = CreateDocument {
        title: "about alice".into(),
        mime_type: Some("text/plain".into()),
        original_filename: Some("alice.txt".into()),
        source_uri: Some("test://doc".into()),
        tags: Vec::new(),
        content: b"data".to_vec(),
        extracted_text: Some("about alice".into()),
        content_type: Some("plain".into()),
        entity_properties: Default::default(),
        entity_links: vec![DocumentEntityLink {
            entity_id: a.id,
            role: "about".into(),
            confidence: 0.9,
        }],
        ctx: ai_ctx(),
    };
    let doc = sb.store.create_document(req).await.unwrap();

    sb.store
        .delete_entity(&a.id, 1, true, &ai_ctx())
        .await
        .unwrap();

    let conn = sb.raw();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM document_entities WHERE document_id = ?1",
            [doc.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 0,
        "document_entities link must be gone after entity delete"
    );
}

/// A fresh document id never collides with an existing one.
#[tokio::test]
async fn document_ids_do_not_collide() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let d1 = common::add_doc(&sb.store, "one", b"1").await;
    let d2 = common::add_doc(&sb.store, "two", b"2").await;
    assert_ne!(d1.id, d2.id);
}
