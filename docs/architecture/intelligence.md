# Intelligence Platform Architecture

## Purpose

The Intelligence Platform (Phase 9) is the AI cognition layer of the AI-native OS. It provides a provider-agnostic abstraction over all AI models and services — language models, embedding models, safety classifiers, and multimodal models — so that the Brain Platform and every other platform module can reason, generate, embed, and classify without importing any provider SDK.

The Intelligence Platform sits between the **Brain Platform** (above, consumer) and the **Execution Platform** (below, for tool-calling actions). It depends on the **Memory Platform** for long-term context and retrieval, on **Runtime** for session and context management, on **Core** for EventBus, Service lifecycle, logging, configuration, and on the **Perception Platform** for enriched observations as model inputs.

---

## Design Principles

1. **Provider Agnostic** — No platform code imports an OpenAI, Anthropic, Google, or Cohere SDK. Providers implement a standard `ModelProvider` trait and are loaded as plugins. Adding a new provider requires zero changes to the Brain or any other consumer.

2. **Capability-Based Routing** — Requests are routed to models based on declared capabilities (`Chat`, `Embed`, `Classify`, `GenerateCode`), latency requirements, cost ceiling, and availability — not by hard-coded model names.

3. **Single Responsibility per Crate** — The 14 crates each own one concern. Prompt rendering knows nothing about streaming. Token accounting knows nothing about model providers. Context assembly knows nothing about prompt templates.

4. **Event-Driven Communication** — All inter-crate communication flows through typed events on the EventBus. No direct references between intelligence crates.

5. **Failover Built In** — Every request can fail over to a secondary provider and model if the primary returns an error, times out, or is rate-limited. Fallback policies are configurable per-request or globally.

6. **Cost and Latency Aware** — Token usage, cost, and latency are tracked per-request and exposed as metrics. The router can select the cheapest model meeting a capability requirement or the fastest model within a latency budget.

7. **Safety First** — All responses pass through a configurable safety pipeline — PII detection, prompt-injection detection, jailbreak detection, toxicity filtering — before they reach the Brain or any other consumer. Safety failures produce `SafetyViolation` events and sanitized responses.

8. **Observable** — Every request emits events (`intelligence.request.*`). Token counts, latency histograms, cost accrual, cache hit rates, and safety event counts are tracked as metrics.

---

## Layer Position

```
intelligence/                     (Layer 8 — this RFC)
  |
  v
brain/                            (Layer 7 — primary consumer)
  |
  v
execution/                        (Layer 6 — tool-calling consumer)
  |
  v
perception/                       (Layer 5 — observation enrichment)
  |
  v
memory/                           (Layer 4 — long-term context, retrieval)
  |
  v
runtime/                          (Layer 3 — sessions, context, permissions)
  |
  v
core/                             (Layer 2 — EventBus, Service, Logger, Config)
```

---

## Crate Structure

```
intelligence/
  +-- intelligence-core/          Foundation: types, errors, events, base traits
  +-- intelligence-providers/     Provider registry, lifecycle, health checks
  +-- intelligence-models/        Model registry, capability discovery, metadata
  +-- intelligence-router/        Model routing: capability, latency, cost, availability
  +-- intelligence-prompts/       Prompt templates, rendering, variable substitution
  +-- intelligence-context/       Context assembly from Memory, Perception, Conversation
  +-- intelligence-embeddings/    Embedding generation, similarity, vector operations
  +-- intelligence-cache/         Response caching, semantic dedup, TTL management
  +-- intelligence-streaming/     Streaming response pipeline, chunk aggregation
  +-- intelligence-tools/         Tool-calling abstraction, schema validation
  +-- intelligence-cost/          Token accounting, cost tracking, budget enforcement
  +-- intelligence-safety/        Safety classifiers, PII detection, prompt-injection guard
  +-- intelligence-telemetry/     Metrics, tracing, request logging, audit trail
  +-- intelligence-coordinator/   Top-level orchestration, pipeline wiring, lifecycle
```

---

## Dependency Graph

```
intelligence-core
  ├── intelligence-providers (core, Runtime, OSAL network)
  ├── intelligence-models (core, providers)
  ├── intelligence-router (core, models, providers, cache, cost)
  ├── intelligence-prompts (core)
  ├── intelligence-context (core, Memory, Perception, conversation)
  ├── intelligence-embeddings (core, models)
  ├── intelligence-cache (core, memory storage)
  ├── intelligence-streaming (core)
  ├── intelligence-tools (core, Execution ToolRegistry)
  ├── intelligence-cost (core, telemetry)
  ├── intelligence-safety (core, models)
  ├── intelligence-telemetry (core, Runtime context)
  └── intelligence-coordinator (all intelligence, Brain, Memory, Execution)
```

### Dependency Table

| Crate | Depends On | Layer | Purpose |
|---|---|---|---|
| `intelligence-core` | `ai-os-core`, `memory-core`, `ai-os-runtime` | Core, Memory, Runtime | Foundation types, events, errors, traits |
| `intelligence-providers` | `intelligence-core`, `ai-os-core`, `ai-os-runtime` | Core, Runtime | Provider adapters, health, lifecycle |
| `intelligence-models` | `intelligence-core`, `intelligence-providers` | Intelligence | Model registry, capability discovery |
| `intelligence-router` | `intelligence-core`, `intelligence-models`, `intelligence-providers`, `intelligence-cache`, `intelligence-cost` | Intelligence | Model selection, failover routing |
| `intelligence-prompts` | `intelligence-core` | Intelligence | Template management, rendering |
| `intelligence-context` | `intelligence-core`, `ai-os-memory-core`, `ai-os-perception-core` | Memory, Perception | Context assembly, retrieval |
| `intelligence-embeddings` | `intelligence-core`, `intelligence-models` | Intelligence | Embedding generation, vector ops |
| `intelligence-cache` | `intelligence-core`, `ai-os-memory-storage` | Memory | Response cache, semantic dedup |
| `intelligence-streaming` | `intelligence-core` | Intelligence | Streaming chunk aggregation |
| `intelligence-tools` | `intelligence-core`, `execution-core` | Execution | Tool-calling abstraction |
| `intelligence-cost` | `intelligence-core`, `intelligence-telemetry` | Intelligence | Token accounting, budget enforcement |
| `intelligence-safety` | `intelligence-core`, `intelligence-models` | Intelligence | Safety classifiers, content filtering |
| `intelligence-telemetry` | `intelligence-core`, `ai-os-runtime` | Runtime | Metrics, tracing, audit logging |
| `intelligence-coordinator` | All intelligence crates, `ai-os-brain-core`, `ai-os-core`, `ai-os-memory-core`, `execution-core` | All | Top-level orchestration |

---

## Core Data Model

### Identity Types

```rust
pub struct ProviderId(uuid::Uuid);      // Unique provider instance
pub struct ModelId(uuid::Uuid);         // Unique model endpoint
pub struct CapabilityId(uuid::Uuid);    // Unique capability declaration
pub struct PromptId(uuid::Uuid);        // Unique prompt template
pub struct ConversationId(uuid::Uuid);  // Unique conversation
pub struct RequestId(uuid::Uuid);       // Unique inference request
pub struct EmbeddingId(uuid::Uuid);     // Unique embedding record
pub struct SafetyRuleId(uuid::Uuid);    // Unique safety rule
pub struct CostAccountId(uuid::Uuid);   // Unique cost/usage record
```

### Model Capability

```rust
pub struct ModelCapability {
    pub id: CapabilityId,
    pub name: String,
    pub kind: CapabilityKind,
    pub input_modalities: Vec<Modality>,
    pub output_modalities: Vec<Modality>,
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub context_window: u32,
}
```

```rust
pub enum CapabilityKind {
    Chat,               // Conversational LLM
    Embedding,          // Vector embedding model
    Classification,     // Text/image classification
    Generation,         // Creative text generation
    CodeGeneration,     // Code synthesis
    Summarization,      // Text summarization
    Transcription,      // Audio → text
    Translation,        // Cross-language translation
    SafetyClassification, // Harm detection, PII detection
    Custom(String),
}
```

### Model Info

```rust
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
```

```rust
pub struct ModelPricing {
    pub currency: String,
    pub input_per_million_tokens: f64,
    pub output_per_million_tokens: f64,
    pub minimum_charge: Option<f64>,
    pub free_tier_tokens: Option<u32>,
}
```

```rust
pub enum AvailabilityStatus {
    Available,
    Degraded { reason: String },
    Unavailable { reason: String },
    Maintenance { until: Timestamp },
}
```

### Model Request and Response

```rust
pub struct ModelRequest {
    pub request_id: RequestId,
    pub model_id: Option<ModelId>,
    pub required_capabilities: Vec<CapabilityKind>,
    pub input: ModelInput,
    pub parameters: GenerationParameters,
    pub constraints: RequestConstraints,
    pub routing_hints: RoutingHints,
    pub conversation_id: Option<ConversationId>,
    pub trace_context: TraceContext,
}

pub enum ModelInput {
    Text(String),
    Messages(Vec<Message>),
    Images(Vec<EmbeddedImage>),
    Audio(EmbeddedAudio),
    Multimodal(Vec<ContentPart>),
}

pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub name: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
    pub timestamp: Timestamp,
}

pub enum MessageRole {
    System, User, Assistant, Tool, Function,
}

pub struct GenerationParameters {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub seed: Option<u64>,
    pub response_format: Option<ResponseFormat>,
}

pub enum ResponseFormat {
    Text,
    Json,
    StructuredJson { schema: serde_json::Value },
}

pub struct RequestConstraints {
    pub max_latency_ms: Option<u64>,
    pub max_cost_cents: Option<f64>,
    pub required_safety_level: SafetyLevel,
    pub max_retries: u32,
    pub fallback_models: Vec<ModelId>,
}

pub struct RoutingHints {
    pub preferred_providers: Vec<ProviderId>,
    pub excluded_providers: Vec<ProviderId>,
    pub preferred_region: Option<String>,
    pub prefer_low_cost: bool,
    pub prefer_low_latency: bool,
}

pub struct ModelResponse {
    pub request_id: RequestId,
    pub model_id: ModelId,
    pub provider_id: ProviderId,
    pub content: String,
    pub structured_data: Option<serde_json::Value>,
    pub usage: TokenUsage,
    pub finish_reason: FinishReason,
    pub tool_calls: Vec<ToolCall>,
    pub safety_events: Vec<SafetyEvent>,
    pub latency_ms: u64,
    pub cached: bool,
}

pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub estimated_cost_cents: f64,
}

pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    SafetyStop,
    Error { reason: String },
}
```

### Prompt Template

```rust
pub struct PromptTemplate {
    pub template_id: PromptId,
    pub name: String,
    pub version: String,
    pub template_text: String,
    pub variables: Vec<PromptVariable>,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
    pub tags: Vec<String>,
    pub safety_rules: Vec<SafetyRuleId>,
}

pub struct PromptVariable {
    pub name: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub description: String,
}
```

### Conversation and Session

```rust
pub struct Conversation {
    pub conversation_id: ConversationId,
    pub session_id: String,
    pub model_id: Option<ModelId>,
    pub messages: Vec<Message>,
    pub system_prompt: Option<String>,
    pub parameters: GenerationParameters,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub metadata: HashMap<String, String>,
}
```

### Embedding

```rust
pub struct Embedding {
    pub embedding_id: EmbeddingId,
    pub model_id: ModelId,
    pub source_text: String,
    pub vector: Vec<f32>,
    pub dimensions: u32,
    pub created_at: Timestamp,
    pub token_count: u32,
}
```

### Safety

```rust
pub struct SafetyRule {
    pub rule_id: SafetyRuleId,
    pub name: String,
    pub category: SafetyCategory,
    pub action: SafetyAction,
    pub severity: SafetySeverity,
    pub enabled: bool,
}

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

pub enum SafetyAction {
    Block,
    Redact,
    Warn,
    Log,
    Sanitize,
}

pub enum SafetySeverity {
    Low, Medium, High, Critical,
}

pub struct SafetyEvent {
    pub rule_id: SafetyRuleId,
    pub category: SafetyCategory,
    pub severity: SafetySeverity,
    pub action: SafetyAction,
    pub matched_text: Option<String>,
    pub position: Option<TextRange>,
}

pub enum SafetyLevel {
    None,       // No safety checks
    Standard,   // Default checks (toxicity, jailbreak)
    Strict,     // All checks including PII redaction
    Custom(Vec<SafetyRuleId>),
}
```

### Cost and Telemetry

```rust
pub struct CostAccount {
    pub account_id: CostAccountId,
    pub request_id: RequestId,
    pub provider_id: ProviderId,
    pub model_id: ModelId,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub input_cost_cents: f64,
    pub output_cost_cents: f64,
    pub total_cost_cents: f64,
    pub currency: String,
    pub timestamp: Timestamp,
}

pub struct TelemetryRecord {
    pub request_id: RequestId,
    pub provider_id: ProviderId,
    pub model_id: ModelId,
    pub latency_ms: u64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_cents: f64,
    pub cached: bool,
    pub fallback_used: bool,
    pub safety_events: u32,
    pub timestamp: Timestamp,
}
```

---

## Provider Registry

The Provider Registry manages the lifecycle of AI service providers. Each provider is loaded as a plugin implementing `ModelProvider`. The registry handles registration, health checking, and graceful degradation.

```rust
pub struct ProviderInfo {
    pub provider_id: ProviderId,
    pub name: String,
    pub kind: ProviderKind,
    pub api_base: String,
    pub auth_method: AuthMethod,
    pub rate_limit: Option<RateLimit>,
    pub region: String,
    pub status: ProviderStatus,
    pub registered_at: Timestamp,
    pub last_health_check: Timestamp,
}

pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Google,
    Cohere,
    Mistral,
    LocalLLM { endpoint: String },
    Custom(String),
}

pub enum AuthMethod {
    ApiKey { key_ref: String },
    OAuth { client_id: String },
    BearerToken { token_ref: String },
    None,
}

pub struct RateLimit {
    pub requests_per_minute: u32,
    pub tokens_per_minute: u32,
    pub concurrent_requests: u32,
}

pub enum ProviderStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
    Disabled,
}
```

### Provider Lifecycle

```
Registered → Initializing → Healthy → (cycle)
                                   └── Degraded → Recovering → Healthy
                                   └── Unhealthy → CircuitOpen → (backoff retry)
                                                      └── RetrySucceeded → Healthy
                                                      └── RetryExhausted → Disabled
```

---

## Model Registry

The Model Registry tracks all available models across providers. Models are discovered via provider metadata or registered manually. Each model declares its capabilities, pricing, and constraints.

```rust
pub struct ModelRegistration {
    pub model_id: ModelId,
    pub provider_id: ProviderId,
    pub external_model_name: String,
    pub capabilities: Vec<ModelCapability>,
    pub pricing: ModelPricing,
    pub context_window: u32,
    pub tags: Vec<String>,
}

pub struct ModelQuery {
    pub capability: Option<CapabilityKind>,
    pub max_input_tokens: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub max_latency_ms: Option<u64>,
    pub max_cost_cents: Option<f64>,
    pub available_only: bool,
    pub tags: Vec<String>,
}

pub enum ModelSortKey {
    LatencyP50Asc,
    CostPer1kTokensAsc,
    ContextWindowDesc,
    AvailabilityDesc,
    Custom(String),
}
```

---

## Model Routing

The Router selects the optimal model for each request based on a scoring function:

```
score(model, request) = w1 * capability_fit + w2 * latency_score + w3 * cost_score + w4 * availability_score
```

Where:
- `capability_fit`: 1.0 if all required capabilities are present, 0.0 otherwise
- `latency_score`: 1.0 - (model.latency_p99 / request.max_latency_ms), clamped to [0, 1]
- `cost_score`: 1.0 - (model.output_price / request.max_cost_cents), clamped to [0, 1]
- `availability_score`: 1.0 for Available, 0.5 for Degraded, 0.0 for Unavailable

The router maintains a sorted list of candidate models by score. On failure, it falls back to the next candidate in the list (configurable via `fallback_models` in `RequestConstraints`).

### Routing Policies

```rust
pub enum RoutingPolicy {
    BestCapability,   // Maximize capability fit, ignore cost/latency
    LowestCost,       // Minimize cost, accept any capable model
    LowestLatency,    // Minimize latency, accept any capable model
    Balanced {        // Weighted scoring (default)
        capability_weight: f64,
        latency_weight: f64,
        cost_weight: f64,
        availability_weight: f64,
    },
}
```

---

## Prompt Rendering

The Prompt subsystem manages prompt templates and renders them with variable substitution, conditional sections, and Safety Rule injection.

```rust
pub struct RenderedPrompt {
    pub template_id: PromptId,
    pub rendered_text: String,
    pub variables_used: HashMap<String, String>,
    pub safety_rules_applied: Vec<SafetyRuleId>,
    pub token_estimate: u32,
}

pub enum TemplateLanguage {
    Mustache,          // {{variable}} syntax
    Handlebars,        // {{#if}} {{#each}} etc.
    Jinja2,            // {% for %} {{ var }} etc.
    Simple,            // {variable} syntax
}
```

### Rendering Pipeline

```
Template Load → Variable Resolution → Conditional Evaluation → Safety Rule Injection → Token Estimation → Rendered Prompt
```

- **Variable Resolution**: Variables come from `ModelRequest.input`, `Conversation.messages`, `ExecutionContext`, and Memory retrieval results.
- **Conditional Sections**: Templates support `{{#if condition}}...{{/if}}` blocks that include or skip sections based on runtime conditions.
- **Safety Rule Injection**: System-level safety instructions are prepended to the rendered prompt according to the request's `SafetyLevel`.

---

## Context Construction

The Context subsystem assembles the full context window for a model request from multiple sources:

```
┌──────────────────────────────────────────────────────┐
│                    Context Assembler                   │
│                                                      │
│  ┌─────────────┐  ┌─────────────┐  ┌────────────────┐│
│  │ Conversation│  │    Memory   │  │  Perception    ││
│  │   Messages  │  │  Retrieved  │  │  Observations  ││
│  └──────┬──────┘  └──────┬──────┘  └──────┬─────────┘│
│         │                │                │           │
│         └────────────────┼────────────────┘           │
│                          v                           │
│               ┌─────────────────────┐                 │
│               │   Context Truncator │ ← max_tokens   │
│               └─────────┬───────────┘                 │
│                         v                             │
│               ┌─────────────────────┐                 │
│               │   ContextWindow     │                 │
│               │  (final assembled)  │                 │
│               └─────────────────────┘                 │
└──────────────────────────────────────────────────────┘
```

### Context Sources

| Source | Provides | Retrieval Method |
|---|---|---|
| Conversation | Prior messages in current conversation | ConversationStore (Memory Platform) |
| Memory | Relevant episodic/semantic memories | Vector similarity search (Embedding → ANN) |
| Perception | Recent observations from Perception Platform | EventBus recent events, Memory retrieval |
| System | System prompt, safety rules | Prompt template system |
| Tools | Available tool definitions | Execution Platform ToolRegistry |

### Truncation Strategies

| Strategy | Behavior |
|---|---|
| `SlidingWindow` | Keep most recent N messages, drop oldest |
| `Summarize` | Summarize older messages using a secondary model call |
| `Relevance` | Rank messages by relevance to current input, keep top N |

---

## Streaming Pipeline

When `GenerationParameters` requests streaming, the response flows through the streaming pipeline:

```
Provider response chunk → StreamingDecoder → ChunkAggregator
    → [optional: tool-call chunk assembly]
    → [required: safety check per chunk]
    → SSE/ChannelSender → Consumer
```

```rust
pub enum StreamChunk {
    TextDelta(String),
    ToolCallDelta(ToolCallDelta),
    SafetyEvent(SafetyEvent),
    UsageUpdate(TokenUsage),
    Done(ModelResponseMetadata),
    Error(StreamError),
}

pub struct StreamHandler {
    pub buffer: Vec<StreamChunk>,
    pub tool_call_buffers: HashMap<String, ToolCallDelta>,
    pub safety_events: Vec<SafetyEvent>,
    pub usage: TokenUsage,
}
```

---

## Tool Calling

The Tool subsystem provides a provider-agnostic tool-calling abstraction. Tools are declared using the `ToolDefinition` schema (shared with Execution Platform), and the Intelligence Platform translates them into provider-specific formats (OpenAI function calling, Anthropic tool use, etc.).

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub required_params: Vec<String>,
    pub output_schema: Option<serde_json::Value>,
}

pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub enum ToolCallResult {
    Success { result: serde_json::Value },
    Error { error: String },
    Pending { tool_call_id: String },
}
```

---

## Routing Hints

```rust
pub struct RoutingHints {
    pub preferred_providers: Vec<ProviderId>,
    pub excluded_providers: Vec<ProviderId>,
    pub preferred_region: Option<String>,
    pub prefer_low_cost: bool,
    pub prefer_low_latency: bool,
}
```

---

## Safety Pipeline

The Safety subsystem applies rules in a configurable order. Each rule can block, redact, warn, log, or sanitize content.

```
Input → PII Detection → Prompt Injection Detection → Jailbreak Detection → Toxicity Detection → SafetyDecision
                                                                                                            │
                    ┌───────────────────────────────────────────────────────────┴────────────────────┐
                    │                                                                               │
              Block/Reject                                                                       Pass
                    │                                                                               │
                    v                                                                               v
            SafetyViolation event                                                           Continue to Model
           Sanitized/Redacted output
```

### Safety Enforcement Levels

| Level | PII | Injection | Jailbreak | Toxicity |
|---|---|---|---|---|
| `None` | off | off | off | off |
| `Standard` | warn | block | block | warn |
| `Strict` | redact | block | block | block |
| `Custom` | per-rule | per-rule | per-rule | per-rule |

---

## Cost Management

The Cost subsystem tracks token usage and accrues costs per-request, per-conversation, and globally.

```rust
pub struct CostBudget {
    pub max_cents_per_request: Option<f64>,
    pub max_cents_per_conversation: Option<f64>,
    pub max_cents_per_day: f64,
    pub alert_threshold_cents: f64,
}

pub struct CostReport {
    pub period: CostPeriod,
    pub total_requests: u64,
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub total_cost_cents: f64,
    pub by_provider: HashMap<ProviderId, ProviderCost>,
    pub by_model: HashMap<ModelId, ModelCost>,
}

pub struct ProviderCost {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_cents: f64,
}

pub enum CostPeriod {
    Hour,
    Day,
    Week,
    Month,
}
```

---

## Caching

The Cache subsystem stores `ModelResponse` objects keyed by a hash of the request input and parameters. Cached responses are returned without calling the provider, eliminating latency and cost.

```rust
pub struct CacheEntry {
    pub request_hash: String,
    pub response: ModelResponse,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub hit_count: u32,
    pub similarity_threshold: f64,
}
```

**Cache key**: SHA-256 of `(model_id + capability_kind + normalized_input + generation_parameters)`.

**TTL configurable** per-model or global default. Cached entries are invalidated when the underlying model is updated or the provider is marked unhealthy.

**Semantic dedup**: For embedding-capable models, the cache can match requests against prior inputs using cosine similarity. Entries with similarity > threshold (default: 0.95) are served from cache.

---

## Threading Model

| Component | Concurrency Model |
|---|---|
| `intelligence-providers` | `RwLock<HashMap<ProviderId, ProviderEntry>>` for registry. Provider health checks run on `tokio::time::interval`. |
| `intelligence-models` | `RwLock<HashMap<ModelId, ModelInfo>>` for registry. Refresh on `tokio::time::interval`. |
| `intelligence-router` | `RwLock<Vec<ModelInfo>>` for sorted candidates. Selection is synchronous (cache-backed). |
| `intelligence-prompts` | Read-heavy. Template storage in `DashMap<String, PromptTemplate>`. Rendering is synchronous. |
| `intelligence-context` | Retrieval is async (Memory queries). Context assembly is synchronous. |
| `intelligence-embeddings` | `Semaphore` for concurrent embedding requests to provider. |
| `intelligence-cache` | `DashMap<String, CacheEntry>` for hot path. Async eviction on `tokio::time::interval`. |
| `intelligence-streaming` | Per-request `mpsc::Sender<StreamChunk>`. Aggregation on `tokio::task::spawn`. |
| `intelligence-tools` | `DashMap<String, ToolDefinition>` for tool lookup. |
| `intelligence-cost` | `RwLock<CostAccount>` per-request. Daily budget check on interval. Async journal to Memory. |
| `intelligence-safety` | `RwLock<Vec<SafetyRule>>`. Rule evaluation is synchronous (regex/AST). |
| `intelligence-telemetry` | `mpsc::Sender<TelemetryRecord>` to async writer task. |
| `intelligence-coordinator` | Single coordinator task. All pipelines wired via bounded `mpsc` channels. `Arc<dyn Trait>` composition. |

---

## Lifecycle

### Init Phase
1. Load configuration from `intelligence.toml`.
2. Initialize Provider Registry (register configured providers with health check endpoints).
3. Initialize Model Registry (discover models from providers or load from config).
4. Initialize Cache (load eviction policy, restore from Memory if warm-start enabled).
5. Initialize Safety module (load safety rules per configured SafetyLevel).
6. Initialize Cost module (load budgets, start tracking).
7. Create bounded `mpsc` channels between pipeline stages.
8. Register EventBus subscriptions.

### Start Phase
1. Start provider health check loops.
2. Start model refresh loop (periodic capability re-discovery).
3. Start cache eviction loop.
4. Start cost budget monitor.
5. Publish `intelligence.coordinator_ready` event.

### Stop Phase
1. Pause provider health checks.
2. Drain in-flight requests (send graceful shutdown to providers, wait for completions or timeout).
3. Flush cost/telemetry journals to Memory.
4. Flush cache to persistent store.
5. Publish `intelligence.coordinator_stopped`.

---

## Error Model

| Category | Variants | Severity | Recovery |
|---|---|---|---|
| **Provider errors** | `ConnectionFailed`, `AuthenticationFailed`, `RateLimited`, `Timeout`, `ServerError`, `InvalidResponse` | Error | Retry with fallback provider/model |
| **Model errors** | `ModelNotFound`, `CapabilityMismatch`, `ContextWindowExceeded`, `ContentFiltered`, `SafetyViolation` | Error | Fallback to alternate model or escalate |
| **Routing errors** | `NoCapableModel`, `AllProvidersUnavailable`, `NoFallbackAvailable` | Error | Escalate to Brain |
| **Prompt errors** | `TemplateNotFound`, `RenderError`, `VariableMissing`, `ContextTooLong` | Error | Return error to caller |
| **Safety errors** | `SafetyRuleViolation { rule, category }`, `SafetyFrameworkError` | Warning | Block response, sanitize, or escalate |
| **Cost errors** | `BudgetExceeded`, `QuotaExceeded` | Warning | Block request, notify operator |
| **Cache errors** | `CacheWriteFailed`, `CacheCorrupted` | Warning | Log, continue without cache |
| **Telemetry errors** | `MetricsWriteFailed` | Warning | Log, do not block request |

---

## Security Considerations

1. **API Key Management** — Provider API keys are loaded from environment variables or a secrets vault. They are never stored in configuration files or logs. Keys are referenced by name (`OPENAI_API_KEY`) and loaded at runtime.

2. **PII Redaction** — At `Strict` safety level, all PII (emails, phone numbers, SSNs, credit cards) in both requests and responses is detected and redacted before logging or storage. Redaction is implemented via regex patterns and local classifiers — no data leaves the system for PII checking.

3. **Prompt Injection Defense** — All user-supplied content in the context window is scanned for prompt-injection patterns (system prompt extraction, role manipulation, instruction override). Detected injections are blocked or redacted.

4. **Audit Trail** — Every inference request is logged with: provider, model, input hash, token count, cost, latency, safety events, and outcome. Logs are written to Memory (episodic store) and to the structured logger.

5. **Request Isolation** — Requests from different conversations/sessions are isolated. Context from one session is never injected into another without explicit authorization.

6. **Cost Limits** — Per-request, per-conversation, and global daily cost limits are enforced before every model call. Requests exceeding limits are rejected with `BudgetExceeded` before any provider is called.

---

## Performance Targets

| Metric | Target | Notes |
|---|---|---|
| Router selection | <1ms | In-memory scoring of cached model list |
| Prompt rendering | <5ms | Template + variable substitution |
| Context assembly | <10ms | Memory retrieval + truncation |
| Safety check (per-request) | <3ms | Regex + local classifier |
| Total overhead (non-provider) | <20ms | Cumulative before/after provider call |
| Cache lookup | <0.1ms | In-memory DashMap |
| Embedding generation | <50ms | Local or embedding model call |
| Streaming first-token (TTFT) | ≤ provider TTFT | Intelligence overhead: <5ms |
| Cost accrual | <0.1ms | Atomic counter increment |

---

## Crate Responsibilities Matrix

| Crate | Responsibility | Key Types | Consumers |
|---|---|---|---|
| `intelligence-core` | Foundation types, errors, events, base traits | `ModelRequest`, `ModelResponse`, `ModelCapability`, `ProviderId`, `ModelId`, `ModelProvider`, `ModelRegistry`, `ModelRouter`, `PromptRenderer`, `ContextAssembler`, `EmbeddingGenerator`, `SafetyEnforcer`, `CostTracker`, `TelemetrySink` | All other intelligence crates |
| `intelligence-providers` | Provider adapters, registration, lifecycle | `ProviderRegistry`, `ProviderEntry`, `ProviderHealth`, `OpenAIProvider`, `AnthropicProvider`, `LocalProvider` | Router, Models, Telemetry |
| `intelligence-models` | Model registry, capability discovery, metadata | `ModelRegistry`, `ModelQuery`, `ModelSortKey`, `CapabilityMatcher` | Router, Coordinator, Context |
| `intelligence-router` | Model selection, failover, routing policies | `ModelRouter`, `RoutingPolicy`, `RoutingDecision`, `FallbackChain` | Coordinator |
| `intelligence-prompts` | Template management, rendering | `PromptStore`, `PromptRenderer`, `RenderedPrompt`, `PromptVariable` | Coordinator, Context |
| `intelligence-context` | Context assembly from Memory + Perception | `ContextAssembler`, `ContextTruncator`, `ConversationStore`, `MemoryRetriever` | Coordinator |
| `intelligence-embeddings` | Embedding generation, similarity | `EmbeddingGenerator`, `EmbeddingStore`, `SimilaritySearch` | Cache, Context, Router |
| `intelligence-cache` | Response caching, TTL, semantic dedup | `ResponseCache`, `CacheEntry`, `CacheKey`, `SemanticDedup` | Router |
| `intelligence-streaming` | Streaming pipeline, chunk aggregation | `StreamDecoder`, `StreamHandler`, `StreamChunk` | Coordinator (streaming mode) |
| `intelligence-tools` | Tool-calling abstraction, schema | `ToolRegistry`, `ToolDefinition`, `ToolCall`, `ToolResult` | Coordinator, Context |
| `intelligence-cost` | Token accounting, budget enforcement | `CostTracker`, `CostBudget`, `CostReport` | Router, Coordinator |
| `intelligence-safety` | Safety classifiers, content filtering | `SafetyPipeline`, `SafetyRule`, `SafetyEnforcer`, `PiiDetector`, `JailbreakDetector` | Router |
| `intelligence-telemetry` | Metrics, tracing, audit logging | `TelemetryCollector`, `RequestLogger`, `MetricsExporter` | All intelligence crates |
| `intelligence-coordinator` | Top-level orchestration, pipeline, lifecycle | `IntelligenceCoordinator`, `PipelineManager`, `CoordinatorStats` | Brain Platform |

---

## References

- [Core Platform Architecture](../architecture/core.md)
- [Runtime Platform Architecture](../architecture/runtime.md)
- [Memory Platform Architecture](../architecture/memory.md)
- [Brain Platform Architecture](../architecture/brain.md)
- [Perception Platform Architecture](../architecture/perception.md)
- [Execution Platform Architecture](../architecture/execution.md)
- [OSAL Architecture](../architecture/osal.md)
- [Intelligence Interfaces](../interfaces/intelligence.md)
- [Intelligence Configuration](../configuration/intelligence.md)
- [Intelligence Pipeline Design](../intelligence-pipeline.md)
- RFC-0001: System Platform
- RFC-0002: Memory Platform
- RFC-0003: Brain Platform
- RFC-0004: Perception Platform
- RFC-0005: Execution Platform
