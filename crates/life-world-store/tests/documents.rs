//! Documents: blob persistence, CRUD, entity links, extracted text, and FTS.

mod common;

use common::{Sandbox, add_entity, ai_ctx, create_doc};

use life_world_store::{
    DocumentEntityLink, DocumentId, UpdateDocument, WorldStore, WorldStoreError,
};
use memory_core::wm::{EntityId, EntityLifecycle};

fn doc_title(d: &life_world_store::Document) -> String {
    d.title.clone()
}

/// create_document writes the blob and metadata and returns a version-1 doc.
#[tokio::test]
async fn create_document_writes_blob_and_metadata() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let content = b"hello document world".to_vec();
    let doc = sb
        .store
        .create_document(create_doc("memo", &content, vec![]))
        .await
        .unwrap();

    assert_eq!(doc.title, "memo");
    assert_eq!(doc.mime_type.as_deref(), Some("text/plain"));
    assert_eq!(doc.size_bytes, Some(content.len() as i64));
    assert_eq!(doc.lifecycle, EntityLifecycle::Active);
    assert_eq!(doc.version, 1);
    assert_eq!(doc.created_by, "ai");

    // The blob lands in the documents dir under `{id}.bin`.
    let blob_path = sb.documents_dir.join(format!("{}.bin", doc.id));
    assert_eq!(std::fs::read(&blob_path).unwrap(), content);
    assert_eq!(
        doc.storage_path.as_deref(),
        Some(format!("{}.bin", doc.id).as_str())
    );
}

/// The same document entity id is shared by the `document` entity row and the
/// documents row, so the property graph can find it.
#[tokio::test]
async fn document_entity_row_uses_same_id() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(create_doc("memo", b"data", vec![]))
        .await
        .unwrap();

    let entity = sb
        .store
        .get_entity(&EntityId::from_uuid(*doc.id.as_uuid()))
        .await
        .unwrap();
    let entity = entity.expect("a `document` entity row must exist");
    assert_eq!(entity.entity_type, "document");
    assert_eq!(entity.name, "memo");
}

/// get_document round-trips and returns None for a missing id.
#[tokio::test]
async fn get_document_roundtrip_and_missing() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(create_doc("memo", b"data", vec![]))
        .await
        .unwrap();
    let got = sb.store.get_document(&doc.id).await.unwrap().unwrap();
    assert_eq!(got.id, doc.id);
    assert_eq!(doc_title(&got), "memo");

    let ghost = DocumentId::new();
    assert!(sb.store.get_document(&ghost).await.unwrap().is_none());
}

/// update_document changes the title/tags/content and bumps the version.
#[tokio::test]
async fn update_document_changes_fields_and_rewrites_blob() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(create_doc("memo", b"v1", vec![]))
        .await
        .unwrap();
    let old_checksum = doc.checksum.clone();

    let new_content = b"version two content, longer".to_vec();
    let updated = sb
        .store
        .update_document(UpdateDocument {
            id: doc.id,
            expected_version: 1,
            title: Some("memo v2".into()),
            tags: Some(vec!["new-tag".into()]),
            content: Some(new_content.clone()),
            extracted_text: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap();

    assert_eq!(updated.version, 2);
    assert_eq!(updated.title, "memo v2");
    assert_eq!(updated.tags, vec!["new-tag".to_string()]);
    assert_ne!(updated.checksum, old_checksum);

    let blob_path = sb.documents_dir.join(format!("{}.bin", doc.id));
    assert_eq!(std::fs::read(&blob_path).unwrap(), new_content);
}

/// update_document enforces optimistic concurrency.
#[tokio::test]
async fn update_document_version_conflict() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(create_doc("memo", b"v1", vec![]))
        .await
        .unwrap();

    let err = sb
        .store
        .update_document(UpdateDocument {
            id: doc.id,
            expected_version: 5,
            title: Some("stale".into()),
            tags: None,
            content: None,
            extracted_text: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, WorldStoreError::VersionConflict { .. }),
        "got {err:?}"
    );
    let now = sb.store.get_document(&doc.id).await.unwrap().unwrap();
    assert_eq!(now.version, 1);
}

/// Updating or archiving a missing document returns DocumentNotFound.
#[tokio::test]
async fn missing_document_errors() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let ghost = DocumentId::new();
    let upd = sb
        .store
        .update_document(UpdateDocument {
            id: ghost,
            expected_version: 1,
            title: None,
            tags: None,
            content: None,
            extracted_text: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap_err();
    assert!(matches!(upd, WorldStoreError::DocumentNotFound(_)));

    let arc = sb
        .store
        .archive_document(&ghost, 1, &ai_ctx())
        .await
        .unwrap_err();
    assert!(matches!(arc, WorldStoreError::DocumentNotFound(_)));
}

/// archive_document flips the document lifecycle to Archived.
#[tokio::test]
async fn archive_document_archives() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(create_doc("memo", b"data", vec![]))
        .await
        .unwrap();

    sb.store
        .archive_document(&doc.id, 1, &ai_ctx())
        .await
        .unwrap();
    let got = sb.store.get_document(&doc.id).await.unwrap().unwrap();
    assert_eq!(got.lifecycle, EntityLifecycle::Archived);

    let err = sb
        .store
        .archive_document(&doc.id, 1, &ai_ctx())
        .await
        .unwrap_err();
    assert!(matches!(err, WorldStoreError::VersionConflict { .. }));
}

/// Entity links supplied at create time are stored in document_entities.
#[tokio::test]
async fn create_document_stores_entity_links() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    let req = create_doc(
        "memo",
        b"data",
        vec![DocumentEntityLink {
            entity_id: a.id,
            role: "about".into(),
            confidence: 0.9,
        }],
    );
    let doc = sb.store.create_document(req).await.unwrap();

    let conn = sb.raw();
    let rows: Vec<(String, String)> = conn
        .prepare("SELECT entity_id, role FROM document_entities WHERE document_id = ?1")
        .unwrap()
        .query_map([doc.id.to_string()], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(rows, vec![(a.id.to_string(), "about".to_string())]);
}

/// A document cannot link a non-existent entity (referential integrity).
#[tokio::test]
async fn create_document_rejects_missing_link_entity() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let ghost = EntityId::new();
    let req = create_doc(
        "memo",
        b"data",
        vec![DocumentEntityLink {
            entity_id: ghost,
            role: "about".into(),
            confidence: 0.9,
        }],
    );
    let res = sb.store.create_document(req).await;
    assert!(res.is_err(), "link to a missing entity must be rejected");
}

/// Extracted text is searchable through the FTS index.
#[tokio::test]
async fn extracted_text_is_searchable() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut req = create_doc("notes", b"ignored bytes", vec![]);
    req.extracted_text = Some("xylophone taxonomy revisited".into());
    let doc = sb.store.create_document(req).await.unwrap();

    let results = sb
        .store
        .search_documents(&life_world_store::SearchQuery {
            text: Some("xylophone".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, doc.id);
}

/// An empty entity-properties map is stored cleanly.
#[tokio::test]
async fn create_document_empty_entity_properties_ok() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let req = create_doc("memo", b"data", vec![]);
    let doc = sb.store.create_document(req).await.unwrap();
    let entity = sb
        .store
        .get_entity(&EntityId::from_uuid(*doc.id.as_uuid()))
        .await
        .unwrap()
        .unwrap();
    assert!(entity.properties.is_empty());
}

/// A stale-version update fails *before* the blob is rewritten, so no orphan
/// blob with the new content is left on disk.
#[tokio::test]
async fn update_document_version_conflict_does_not_orphan_blob() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(create_doc("memo", b"v1", vec![]))
        .await
        .unwrap();

    let err = sb
        .store
        .update_document(UpdateDocument {
            id: doc.id,
            expected_version: 5,
            title: Some("stale".into()),
            tags: None,
            content: Some(b"should never be written".to_vec()),
            extracted_text: None,
            ctx: ai_ctx(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, WorldStoreError::VersionConflict { .. }),
        "got {err:?}"
    );

    let blob_path = sb.documents_dir.join(format!("{}.bin", doc.id));
    assert_eq!(
        std::fs::read(&blob_path).unwrap(),
        b"v1",
        "blob must still hold the original content after a rejected update"
    );
}

/// Hard-deleting a document entity purges the documents/document_content rows
/// and the managed blob (blob is retained only until hard delete, ADR-0008 D4).
#[tokio::test]
async fn hard_delete_document_removes_rows_and_blob() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(create_doc("memo", b"payload bytes", vec![]))
        .await
        .unwrap();
    let entity_id = EntityId::from_uuid(*doc.id.as_uuid());
    let blob_path = sb.documents_dir.join(format!("{}.bin", doc.id));
    assert!(blob_path.exists());

    sb.store
        .delete_entity(&entity_id, 1, true, &ai_ctx())
        .await
        .unwrap();

    assert!(!blob_path.exists(), "blob must be removed on hard delete");
    assert!(sb.store.get_document(&doc.id).await.unwrap().is_none());
    assert!(sb.store.get_entity(&entity_id).await.unwrap().is_none());

    let conn = sb.raw();
    let documents: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM documents WHERE id = ?1",
            [doc.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let content_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM document_content WHERE document_id = ?1",
            [doc.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(documents, 0, "documents row must be purged");
    assert_eq!(content_rows, 0, "document_content row must be purged");
}

/// Soft-deleting a document keeps the blob and rows intact (retained until hard
/// delete).
#[tokio::test]
async fn soft_delete_document_keeps_blob_and_rows() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(create_doc("memo", b"kept", vec![]))
        .await
        .unwrap();

    sb.store
        .archive_document(&doc.id, 1, &ai_ctx())
        .await
        .unwrap();

    let blob_path = sb.documents_dir.join(format!("{}.bin", doc.id));
    assert!(blob_path.exists(), "blob retained while archived");
    let got = sb.store.get_document(&doc.id).await.unwrap().unwrap();
    assert_eq!(got.lifecycle, EntityLifecycle::Archived);
}

/// archive_document records the writer's provenance on the row.
#[tokio::test]
async fn archive_document_records_provenance() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let doc = sb
        .store
        .create_document(create_doc("memo", b"data", vec![]))
        .await
        .unwrap();

    sb.store
        .archive_document(
            &doc.id,
            1,
            &life_world_store::WriteContext {
                actor: life_world_store::Actor::Ai,
                provenance: Some("obs-archive-7".into()),
            },
        )
        .await
        .unwrap();

    let (_, provenance) = common::document_provenance(&sb.raw(), &doc.id);
    assert_eq!(provenance.as_deref(), Some("obs-archive-7"));
}
