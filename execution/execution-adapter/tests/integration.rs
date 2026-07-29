use execution_adapter::DefaultAdapterGenerator;
use execution_core::*;
use std::collections::HashMap;

#[tokio::test]
async fn test_adapter_generates_valid_templates() {
    let generator = DefaultAdapterGenerator::new();
    let request = CapabilitySynthesisRequest {
        capability_id: "deploy.app".into(),
        capability_name: "Deploy Application".into(),
        description: "Deploy an application to the cloud".into(),
        provider: ProviderMetadata::new("railway", "cli", "https://railway.app"),
        input_schema: HashMap::new(),
        output_schema: HashMap::new(),
        preferred_adapter_type: Some(AdapterType::CliWrapper),
    };

    let adapter = generator.generate(&request).await.unwrap();
    assert_eq!(adapter.adapter_type, AdapterType::CliWrapper);
    assert!(adapter.template.contains("railway"));
}

#[tokio::test]
async fn test_adapter_generates_rest_client() {
    let generator = DefaultAdapterGenerator::new();
    let request = CapabilitySynthesisRequest {
        capability_id: "api.query".into(),
        capability_name: "Query API".into(),
        description: "Query a REST API".into(),
        provider: ProviderMetadata::new("myapi", "api", "https://api.myapi.com"),
        input_schema: HashMap::new(),
        output_schema: HashMap::new(),
        preferred_adapter_type: Some(AdapterType::RestClient),
    };

    let adapter = generator.generate(&request).await.unwrap();
    assert_eq!(adapter.adapter_type, AdapterType::RestClient);
    assert!(adapter.template.contains("myapi"));
}

#[tokio::test]
async fn test_can_synthesize_compatible() {
    let generator = DefaultAdapterGenerator::new();
    let mut trust = TrustScore::new(CapabilityOrigin::Generated);
    trust.score = 0.5;
    let cap = DiscoveredCapability {
        id: "test".into(),
        name: "test".into(),
        description: "test".into(),
        origin: CapabilityOrigin::Generated,
        provider: ProviderMetadata::new("t", "cli", "/t"),
        trust,
        input_schema: HashMap::new(),
        output_schema: HashMap::new(),
        required_permissions: Vec::new(),
        supported_parameters: HashMap::new(),
        adapter_template: None,
        compatible: true,
    };
    assert!(generator.can_synthesize(&cap).await);
}
