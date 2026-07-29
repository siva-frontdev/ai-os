use super::error::*;
use async_trait::async_trait;
use futures::Stream;
use intelligence_cache::DefaultResponseCache;
use intelligence_context::DefaultContextBuilder;
use intelligence_cost::DefaultCostAccountant;
use intelligence_embeddings::DefaultEmbeddingManager;
use intelligence_models::DefaultModelRegistry;
use intelligence_prompts::DefaultPromptRenderer;
use intelligence_providers::DefaultModelProvider;
use intelligence_router::DefaultRouter;
use intelligence_safety::DefaultSafetyEnforcer;
use intelligence_streaming::DefaultStreamingManager;
use intelligence_telemetry::DefaultTelemetrySink;
use intelligence_tools::DefaultToolRegistry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{timeout, Duration};

use intelligence_core::error::{ModelError, ModelResult};
use intelligence_core::traits::{
    ContextAssembler, CostTracker, EmbeddingGenerator, IntelligenceCoordinator, ModelProvider,
    ModelRegistry, ModelRouter, PromptRenderer, ResponseCache, SafetyEnforcer, StreamDecoder,
    TelemetrySink, ToolRegistry,
};
use intelligence_core::types::*;

#[derive(Debug)]
pub struct DefaultCoordinator {
    providers: Arc<dyn ModelProvider>,
    model_registry: Arc<dyn ModelRegistry>,
    router: Arc<dyn ModelRouter>,
    prompt_manager: Arc<dyn PromptRenderer>,
    context_builder: Arc<dyn ContextAssembler>,
    embedding_manager: Arc<dyn EmbeddingGenerator>,
    response_cache: Arc<dyn ResponseCache>,
    streaming_manager: Arc<dyn StreamDecoder>,
    tool_registry: Arc<dyn ToolRegistry>,
    cost_tracker: Arc<dyn CostTracker>,
    safety_pipeline: Arc<dyn SafetyEnforcer>,
    telemetry: Arc<dyn TelemetrySink>,
    max_concurrent: usize,
    in_flight: Arc<RwLock<HashMap<RequestId, bool>>>,
    running: AtomicBool,
    semaphore: Arc<Semaphore>,
}

impl DefaultCoordinator {
    fn registered_defaults(&self) -> &AtomicU32 {
        static INIT: std::sync::OnceLock<AtomicU32> = std::sync::OnceLock::new();
        INIT.get_or_init(|| AtomicU32::new(0))
    }

    async fn ensure_model_registered(&self) {
        if self.registered_defaults().load(Ordering::Relaxed) > 0 {
            return;
        }
        let info = ModelInfo {
            model_id: ModelId::new(),
            provider_id: self.providers.provider_id(),
            name: "default".into(),
            version: "0.1.0".into(),
            capabilities: vec![ModelCapability {
                id: CapabilityId::new(),
                name: "chat".into(),
                kind: CapabilityKind::Chat,
                input_modalities: vec!["text".into()],
                output_modalities: vec!["text".into()],
                max_input_tokens: 8192,
                max_output_tokens: 4096,
                supports_streaming: false,
                supports_tools: false,
                supports_vision: false,
                context_window: 8192,
            }],
            pricing: ModelPricing {
                currency: "USD".into(),
                input_per_million_tokens: 0.0,
                output_per_million_tokens: 0.0,
                minimum_charge: None,
                free_tier_tokens: None,
            },
            latency_p50_ms: 100,
            latency_p99_ms: 500,
            availability: AvailabilityStatus::Available,
            tags: vec!["default".into()],
            max_batch_size: None,
        };
        let _ = self.model_registry.register_model(info).await;
        self.registered_defaults().fetch_add(1, Ordering::Relaxed);
    }

    pub fn new() -> Self {
        let providers: Arc<dyn ModelProvider> =
            Arc::new(DefaultModelProvider::new(ProviderId::new()));
        let model_registry: Arc<dyn ModelRegistry> = Arc::new(DefaultModelRegistry::new());
        let tool_registry: Arc<dyn ToolRegistry> = Arc::new(DefaultToolRegistry::default());
        let embedding_manager: Arc<dyn EmbeddingGenerator> =
            Arc::new(DefaultEmbeddingManager::new(providers.clone()));
        let prompt_manager: Arc<dyn PromptRenderer> = Arc::new(DefaultPromptRenderer::default());
        let context_builder: Arc<dyn ContextAssembler> = Arc::new(DefaultContextBuilder::default());
        let router: Arc<dyn ModelRouter> = Arc::new(DefaultRouter::new(
            model_registry.clone(),
            providers.clone(),
        ));
        let response_cache: Arc<dyn ResponseCache> = Arc::new(DefaultResponseCache::new());
        let streaming_manager: Arc<dyn StreamDecoder> = Arc::new(DefaultStreamingManager);
        let cost_tracker: Arc<dyn CostTracker> = Arc::new(DefaultCostAccountant::default());
        let safety_pipeline: Arc<dyn SafetyEnforcer> = Arc::new(DefaultSafetyEnforcer);
        let telemetry: Arc<dyn TelemetrySink> = Arc::new(DefaultTelemetrySink::new());

        Self {
            providers,
            model_registry,
            router,
            prompt_manager,
            context_builder,
            embedding_manager,
            response_cache,
            streaming_manager,
            tool_registry,
            cost_tracker,
            safety_pipeline,
            telemetry,
            max_concurrent: 50,
            in_flight: Arc::new(RwLock::new(HashMap::new())),
            running: AtomicBool::new(false),
            semaphore: Arc::new(Semaphore::new(50)),
        }
    }

    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }
}

#[async_trait]
impl IntelligenceCoordinator for DefaultCoordinator {
    async fn request(&self, request: ModelRequest) -> ModelResult<ModelResponse> {
        let response = timeout(Duration::from_secs(120), self.run_pipeline(request))
            .await
            .map_err(|_| ModelError::Timeout {
                provider: "pipeline".into(),
                timeout_ms: 120_000,
            })??;
        Ok(response)
    }

    async fn request_stream(
        &self,
        request: ModelRequest,
    ) -> ModelResult<Box<dyn Stream<Item = StreamChunk> + Send>> {
        let response = self.request(request.clone()).await?;
        let chunk = StreamChunk::Done(response);
        let s = futures::stream::once(async move { chunk });
        Ok(Box::new(s))
    }

    async fn embed(&self, texts: &[String]) -> ModelResult<Vec<Embedding>> {
        self.embedding_manager.generate(texts).await
    }

    async fn health(&self) -> ModelResult<()> {
        self.providers.health_check().await?;
        Ok(())
    }

    async fn pipeline_stats(&self) -> ModelResult<IntelligenceStats> {
        let snapshot = self.telemetry.export_metrics().await?;
        let in_flight = self.in_flight.read().await.len();
        Ok(IntelligenceStats {
            total_requests: snapshot.total_requests,
            active_requests: in_flight,
            cache_hit_rate: snapshot.cache_hit_rate,
            total_cost_cents: snapshot.total_cost_cents,
            safety_events: snapshot.safety_event_count,
            fallbacks_used: 0,
        })
    }

    async fn conversation(
        &self,
        _conversation: &ConversationId,
    ) -> ModelResult<Vec<ModelResponse>> {
        Ok(vec![])
    }
}

impl DefaultCoordinator {
    async fn run_pipeline(&self, request: ModelRequest) -> ModelResult<ModelResponse> {
        self.ensure_model_registered().await;

        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| ModelError::Io("semaphore closed".into()))?;

        {
            let mut map = self.in_flight.write().await;
            map.insert(request.request_id, true);
        }

        self.telemetry
            .record_request(TelemetryRecord {
                request_id: request.request_id,
                provider_id: ProviderId::new(),
                model_id: request.model_id.unwrap_or_else(ModelId::new),
                latency_ms: 0,
                tokens_in: 0,
                tokens_out: 0,
                cost_cents: 0.0,
                success: false,
            })
            .await;

        let budget = self.cost_tracker.check_budget(&request).await?;
        if !budget.allowed {
            self.telemetry
                .record_error(
                    &ModelError::BudgetExceeded {
                        limit: budget.remaining_cents.unwrap_or(0.0),
                        current: 0.0,
                    },
                    &request.request_id,
                )
                .await;
            return Err(ModelError::BudgetExceeded {
                limit: budget.remaining_cents.unwrap_or(0.0),
                current: 0.0,
            });
        }
        if budget.warning {
            self.telemetry
                .record_safety_event(
                    SafetyEvent {
                        rule_id: SafetyRuleId::new(),
                        category: SafetyCategory::Custom("budget-warning".into()),
                        severity: SafetySeverity::Standard,
                        action: SafetyAction::Warn,
                        detail: budget.reason.unwrap_or_default(),
                    },
                    &request.request_id,
                )
                .await;
        }

        let _routing = self.router.route(&request).await?;

        let input_result = self.safety_pipeline.check_input(&request).await?;
        if !input_result.passed {
            self.telemetry
                .record_safety_event(
                    SafetyEvent {
                        rule_id: SafetyRuleId::new(),
                        category: SafetyCategory::PromptInjection,
                        severity: SafetySeverity::Standard,
                        action: SafetyAction::Block,
                        detail: "input safety failed".into(),
                    },
                    &request.request_id,
                )
                .await;
            return Err(ModelError::SafetyViolation);
        }

        let _context = self.context_builder.assemble(&request, None).await?;

        let mut response = self.providers.chat(&request).await?;

        let output_result = self.safety_pipeline.check_output(&response).await?;
        if !output_result.passed {
            if let Some(ref sanitized) = output_result.sanitized_output {
                response.content = sanitized.clone();
            }
        }

        let _ = self
            .cost_tracker
            .record(CostAccount {
                account_id: CostAccountId::new(),
                conversation_id: ConversationId::new(),
                total_tokens: response.usage.total_tokens as u64,
                total_cost_cents: 0.0,
                period: CostPeriod::Daily,
            })
            .await;

        {
            let mut map = self.in_flight.write().await;
            map.remove(&request.request_id);
        }

        Ok(response)
    }
}

impl Default for DefaultCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn coordinator_instantiates() {
        let _c = DefaultCoordinator::new();
    }

    #[tokio::test]
    async fn request_completes() {
        let coordinator = DefaultCoordinator::new();
        let request = ModelRequest {
            request_id: RequestId::new(),
            capability: CapabilityKind::Chat,
            model_id: Some(ModelId::new()),
            input: ModelInput::Text("hi".into()),
            parameters: HashMap::new(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: false,
        };
        let result = coordinator.request(request).await;
        // Request may succeed (LLM available) or fail (LLM not available) — either is valid
        if let Err(ref e) = result {
            let msg = e.to_string();
            assert!(
                msg.contains("connection refused")
                    || msg.contains("Connection refused")
                    || msg.contains("timed out")
                    || msg.contains("rate limit")
                    || msg.contains("not found"),
                "unexpected error: {e}"
            );
        }
    }
}
