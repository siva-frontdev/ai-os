use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

// -- IDs -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderId(pub Uuid);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(pub Uuid);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityId(pub Uuid);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PromptId(pub Uuid);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub Uuid);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafetyRuleId(pub Uuid);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CostAccountId(pub Uuid);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConversationId(pub Uuid);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingId(pub Uuid);

impl ProviderId {
    pub fn new() -> Self {
        ProviderId(Uuid::now_v7())
    }
}
impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for ProviderId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(ProviderId)
    }
}

impl ModelId {
    pub fn new() -> Self {
        ModelId(Uuid::now_v7())
    }
}
impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl CapabilityId {
    pub fn new() -> Self {
        CapabilityId(Uuid::now_v7())
    }
}
impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl PromptId {
    pub fn new() -> Self {
        PromptId(Uuid::now_v7())
    }
}
impl fmt::Display for PromptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl RequestId {
    pub fn new() -> Self {
        RequestId(Uuid::now_v7())
    }
}
impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl SafetyRuleId {
    pub fn new() -> Self {
        SafetyRuleId(Uuid::now_v7())
    }
}
impl fmt::Display for SafetyRuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl CostAccountId {
    pub fn new() -> Self {
        CostAccountId(Uuid::now_v7())
    }
}
impl fmt::Display for CostAccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl ConversationId {
    pub fn new() -> Self {
        ConversationId(Uuid::now_v7())
    }
}
impl fmt::Display for ConversationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl EmbeddingId {
    pub fn new() -> Self {
        EmbeddingId(Uuid::now_v7())
    }
}
impl fmt::Display for EmbeddingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for ModelId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(ModelId)
    }
}

// -- Capability ----------------------------------------------------------------

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityKind {
    Chat,
    Embedding,
    Classification,
    Generation,
    CodeGeneration,
    Summarization,
    Transcription,
    Translation,
    SafetyClassification,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapability {
    pub id: CapabilityId,
    pub name: String,
    pub kind: CapabilityKind,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub context_window: u32,
}

// -- Model metadata ------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub currency: String,
    pub input_per_million_tokens: f64,
    pub output_per_million_tokens: f64,
    pub minimum_charge: Option<f64>,
    pub free_tier_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AvailabilityStatus {
    Available,
    Degraded { reason: String },
    Unavailable { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub model_id: ModelId,
    pub provider_id: ProviderId,
    pub name: String,
    pub version: String,
    pub capabilities: Vec<ModelCapability>,
    pub pricing: ModelPricing,
    pub latency_p50_ms: u64,
    pub latency_p99_ms: u64,
    pub availability: AvailabilityStatus,
    pub tags: Vec<String>,
    pub max_batch_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistration {
    pub model_id: ModelId,
    pub provider_id: ProviderId,
    pub name: String,
    pub capabilities: Vec<CapabilityKind>,
    pub context_window: u32,
    pub pricing: ModelPricing,
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelQuery {
    pub required_capabilities: Vec<CapabilityKind>,
    pub max_latency_ms: Option<u64>,
    pub max_cost_cents: Option<f64>,
    pub tags: Option<Vec<String>>,
}

// -- Provider ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<ModelCapability>,
    pub health: ProviderHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderHealth {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

// -- Request / Response --------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub request_id: RequestId,
    pub capability: CapabilityKind,
    pub model_id: Option<ModelId>,
    pub input: ModelInput,
    pub parameters: HashMap<String, String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelInput {
    Text(String),
    Multimodal(Vec<MultimodalPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalPart {
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub request_id: RequestId,
    pub model_id: ModelId,
    pub content: String,
    pub usage: TokenUsage,
    pub finished: bool,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// -- Prompt --------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: PromptId,
    pub name: String,
    pub content: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedPrompt {
    pub content: String,
    pub token_count: usize,
}

// -- Context -------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextWindow {
    pub system_prompt: Option<String>,
    pub messages: Vec<String>,
    pub retrieved_memories: Vec<String>,
    pub observations: Vec<String>,
    pub tool_definitions: Vec<String>,
    pub total_tokens: u32,
    pub max_tokens: u32,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TruncationStrategy {
    SlidingWindow {
        keep_last_n: u32,
    },
    Summarize {
        summarize_model: ModelId,
        summary_max_tokens: u32,
    },
    Relevance {
        similarity_threshold: f32,
        max_messages: u32,
    },
}

// -- Embedding ----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub id: EmbeddingId,
    pub vector: Vec<f32>,
}

// -- Safety --------------------------------------------------------------------

// -- Cache ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheKey {
    pub model_id: ModelId,
    pub capability: CapabilityKind,
    pub input_hash: String,
    pub parameters_hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub size_bytes: u64,
    pub entry_count: usize,
}

// -- Safety --------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRule {
    pub id: SafetyRuleId,
    pub category: SafetyCategory,
    pub severity: SafetySeverity,
    pub action: SafetyAction,
    pub pattern: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyCategory {
    Pii,
    PromptInjection,
    Jailbreak,
    Toxicity,
    Bias,
    SexualContent,
    Violence,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetySeverity {
    None,
    Standard,
    Strict,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetyAction {
    Block,
    Redact,
    Warn,
    Log,
    Sanitize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyResult {
    pub passed: bool,
    pub events: Vec<SafetyEvent>,
    pub sanitized_input: Option<String>,
    pub sanitized_output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyEvent {
    pub rule_id: SafetyRuleId,
    pub category: SafetyCategory,
    pub severity: SafetySeverity,
    pub action: SafetyAction,
    pub detail: String,
}

// -- Cost ----------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAccount {
    pub account_id: CostAccountId,
    pub conversation_id: ConversationId,
    pub total_tokens: u64,
    pub total_cost_cents: f64,
    pub period: CostPeriod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CostPeriod {
    Daily,
    Monthly,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetCheck {
    pub allowed: bool,
    pub remaining_cents: Option<f64>,
    pub warning: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    pub period: CostPeriod,
    pub total_tokens: u64,
    pub total_cost_cents: f64,
}

// -- Telemetry ----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    pub request_id: RequestId,
    pub provider_id: ProviderId,
    pub model_id: ModelId,
    pub latency_ms: u64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_cents: f64,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetrics {
    pub requests: u64,
    pub avg_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub error_rate: f64,
    pub cost_cents: f64,
}

// -- Tools ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub call_id: String,
    pub name: Option<String>,
    pub arguments_delta: Option<String>,
}

// -- Streaming ----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamChunk {
    ContentDelta(String),
    ToolCallDelta(ToolCallDelta),
    SafetyEvent(SafetyEvent),
    UsageUpdate(TokenUsage),
    Done(ModelResponse),
    Error { error: String, recoverable: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamHandler {
    pub request_id: RequestId,
    pub buffer: Vec<StreamChunk>,
    pub tool_call_buffers: HashMap<String, ToolCallDelta>,
    pub safety_events: Vec<SafetyEvent>,
    pub usage: TokenUsage,
    pub finished: bool,
}

// -- Coordination --------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceStats {
    pub total_requests: u64,
    pub active_requests: usize,
    pub cache_hit_rate: f64,
    pub total_cost_cents: f64,
    pub safety_events: u32,
    pub fallbacks_used: u64,
}
