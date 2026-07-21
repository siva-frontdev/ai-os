use super::error::*;
use async_trait::async_trait;
use intelligence_core::error::ModelResult;
use intelligence_core::traits::{ModelProvider, ModelRegistry, ModelRouter};
use intelligence_core::types::{CapabilityKind, ModelInfo, ModelQuery, ModelRequest};
use intelligence_core::{RoutingDecision, RoutingPolicy};
use std::sync::Arc;

#[derive(Debug)]
pub struct DefaultRouter {
    model_registry: Arc<dyn intelligence_core::traits::ModelRegistry>,
    provider_registry: Arc<dyn intelligence_core::traits::ModelProvider>,
}

impl DefaultRouter {
    pub fn new(
        model_registry: Arc<dyn intelligence_core::traits::ModelRegistry>,
        provider_registry: Arc<dyn intelligence_core::traits::ModelProvider>,
    ) -> Self {
        Self {
            model_registry,
            provider_registry,
        }
    }
}

#[async_trait]
impl intelligence_core::traits::ModelRouter for DefaultRouter {
    async fn route(&self, request: &ModelRequest) -> ModelResult<RoutingDecision> {
        let candidates = self
            .model_registry
            .query(&ModelQuery {
                required_capabilities: vec![request.capability.clone()],
                ..Default::default()
            })
            .await
            .unwrap_or_default();

        if candidates.is_empty() {
            let capability = format!("{0:?}", request.capability);
            return Err(RouterError::NoModelFound(capability).into());
        }

        let best = candidates[0].clone();
        let fallbacks: Vec<ModelInfo> = candidates.into_iter().take(3).collect();

        Ok(RoutingDecision {
            primary: best.clone(),
            fallbacks,
            routing_policy: RoutingPolicy::Balanced,
            estimated_latency_ms: best.latency_p50_ms,
            estimated_cost_cents: best.pricing.output_per_million_tokens,
            routing_reason: "sorted by latency".into(),
        })
    }

    async fn route_with_fallback(&self, request: &ModelRequest) -> ModelResult<RoutingDecision> {
        self.route(request).await
    }

    fn set_policy(&self, _policy: RoutingPolicy) {}
    fn policy(&self) -> RoutingPolicy {
        RoutingPolicy::Balanced
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use intelligence_models::DefaultModelRegistry;
    use std::sync::Arc;

    #[derive(Debug)]
    struct StubProvider;
    #[async_trait]
    impl intelligence_core::traits::ModelProvider for StubProvider {
        fn capabilities(&self) -> Vec<intelligence_core::types::ModelCapability> {
            vec![]
        }
        fn provider_id(&self) -> intelligence_core::types::ProviderId {
            intelligence_core::types::ProviderId::new()
        }
        async fn chat(
            &self,
            _: &intelligence_core::types::ModelRequest,
        ) -> intelligence_core::error::ModelResult<intelligence_core::types::ModelResponse>
        {
            Ok(intelligence_core::types::ModelResponse {
                request_id: intelligence_core::types::RequestId::new(),
                model_id: intelligence_core::types::ModelId::new(),
                content: String::new(),
                usage: Default::default(),
                finished: true,
                finish_reason: None,
            })
        }
        async fn chat_stream(
            &self,
            _: &intelligence_core::types::ModelRequest,
        ) -> intelligence_core::error::ModelResult<intelligence_core::traits::ChatStream> {
            Ok(futures::stream::empty())
        }
        async fn embed(
            &self,
            _: &intelligence_core::types::ModelRequest,
        ) -> intelligence_core::error::ModelResult<Vec<intelligence_core::types::Embedding>>
        {
            Ok(vec![])
        }
        async fn classify(
            &self,
            _: &intelligence_core::types::ModelRequest,
        ) -> intelligence_core::error::ModelResult<String> {
            Ok(String::new())
        }
        async fn health_check(
            &self,
        ) -> intelligence_core::error::ModelResult<intelligence_core::types::ModelInfo> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn router_smoke() {
        let registry: Arc<dyn intelligence_core::traits::ModelRegistry> =
            Arc::new(DefaultModelRegistry::new());
        let provider_registry: Arc<dyn intelligence_core::traits::ModelProvider> =
            Arc::new(StubProvider);
        let router = DefaultRouter::new(registry.clone(), provider_registry);
        let req = intelligence_core::types::ModelRequest {
            request_id: intelligence_core::types::RequestId::new(),
            capability: intelligence_core::types::CapabilityKind::Chat,
            model_id: None,
            input: intelligence_core::types::ModelInput::Text("hi".into()),
            parameters: Default::default(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: false,
        };
        let _ = router.route(&req).await;
    }
}
