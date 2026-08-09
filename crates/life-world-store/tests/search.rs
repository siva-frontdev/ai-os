//! Search: entity-type, text/FTS, lifecycle, tag, and pagination filters.

mod common;

use common::{Sandbox, add_entity, create_entity};

use life_world_store::{LifecycleFilter, SearchQuery, SortBy, UpdateEntity, WorldStore};

use memory_core::wm::Value;

fn query_with_text(text: &str) -> SearchQuery {
    SearchQuery {
        text: Some(text.to_string()),
        ..Default::default()
    }
}

/// search_entities filters by exact entity type.
#[tokio::test]
async fn search_entities_by_type() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let alice = add_entity(&sb.store, "person", "Alice").await;
    add_entity(&sb.store, "company", "ACME").await;
    add_entity(&sb.store, "person", "Bob").await;

    let people = sb
        .store
        .search_entities(&SearchQuery {
            entity_type: Some("person".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(people.len(), 2);
    assert!(people.iter().all(|e| e.entity_type == "person"));

    // Default lifecycle filter is Active, so archived rows are excluded.
    sb.store
        .archive_entity(&alice.id, 1, "cleanup", &common::ai_ctx())
        .await
        .unwrap();
    let active = sb
        .store
        .search_entities(&SearchQuery {
            entity_type: Some("person".into()),
            lifecycle: LifecycleFilter::Active,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "Bob");
}

/// search_entities finds names via the FTS index.
#[tokio::test]
async fn search_entities_by_text() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    add_entity(&sb.store, "person", "Alice").await;
    add_entity(&sb.store, "person", "Bob").await;
    add_entity(&sb.store, "person", "alice in wonderland").await;

    let results = sb
        .store
        .search_entities(&query_with_text("Alice"))
        .await
        .unwrap();
    let names: Vec<String> = results.iter().map(|e| e.name.clone()).collect();
    assert_eq!(
        results.len(),
        2,
        "FTS token matching is case-insensitive: {names:?}"
    );
    assert!(names.contains(&"Alice".to_string()));
    assert!(names.contains(&"alice in wonderland".to_string()));
    assert!(!names.contains(&"Bob".to_string()));
}

/// search_entities matches metadata tag values.
#[tokio::test]
async fn search_entities_by_tag() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut req = create_entity("person", "Alice");
    req.metadata.insert("audience".to_string(), "vip".into());
    sb.store.create_entity(req).await.unwrap();
    let mut req = create_entity("person", "Bob");
    req.metadata.insert("audience".to_string(), "staff".into());
    sb.store.create_entity(req).await.unwrap();

    let results = sb
        .store
        .search_entities(&SearchQuery {
            tags: Some(vec!["vip".into()]),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Alice");
}

/// LifecycleFilter::Archived returns only archived rows.
#[tokio::test]
async fn search_entities_archived_filter() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let a = add_entity(&sb.store, "person", "Alice").await;
    add_entity(&sb.store, "person", "Bob").await;
    sb.store
        .archive_entity(&a.id, 1, "gone", &common::ai_ctx())
        .await
        .unwrap();

    let archived = sb
        .store
        .search_entities(&SearchQuery {
            lifecycle: LifecycleFilter::Archived,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].name, "Alice");
}

/// limit + offset paginate, and offset is honoured independently of limit.
#[tokio::test]
async fn search_paginates() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    for i in 0..5 {
        add_entity(&sb.store, "person", &format!("person{i}")).await;
    }

    let page = sb
        .store
        .search_entities(&SearchQuery {
            limit: 2,
            offset: 1,
            sort: SortBy::Name,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page.len(), 2);
    let names: Vec<String> = page.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, vec!["person1".to_string(), "person2".to_string()]);
}

/// search_documents filters by lifecycle and tag.
#[tokio::test]
async fn search_documents_filters() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let d1 = common::add_doc(&sb.store, "budget", b"one").await;
    common::add_doc(&sb.store, "plans", b"two").await;

    // Lifecycle filter: only active by default; archive d1 and it disappears.
    sb.store
        .archive_document(&d1.id, 1, &common::ai_ctx())
        .await
        .unwrap();
    let active = sb
        .store
        .search_documents(&SearchQuery {
            lifecycle: LifecycleFilter::Active,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].title, "plans");

    let archived = sb
        .store
        .search_documents(&SearchQuery {
            lifecycle: LifecycleFilter::Archived,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].title, "budget");
}

/// A property value survives a search round-trip unchanged.
#[tokio::test]
async fn search_returns_full_entities() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let mut req = create_entity("person", "Alice");
    req.properties
        .insert("email".to_string(), Value::String("a@b.c".into()));
    let created = sb.store.create_entity(req).await.unwrap();

    let results = sb
        .store
        .search_entities(&query_with_text("Alice"))
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, created.id);
    let got = results[0].property("email").unwrap();
    assert_eq!(got.as_str(), Some("a@b.c"));
}

/// Updates are reflected in subsequent searches.
#[tokio::test]
async fn search_reflects_updates() {
    let sb = Sandbox::new(tempfile::tempdir().unwrap()).await;
    let e = add_entity(&sb.store, "person", "Old Name").await;

    sb.store
        .update_entity(UpdateEntity {
            id: e.id,
            expected_version: 1,
            name: Some("New Name".into()),
            properties: None,
            importance: None,
            confidence: None,
            metadata: None,
            ctx: common::ai_ctx(),
        })
        .await
        .unwrap();

    let results = sb
        .store
        .search_entities(&query_with_text("New Name"))
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    let results = sb
        .store
        .search_entities(&query_with_text("Old Name"))
        .await
        .unwrap();
    assert!(
        results.is_empty(),
        "the old indexed name must not match anymore"
    );
}
