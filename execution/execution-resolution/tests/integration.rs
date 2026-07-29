use execution_core::*;
use execution_registry::InMemoryToolRegistry;
use execution_resolution::DefaultResolutionPipeline;
use execution_sandbox::DefaultSandboxEnforcer;
use std::collections::HashMap;
use std::sync::Arc;

fn make_request(capability_id: &str) -> ExecutionRequest {
    ExecutionRequest {
        requirement_id: format!("req-{}", capability_id),
        capability_id: capability_id.into(),
        inputs: HashMap::new(),
        context: ExecutionContext {
            session: ExecutionSession {
                session_id: "int-session".into(),
                user_id: "int-user".into(),
                roles: vec!["admin".into()],
                permissions: ExecutionPermissions::default(),
            },
            trace_id: "int-trace".into(),
            span_id: "int-span".into(),
            originating_goal: None,
            originating_plan: None,
        },
        budget: ExecutionBudget::default(),
        priority: ExecutionPriority::default(),
        retry_policy: RetryPolicy::default(),
    }
}

#[tokio::test]
async fn test_pipeline_exhausts_on_unknown() {
    let registry = Arc::new(InMemoryToolRegistry::new());
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let pipeline = DefaultResolutionPipeline::new(registry, sandbox, None);

    let request = make_request("completely.unknown.capability");
    let result = pipeline.resolve(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_pipeline_bypass_engages_and_disengages() {
    let registry = Arc::new(InMemoryToolRegistry::new());
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let pipeline = DefaultResolutionPipeline::new(registry, sandbox, None);

    pipeline.set_bypass(ResolutionStage::NativeRegistry, true);
    let state = pipeline.bypass_state(&ResolutionStage::NativeRegistry);
    assert!(state.unwrap().engaged);

    pipeline.set_bypass(ResolutionStage::NativeRegistry, false);
    let state = pipeline.bypass_state(&ResolutionStage::NativeRegistry);
    assert!(!state.unwrap().engaged);
}

#[tokio::test]
async fn test_pipeline_stats_track_invocations() {
    let registry = Arc::new(InMemoryToolRegistry::new());
    let sandbox = Arc::new(DefaultSandboxEnforcer::new());
    let pipeline = DefaultResolutionPipeline::new(registry, sandbox, None);

    let _ = pipeline.resolve(make_request("unknown.x")).await;
    let stats = pipeline.pipeline_stats();
    assert!(stats.get("native_registry_attempts").unwrap_or(&0) >= &1);
}
