use memory_context::{
    ContextManager, ContextProvider, DefaultContextManager, StaticContextProvider,
    TreeContextManager,
};

fn make_tree() -> TreeContextManager {
    TreeContextManager::new()
}

#[tokio::test]
async fn test_full_lifecycle() {
    let mgr = make_tree();
    let root = mgr.create_context(None).await.unwrap();
    mgr.set_value(&root, "platform", b"ai-os".to_vec())
        .await
        .unwrap();

    let child = mgr.create_context(Some(root)).await.unwrap();
    mgr.set_value(&child, "task", b"reasoning".to_vec())
        .await
        .unwrap();

    assert_eq!(
        mgr.get_value(&child, "platform").await.unwrap(),
        Some(b"ai-os".to_vec())
    );
    assert_eq!(
        mgr.get_value(&child, "task").await.unwrap(),
        Some(b"reasoning".to_vec())
    );

    mgr.delete_value(&child, "task").await.unwrap();
    assert!(mgr.get_value(&child, "task").await.unwrap().is_none());

    mgr.destroy_context(&root).await.unwrap();
    assert!(mgr.get_value(&child, "task").await.is_err());
}

#[tokio::test]
async fn test_context_inheritance() {
    let mgr = make_tree();
    let grandparent = mgr.create_context(None).await.unwrap();
    mgr.set_value(&grandparent, "level", b"0".to_vec())
        .await
        .unwrap();

    let parent = mgr.create_context(Some(grandparent)).await.unwrap();
    mgr.set_value(&parent, "level", b"1".to_vec())
        .await
        .unwrap();

    let child = mgr.create_context(Some(parent)).await.unwrap();
    mgr.set_value(&child, "level", b"2".to_vec()).await.unwrap();

    assert_eq!(
        mgr.get_value(&child, "level").await.unwrap(),
        Some(b"2".to_vec())
    );
    assert_eq!(
        mgr.get_value(&parent, "level").await.unwrap(),
        Some(b"1".to_vec())
    );
    assert_eq!(
        mgr.get_value(&grandparent, "level").await.unwrap(),
        Some(b"0".to_vec())
    );
}

#[tokio::test]
async fn test_concurrent_context_operations() {
    let mgr = std::sync::Arc::new(make_tree());
    let root = mgr.create_context(None).await.unwrap();

    let mut handles = Vec::new();
    for i in 0..10usize {
        let mgr = mgr.clone();
        handles.push(tokio::spawn(async move {
            let child = mgr.create_context(Some(root)).await.unwrap();
            mgr.set_value(&child, "idx", format!("{}", i).into_bytes().to_vec())
                .await
                .unwrap();
            child
        }));
    }

    let mut child_ids = Vec::new();
    for h in handles {
        child_ids.push(h.await.unwrap());
    }

    let all = mgr.active_contexts().await.unwrap();
    assert_eq!(all.len(), 11); // root + 10 children
}

#[tokio::test]
async fn test_provider_integration() {
    let ctx_id = TreeContextManager::new()
        .create_context(None)
        .await
        .unwrap();
    let mut provider = StaticContextProvider::new();
    provider.set("env_key", b"env_val".to_vec());

    let keys = provider.available_keys().await.unwrap();
    assert_eq!(keys, vec!["env_key"]);

    let vals = provider.provide_context(&["env_key".into()]).await.unwrap();
    assert_eq!(vals.get("env_key").unwrap(), b"env_val");
    drop(ctx_id);
}

#[tokio::test]
async fn test_hierarchical_override() {
    let mgr = make_tree();
    let root = mgr.create_context(None).await.unwrap();
    mgr.set_value(&root, "theme", b"dark".to_vec())
        .await
        .unwrap();

    let child = mgr.create_context(Some(root)).await.unwrap();
    mgr.set_value(&child, "theme", b"light".to_vec())
        .await
        .unwrap();

    assert_eq!(
        mgr.get_value(&child, "theme").await.unwrap(),
        Some(b"light".to_vec())
    );
    assert_eq!(
        mgr.get_value(&root, "theme").await.unwrap(),
        Some(b"dark".to_vec())
    );
}
