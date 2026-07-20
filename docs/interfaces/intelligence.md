# Intelligence Platform Interfaces

## Overview

This document defines the public API surfaces of the Intelligence Platform: traits, events, error types, and key data structures. All code is Rust-like pseudocode. No implementation details are embedded.

---

## Traits

### ModelProvider

The fundamental trait that all AI provider adapters implement. Each provider wraps a specific SDK or API and translates between the platform's canonical types and the provider's native types.

```rust
#[async_trait]
pub trait ModelProvider: Debug + Send + Sync {
    async fn chat(&self, request: &ModelRequest) -> ModelResult<ModelResponse>;
    async fn chat_stream(&self, request: &ModelRequest) -> ModelResult<StreamHandle>;
    async fn embed(&self, request: &EmbeddingRequest) -> ModelResult<Vec<Embedding>>;
    async fn classify(&self, request: &ClassificationRequest) -> ModelResult<ClassificationResult>;
    async fn health_check(&self) -> ModelResult<ProviderHealth>;
    fn capabilities(&self) -> Vec<ModelCapability>;
    fn provider_info(&self) -> ProviderInfo;
}
```

### ModelRegistry

Manages model discovery and lookup.

```rust
#[async_trait]
pub trait ModelRegistry: Debug + Send + Sync {
    async fn register_model(&self, registration: ModelRegistration) -> ModelResult<()>;
    async fn deregister_model(&self, model_id: &ModelId) -> ModelResult<()>;
    async fn query(&self, query: &ModelQuery) -> ModelResult<Vec<ModelInfo>>;
    async fn get(&self, model_id: &ModelId) -> ModelResult<Option<ModelInfo>>;
    async fn list_capabilities(&self, kind: CapabilityKind) -> ModelResult<Vec<ModelInfo>>;
    async fn refresh(&self) -> ModelResult<()>;
}
```

### ModelRouter

Selects the optimal model for a request.

```rust
#[async_trait]
pub trait ModelRouter: Debug + Send + Sync {
    async fn route(&self, request: &ModelRequest) -> ModelResult<RoutingDecision>;
    async fn route_with_fallback(&self, request: &ModelRequest) -> ModelResult<RoutingDecision>;
    fn set_policy(&self, policy: RoutingPolicy);
    fn policy(&self) -> RoutingPolicy;
}
```

```rust
pub struct RoutingDecision {
    pub primary: ModelInfo,
    pub fallbacks: Vec<ModelInfo>,
    pub routing_policy: RoutingPolicy,
    pub estimated_latency_ms: u64,
    pub estimated_cost_cents: f64,
    pub routing_reason: String,
}
```

### PromptRenderer

Renders prompt templates into final prompt text.

```rust
#[async_trait]
pub trait PromptRenderer: Debug + Send + Sync {
    async fn render(&self, template: &PromptTemplate, variables: &HashMap<String, String>) -> ModelResult<RenderedPrompt>;
    async fn render_with_context(&self, template: &PromptTemplate, variables: &HashMap<String, String>, context: &ContextWindow) -> ModelResult<RenderedPrompt>;
    async fn register_template(&self, template: PromptTemplate) -> ModelResult<()>;
    async fn get_template(&self, template_id: &PromptId) -> ModelResult<Option<PromptTemplate>>;
    async fn list_templates(&self, tags: &[String]) -> ModelResult<Vec<PromptTemplate>>;
}
```

### ContextAssembler

Assembles the full context window for a request from multiple sources.

```rust
#[async_trait]
pub trait ContextAssembler: Debug + Send + Sync {
    async fn assemble(&self, request: &ModelRequest, conversation: Option<&Conversation>) -> ModelResult<ContextWindow>;
    async fn assemble_with_memory(&self, request: &ModelRequest, conversation: Option<&Conversation>, memory_query: &str) -> ModelResult<ContextWindow>;
    fn set_truncation_strategy(&self, strategy: TruncationStrategy);
}

pub struct ContextWindow {
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
    pub retrieved_memories: Vec<MemoryReference>,
    pub observations: Vec<ObservationReference>,
    pub tool_definitions: Vec<ToolDefinition>,
    pub total_tokens: u32,
    pub max_tokens: u32,
    pub truncated: bool,
}

pub enum TruncationStrategy {
    SlidingWindow { keep_last_n: u32 },
    Summarize { summarize_model: ModelId, summary_max_tokens: u32 },
    Relevance { similarity_threshold: f32, max_messages: u32 },
}
```

### EmbeddingGenerator

Generates and manages embeddings.

```rust
#[async_trait]
pub trait EmbeddingGenerator: Debug + Send + Sync {
    async fn generate(&self, texts: &[String]) -> ModelResult<Vec<Embedding>>;
    async fn generate_single(&self, text: &str) -> ModelResult<Embedding>;
    async fn similarity(&self, a: &Embedding, b: &Embedding) -> ModelResult<f32>;
    async fn search(&self, query: &Embedding, corpus: &[Embedding], top_k: u32) -> ModelResult<Vec<(usize, f32)>>;
    fn dimensions(&self) -> u32;
    fn model_id(&self) -> ModelId;
}
```

### ResponseCache

Caches model responses keyed by request content.

```rust
#[async_trait]
pub trait ResponseCache: Debug + Send + Sync {
    async fn get(&self, key: &CacheKey) -> ModelResult<Option<ModelResponse>>;
    async fn put(&self, key: CacheKey, response: ModelResponse, ttl: Duration) -> ModelResult<()>;
    async fn invalidate(&self, model_id: &ModelId) -> ModelResult<u64>;
    async fn clear(&self) -> ModelResult<()>;
    async fn stats(&self) -> CacheStats;
}

pub struct CacheKey {
    pub model_id: ModelId,
    pub capability: CapabilityKind,
    pub input_hash: String,
    pub parameters_hash: String,
}

pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub size_bytes: u64,
    pub entry_count: usize,
}
```

### StreamDecoder

Decodes streaming responses.

```rust
#[async_trait]
pub trait StreamDecoder: Debug + Send + Sync {
    async fn decode(&self, stream: impl Stream<Item = bytes::Bytes>) -> ModelResult<StreamHandler>;
    fn aggregate_chunks(&self, handler: StreamHandler) -> ModelResult<ModelResponse>;
}

pub struct StreamHandler {
    pub id: RequestId,
    pub buffer: Vec<StreamChunk>,
    pub tool_call_buffers: HashMap<String, ToolCallDelta>,
    pub safety_events: Vec<SafetyEvent>,
    pub usage: TokenUsage,
    pub finished: bool,
}

pub enum StreamChunk {
    ContentDelta(String),
    ToolCallDelta(ToolCallDelta),
    SafetyEvent(SafetyEvent),
    UsageUpdate(TokenUsage),
    Done(ModelResponseMetadata),
    Error { error: String, recoverable: bool },
}

pub struct ToolCallDelta {
    pub call_id: String,
    pub name: Option<String>,
    pub arguments_delta: Option<String>,
}

pub struct ModelResponseMetadata {
    pub finish_reason: FinishReason,
    pub model_id: ModelId,
    pub provider_id: ProviderId,
}
```

### SafetyEnforcer

Applies safety rules to model inputs and outputs.

```rust
#[async_trait]
pub trait SafetyEnforcer: Debug + Send + Sync {
    async fn check_input(&self, request: &ModelRequest) -> ModelResult<SafetyResult>;
    async fn check_output(&self, response: &ModelResponse) -> ModelResult<SafetyResult>;
    async fn add_rule(&self, rule: SafetyRule) -> ModelResult<()>;
    async fn remove_rule(&self, rule_id: &SafetyRuleId) -> ModelResult<()>;
    async fn list_rules(&self) -> ModelResult<Vec<SafetyRule>>;
}

pub struct SafetyResult {
    pub passed: bool,
    pub events: Vec<SafetyEvent>,
    pub sanitized_input: Option<String>,
    pub sanitized_output: Option<String>,
}
```

### CostTracker

Tracks token usage and cost.

```rust
#[async_trait]
pub trait CostTracker: Debug + Send + Sync {
    async fn record(&self, account: CostAccount) -> ModelResult<()>;
    async fn check_budget(&self, request: &ModelRequest) -> ModelResult<BudgetCheck>;
    async fn get_report(&self, period: CostPeriod) -> ModelResult<CostReport>;
    async fn get_conversation_cost(&self, conversation_id: &ConversationId) -> ModelResult<f64>;
}

pub struct BudgetCheck {
    pub allowed: bool,
    pub remaining_cents: Option<f64>,
    pub warning: bool,
    pub reason: Option<String>,
}
```

### TelemetrySink

Exports metrics and audit records.

```rust
#[async_trait]
pub trait TelemetrySink: Debug + Send + Sync {
    async fn record_request(&self, record: TelemetryRecord);
    async fn record_safety_event(&self, event: SafetyEvent, request_id: &RequestId);
    async fn record_error(&self, error: &ModelError, request_id: &RequestId);
    async fn export_metrics(&self) -> ModelResult<MetricsSnapshot>;
}

pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub total_latency_ms_sum: u64,
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub total_cost_cents: f64,
    pub cache_hit_rate: f64,
    pub safety_event_count: u32,
    pub error_rate: f64,
    pub per_provider: HashMap<ProviderId, ProviderMetrics>,
}

pub struct ProviderMetrics {
    pub requests: u64,
    pub avg_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub error_rate: f64,
    pub cost_cents: f64,
}
```

### ToolRegistry (Intelligence)

```rust
#[async_trait]
pub trait ToolRegistry: Debug + Send + Sync {
    async fn register(&self, definition: ToolDefinition) -> ModelResult<()>;
    async fn deregister(&self, name: &str) -> ModelResult<()>;
    async fn get(&self, name: &str) -> ModelResult<Option<ToolDefinition>>;
    async fn list(&self) -> ModelResult<Vec<ToolDefinition>>;
    fn to_provider_format(&self, provider_kind: &ProviderKind) -> serde_json::Value;
}
```

### Coordinator

```rust
#[async_trait]
pub trait IntelligenceCoordinator: Debug + Send + Sync {
    async fn request(&self, request: ModelRequest) -> ModelResult<ModelResponse>;
    async fn request_stream(&self, request: ModelRequest) -> ModelResult<impl Stream<Item = StreamChunk>>;
    async fn embed(&self, texts: &[String]) -> ModelResult<Vec<Embedding>>;
    async fn conversation(&self, conversation: &Conversation) -> ModelResult<ModelResponse>;
    async fn health(&self) -> ModelResult<()>;
    async fn pipeline_stats(&self) -> ModelResult<IntelligenceStats>;
}

pub struct IntelligenceStats {
    pub total_requests: u64,
    pub active_requests: usize,
    pub cache_hit_rate: f64,
    pub total_cost_cents: f64,
    pub safety_events: u32,
    pub fallbacks_used: u64,
}
```

---

## Events

### Published Events

| Event Type | Payload | Description |
|---|---|---|
| `intelligence.provider.registered` | `ProviderRegisteredPayload` | New provider registered |
| `intelligence.provider.deregistered` | `ProviderDeregisteredPayload` | Provider removed |
| `intelligence.provider.health_changed` | `ProviderHealthChangedPayload` | Provider status changed |
| `intelligence.model.registered` | `ModelRegisteredPayload` | New model registered |
| `intelligence.model.deregistered` | `ModelDeregisteredPayload` | Model removed |
| `intelligence.request.started` | `RequestStartedPayload` | Inference request started |
| `intelligence.request.completed` | `RequestCompletedPayload` | Inference completed successfully |
| `intelligence.request.failed` | `RequestFailedPayload` | Inference failed |
| `intelligence.request.fallback_used` | `FallbackUsedPayload` | Fell back to secondary model |
| `intelligence.request.cached` | `RequestCachedPayload` | Served from cache |
| `intelligence.stream.chunk` | `StreamChunkPayload` | Streaming chunk (if streamed) |
| `intelligence.stream.done` | `StreamDonePayload` | Stream completed |
| `intelligence.safety.violation` | `SafetyViolationPayload` | Safety rule triggered |
| `intelligence.safety.blocked` | `SafetyBlockedPayload` | Request/response blocked |
| `intelligence.cost.budget_warning` | `BudgetWarningPayload` | Cost budget approaching limit |
| `intelligence.cost.budget_exceeded` | `BudgetExceededPayload` | Cost budget exceeded |
| `intelligence.cache.hit` | `CacheHitPayload` | Cache hit |
| `intelligence.cache.miss` | `CacheMissPayload` | Cache miss |
| `intelligence.coordinator.ready` | `CoordinatorStatusPayload` | Coordinator initialized |

### Consumed Events

| Event Type | Source | Handler | Description |
|---|---|---|---|
| `core.lifecycle.stopping` | Core Platform | Coordinator | Graceful drain of in-flight requests |
| `core.config.reloaded` | Core Platform | All modules | Hot-reload configuration |
| `runtime.session_closed` | Runtime Platform | Context | End conversation for closed session |
| `memory.memory_stored` | Memory Platform | Context | New memory available for retrieval |
| `brain.goal.updated` | Brain Platform | Coordinator | New goal requiring inference |
| `execution.tool_result.ready` | Execution Platform | Context | Tool execution result for tool-calling loop |
| `perception.observation.ready` | Perception Platform | Context | New observation for context enrichment |

---

## Error Model

```rust
pub enum ModelError {
    // Provider errors (1xxx)
    ConnectionFailed { provider: String, detail: String },
    AuthenticationFailed { provider: String },
    RateLimited { provider: String, retry_after_ms: u64 },
    Timeout { provider: String, timeout_ms: u64 },
    ServerError { provider: String, status: u16 },
    InvalidResponse { provider: String, detail: String },

    // Model errors (2xxx)
    ModelNotFound { model_id: ModelId },
    CapabilityMismatch { required: CapabilityKind, available: Vec<CapabilityKind> },
    ContextWindowExceeded { current: u32, max: u32 },
    ContentFiltered { reason: String },
    SafetyViolation { events: Vec<SafetyEvent> },

    // Routing errors (3xxx)
    NoCapableModel { capability: CapabilityKind },
    AllProvidersUnavailable { capability: CapabilityKind },
    NoFallbackAvailable { model_id: ModelId },

    // Prompt errors (4xxx)
    TemplateNotFound { template_id: PromptId },
    RenderError { detail: String },
    VariableMissing { variable: String },
    ContextTooLong { tokens: u32, max: u32 },

    // Safety errors (5xxx)
    SafetyRuleViolation { rule_id: SafetyRuleId, category: SafetyCategory },
    SafetyFrameworkError { detail: String },

    // Cost errors (6xxx)
    BudgetExceeded { limit: f64, current: f64 },
    QuotaExceeded { period: CostPeriod },

    // Cache errors (7xxx)
    CacheWriteFailed { detail: String },
    CacheReadFailed { detail: String },

    // Core errors
    Core(#[from] CoreError),
    Io(#[from] std::io::Error),
    Serialization(#[from] serde_json::Error),
}
```

---

## Dependency Table

| Crate | Depends On | Purpose |
|---|---|---|
| `intelligence-core` | `ai-os-core`, `memory-core`, `ai-os-runtime` | Foundation types, EventBus, Timestamp, Context, Budget |
| `intelligence-providers` | `intelligence-core`, `ai-os-core` | Provider adapters, health checks, authentication |
| `intelligence-models` | `intelligence-core`, `intelligence-providers` | Model registry, capability discovery |
| `intelligence-router` | `intelligence-core`, `intelligence-models`, `intelligence-providers`, `intelligence-cache`, `intelligence-cost` | Model selection, failover |
| `intelligence-prompts` | `intelligence-core` | Template management, rendering |
| `intelligence-context` | `intelligence-core`, `ai-os-memory-core`, `ai-os-perception-core` | Context assembly, Memory retrieval |
| `intelligence-embeddings` | `intelligence-core`, `intelligence-models` | Embedding generation, similarity |
| `intelligence-cache` | `intelligence-core`, `ai-os-memory-storage` | Response cache |
| `intelligence-streaming` | `intelligence-core` | Streaming aggregation |
| `intelligence-tools` | `intelligence-core`, `execution-core` | Tool-calling abstraction |
| `intelligence-cost` | `intelligence-core`, `intelligence-telemetry` | Token accounting, budgets |
| `intelligence-safety` | `intelligence-core`, `intelligence-models` | Safety classifiers, filtering |
| `intelligence-telemetry` | `intelligence-core`, `ai-os-runtime` | Metrics, tracing, audit |
| `intelligence-coordinator` | All intelligence crates, `ai-os-brain-core`, `ai-os-memory-core`, `execution-core` | Top-level orchestration |
