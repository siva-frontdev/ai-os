use execution_core::*;
use execution_discovery::{DefaultExternalDiscoverer, DefaultLocalDiscoverer};

#[tokio::test]
async fn test_local_discovery_integration() {
    let discoverer = DefaultLocalDiscoverer::new();
    let query = DiscoveryQuery {
        capability_id: "build.project".into(),
        capability_name: "Build Project".into(),
        keywords: vec!["cargo".into(), "compile".into()],
        categories: Vec::new(),
    };
    let results = discoverer.discover_local(&query).await.unwrap();
    assert!(!results.is_empty());
    for cap in &results {
        assert_eq!(cap.origin, CapabilityOrigin::DiscoveredLocal);
        assert!(cap.compatible);
    }
}

#[tokio::test]
async fn test_external_discovery_integration() {
    let discoverer = DefaultExternalDiscoverer::new();
    let query = DiscoveryQuery {
        capability_id: "deploy.container".into(),
        capability_name: "Deploy Container".into(),
        keywords: vec!["docker".into(), "container".into()],
        categories: Vec::new(),
    };
    let results = discoverer.discover_external(&query).await.unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|c| c.provider.name == "docker"));
}

#[tokio::test]
async fn test_installed_clis_non_empty() {
    let discoverer = DefaultLocalDiscoverer::new();
    let clis = discoverer.discover_installed_clis().await.unwrap();
    assert!(!clis.is_empty());
}
