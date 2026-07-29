use execution_cache::{DefaultLearningEngine, InMemoryCapabilityCache};
use execution_core::*;
use memory_core::Timestamp;
use std::collections::HashMap;

#[tokio::test]
async fn test_cache_and_learning_integration() {
    let cache = InMemoryCapabilityCache::new();
    let engine = DefaultLearningEngine::new();

    let entry = CapabilityCacheEntry {
        capability_id: "deploy.app".into(),
        origin: CapabilityOrigin::DiscoveredLocal,
        provider_metadata: ProviderMetadata::new("railway", "cli", "/usr/bin/railway"),
        trust: TrustScore::new(CapabilityOrigin::DiscoveredLocal),
        supported_parameters: HashMap::new(),
        required_permissions: vec!["subprocess".into()],
        adapter: None,
        created_at: Timestamp::now(),
        last_accessed: Timestamp::now(),
        access_count: 0,
    };
    cache.store(entry).await.unwrap();

    let cached = cache.lookup("deploy.app").await.unwrap();
    assert!(cached.is_some());

    engine
        .record_execution("deploy.app", true, 5000, None)
        .await
        .unwrap();
    let learned = engine.get_learned("deploy.app").await.unwrap();
    assert!(learned.is_some());
    assert_eq!(learned.unwrap().execution_count, 1);
}

#[tokio::test]
async fn test_cache_hit_rate_multiple_ops() {
    let cache = InMemoryCapabilityCache::new();
    let entry = CapabilityCacheEntry {
        capability_id: "build.project".into(),
        origin: CapabilityOrigin::Native,
        provider_metadata: ProviderMetadata::new("cargo", "cli", "/usr/bin/cargo"),
        trust: TrustScore::verified(CapabilityOrigin::Native, 1.0),
        supported_parameters: HashMap::new(),
        required_permissions: Vec::new(),
        adapter: None,
        created_at: Timestamp::now(),
        last_accessed: Timestamp::now(),
        access_count: 0,
    };
    cache.store(entry).await.unwrap();

    let _ = cache.lookup("build.project").await;
    let _ = cache.lookup("nonexistent").await;

    let rate = cache.hit_rate().await;
    assert!((rate - 0.5).abs() < 0.01);
}
