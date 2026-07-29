#[cfg(test)]
mod nvidia_e2e {
    use brain_core::traits::ReasoningService;
    use std::sync::Arc;

    #[tokio::test]
    async fn reason_with_nvidia_llm() {
        // Set up NVIDIA API connection
        std::env::set_var("AI_OS_LLM_API_BASE", "https://integrate.api.nvidia.com/v1");
        std::env::set_var("AI_OS_LLM_MODEL", "meta/llama-3.1-8b-instruct");
        std::env::set_var(
            "AI_OS_LLM_API_KEY",
            "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",
        );

        let coordinator = Arc::new(intelligence_coordinator::DefaultCoordinator::default());
        let service = intelligence_coordinator::IntelligenceReasoningService::new(coordinator);

        // Run the reasoning pipeline end-to-end
        let result = service.reason("Open Firefox and go to example.com").await;

        match &result {
            Ok(r) => {
                println!("\n=== REASONING RESULT ===");
                println!("summary:        {}", r.summary);
                println!("intent:         {}", r.intent);
                println!("confidence:     {}", r.confidence.0);
                println!("goal:           {}", r.suggested_goal);
                println!("priority:       {:?}", r.suggested_priority);
                println!("capabilities:   {:?}", r.required_capabilities);
                println!("entity count:   {}", r.entities.len());
                for e in &r.entities {
                    println!("  entity: {} ({}) = {}", e.name, e.entity_type, e.value);
                }
                println!("observations:   {:?}", r.observations);
                println!("explanation:    {}", r.explanation);

                assert!(!r.summary.is_empty(), "summary should not be empty");
                assert!(!r.intent.is_empty(), "intent should not be empty");
                assert!(
                    !r.required_capabilities.is_empty(),
                    "should have required capabilities"
                );
            }
            Err(e) => {
                panic!("reasoning pipeline failed: {e}");
            }
        }
    }
}
