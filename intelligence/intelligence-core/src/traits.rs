use super::error::ModelResult;
use super::types::*;
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;

use crate::ModelError;
use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

pub type ChatStream = futures::stream::Empty<bytes::Bytes>;

#[async_trait]
pub trait ModelProvider: Send + Sync + std::fmt::Debug {
    async fn chat(&self, request: &ModelRequest) -> ModelResult<ModelResponse>;
    async fn chat_stream(&self, request: &ModelRequest) -> ModelResult<ChatStream>;
    async fn embed(&self, request: &ModelRequest) -> ModelResult<Vec<Embedding>>;
    async fn classify(&self, request: &ModelRequest) -> ModelResult<String>;
    async fn health_check(&self) -> ModelResult<ModelInfo>;
    fn capabilities(&self) -> Vec<ModelCapability>;
    fn provider_id(&self) -> ProviderId;
}

#[async_trait]
pub trait ModelRegistry: Send + Sync + std::fmt::Debug {
    async fn register_model(&self, info: ModelInfo) -> ModelResult<()>;
    async fn deregister_model(&self, model_id: &ModelId) -> ModelResult<()>;
    async fn query(&self, query: &ModelQuery) -> ModelResult<Vec<ModelInfo>>;
    async fn get(&self, model_id: &ModelId) -> ModelResult<Option<ModelInfo>>;
    async fn list_capabilities(&self, kind: CapabilityKind) -> ModelResult<Vec<ModelInfo>>;
    async fn refresh(&self) -> ModelResult<()>;
}

#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub primary: ModelInfo,
    pub fallbacks: Vec<ModelInfo>,
    pub routing_policy: RoutingPolicy,
    pub estimated_latency_ms: u64,
    pub estimated_cost_cents: f64,
    pub routing_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingPolicy {
    BestCapability,
    LowestCost,
    LowestLatency,
    Balanced,
}

#[async_trait]
pub trait ModelRouter: Send + Sync + std::fmt::Debug {
    async fn route(&self, request: &ModelRequest) -> ModelResult<RoutingDecision>;
    async fn route_with_fallback(&self, request: &ModelRequest) -> ModelResult<RoutingDecision>;
    fn set_policy(&self, policy: RoutingPolicy);
    fn policy(&self) -> RoutingPolicy;
}

#[async_trait]
pub trait PromptRenderer: Send + Sync + std::fmt::Debug {
    async fn render(
        &self,
        template: &PromptTemplate,
        variables: &HashMap<String, String>,
    ) -> ModelResult<RenderedPrompt>;
    async fn render_with_context(
        &self,
        template: &PromptTemplate,
        variables: &HashMap<String, String>,
        context: &ContextWindow,
    ) -> ModelResult<RenderedPrompt>;
    async fn register_template(&self, template: PromptTemplate) -> ModelResult<()>;
    async fn get_template(&self, template_id: &PromptId) -> ModelResult<Option<PromptTemplate>>;
    async fn list_templates(&self, tags: &[String]) -> ModelResult<Vec<PromptTemplate>>;
}

#[async_trait]
pub trait ContextAssembler: Send + Sync + std::fmt::Debug {
    async fn assemble(
        &self,
        request: &ModelRequest,
        conversation: Option<&ConversationId>,
    ) -> ModelResult<ContextWindow>;
    async fn assemble_with_memory(
        &self,
        request: &ModelRequest,
        conversation: Option<&ConversationId>,
        memory_query: &str,
    ) -> ModelResult<ContextWindow>;
    fn set_truncation_strategy(&self, strategy: TruncationStrategy);
}

#[async_trait]
pub trait EmbeddingGenerator: Send + Sync + std::fmt::Debug {
    async fn generate(&self, texts: &[String]) -> ModelResult<Vec<Embedding>>;
    async fn generate_single(&self, text: &str) -> ModelResult<Embedding>;
    async fn similarity(&self, a: &Embedding, b: &Embedding) -> ModelResult<f32>;
    async fn search(
        &self,
        query: &Embedding,
        corpus: &[Embedding],
        top_k: u32,
    ) -> ModelResult<Vec<(usize, f32)>>;
    fn dimensions(&self) -> u32;
    fn model_id(&self) -> ModelId;
}

#[async_trait]
pub trait ResponseCache: Send + Sync + std::fmt::Debug {
    async fn get(&self, key: &CacheKey) -> ModelResult<Option<ModelResponse>>;
    async fn put(&self, key: CacheKey, response: ModelResponse, ttl: Duration) -> ModelResult<()>;
    async fn invalidate(&self, model_id: &ModelId) -> ModelResult<u64>;
    async fn clear(&self) -> ModelResult<()>;
    fn stats(&self) -> CacheStats;
}

#[async_trait]
pub trait StreamDecoder: Send + Sync + std::fmt::Debug {
    async fn decode(
        &self,
        stream: Pin<Box<dyn Stream<Item = Bytes> + Send>>,
    ) -> ModelResult<StreamHandler>;
    fn aggregate_chunks(&self, handler: StreamHandler) -> ModelResult<ModelResponse>;
}

#[async_trait]
pub trait SafetyEnforcer: Send + Sync + std::fmt::Debug {
    async fn check_input(&self, request: &ModelRequest) -> ModelResult<SafetyResult>;
    async fn check_output(&self, response: &ModelResponse) -> ModelResult<SafetyResult>;
    async fn add_rule(&self, rule: SafetyRule) -> ModelResult<()>;
    async fn remove_rule(&self, rule_id: &SafetyRuleId) -> ModelResult<()>;
    async fn list_rules(&self) -> ModelResult<Vec<SafetyRule>>;
}

#[async_trait]
pub trait CostTracker: Send + Sync + std::fmt::Debug {
    async fn record(&self, account: CostAccount) -> ModelResult<()>;
    async fn check_budget(&self, request: &ModelRequest) -> ModelResult<BudgetCheck>;
    async fn get_report(&self, period: CostPeriod) -> ModelResult<CostReport>;
    async fn get_conversation_cost(&self, conversation_id: &ConversationId) -> ModelResult<f64>;
}

#[async_trait]
pub trait TelemetrySink: Send + Sync + std::fmt::Debug {
    async fn record_request(&self, record: TelemetryRecord);
    async fn record_safety_event(&self, event: SafetyEvent, request_id: &RequestId);
    async fn record_error(&self, error: &ModelError, request_id: &RequestId);
    async fn export_metrics(&self) -> ModelResult<MetricsSnapshot>;
}

#[async_trait]
pub trait ToolRegistry: Send + Sync + std::fmt::Debug {
    async fn register(&self, definition: ToolDefinition) -> ModelResult<()>;
    async fn deregister(&self, name: &str) -> ModelResult<()>;
    async fn get(&self, name: &str) -> ModelResult<Option<ToolDefinition>>;
    async fn list(&self) -> ModelResult<Vec<ToolDefinition>>;
    fn to_provider_format(&self, provider_kind: &str) -> serde_json::Value;
}

#[async_trait]
pub trait IntelligenceCoordinator: Send + Sync + std::fmt::Debug {
    async fn request(&self, request: ModelRequest) -> ModelResult<ModelResponse>;
    async fn request_stream(
        &self,
        request: ModelRequest,
    ) -> ModelResult<Box<dyn Stream<Item = StreamChunk> + Send>>;
    async fn embed(&self, texts: &[String]) -> ModelResult<Vec<Embedding>>;
    async fn conversation(&self, conversation: &ConversationId) -> ModelResult<Vec<ModelResponse>>;
    async fn health(&self) -> ModelResult<()>;
    async fn pipeline_stats(&self) -> ModelResult<IntelligenceStats>;
}
