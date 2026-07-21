#[tokio::test]
async fn test_core_compile() {
    // Ensure the core crate compiles and basic types are usable.
    use intelligence_core::*;
    let _id = ProviderId::new();
    let _model = ModelId::new();
    // Just construct a dummy request.
    let request = ModelRequest {
        request_id: RequestId(uuid::Uuid::now_v7()),
        capability: CapabilityKind::Chat,
        model_id: None,
        input: ModelInput::Text(String::new()),
        parameters: std::collections::HashMap::new(),
        temperature: None,
        top_p: None,
        max_tokens: None,
        stream: false,
    };
    // No actual call, just ensure struct can be built.
    assert_eq!(request.stream, false);
}
