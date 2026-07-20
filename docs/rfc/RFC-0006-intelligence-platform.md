# RFC-0006: Intelligence Platform

| Field | Value |
|---|---|
| **Status** | Draft |
| **Author** | AI-OS Architecture Team |
| **Phase** | 9 |
| **Created** | 2026-07-20 |
| **Updated** | 2026-07-20 |
| **Requires** | RFC-0001 (OSAL), RFC-0002 (Memory), RFC-0003 (Brain), RFC-0004 (Perception), RFC-0005 (Execution) |
| **Supersedes** | None |

## Abstract

The Intelligence Platform (Phase 9) adds AI cognition to the AI-native OS. It provides a provider-agnostic abstraction over all AI models and services, so the Brain Platform can request chat, embedding, classification, and other AI capabilities without importing any provider SDK. The platform includes model routing (capability-based, cost-aware, latency-aware), prompt template rendering, context assembly from Memory and Perception, response caching, streaming, a provider plugin system, tool-calling abstraction, token accounting and cost tracking, a multi-tier safety pipeline, and comprehensive telemetry. The Brain Platform never imports an OpenAI, Anthropic, Google, or Cohere SDK. Adding a new AI provider is a plugin registration: zero changes to Brain or any other consumer.

## Motivation

The Brain Platform (Phase 6) is a structured reasoning engine. It produces decision requests that need AI-generated content: text completions, embeddings, classifications, and tool-call generation. Without an Intelligence Platform:

1. **The Brain would import provider SDKs directly** — `brain-chat` would import `openai`, `brain-embed` would import `cohere`. Every new provider forces changes across every Brain sub-crate.
2. **No cost visibility** — Token usage and cost would be opaque. A single runaway conversation could exhaust a budget without warning.
3. **No safety guarantees** — Each provider's safety is different. Some have no built-in safety. A unified safety pipeline is needed.
4. **No caching** — Repeated identical requests would hit providers on every call, adding cost and latency.
5. **No fallback** — A provider outage would brick the entire Brain. A fallback chain to alternative providers/models is essential.
6. **Tight coupling** — The only way to swap models would be to change code. We need runtime routing based on capabilities, cost, latency, and availability.

The Intelligence Platform solves all of these by sitting between the Brain and all AI providers.

### Specification Cross-References

- `specification.md` section 7 (AI Lifecycle): The Intelligence Platform provides the Act stage for AI-generated output.
- `specification.md` section 4 (Security Goals): Safety pipeline; PII redaction; audit trail.
- `specification.md` section 12 (Module Contracts): Service trait, EventBus, HealthMonitor.
- [Intelligence Architecture](../architecture/intelligence.md)
- [Intelligence Interfaces](../interfaces/intelligence.md)
- [Intelligence Configuration](../configuration/intelligence.md)
- [Intelligence Pipeline Design](../intelligence-pipeline.md)

## Design

### Architecture Overview

```
brain/                        (Layer 9 — Primary consumer)
  |
  v
intelligence/                 (Layer 8 — This RFC)
  |
  ├── intelligence-providers/  Provider adapters (OpenAI, Anthropic, Google, Local)
  ├── intelligence-models/     Model registry, capability discovery
  ├── intelligence-router/     Capability/latency/cost-based routing
  ├── intelligence-prompts/    Prompt templates, rendering
  ├── intelligence-context/    Context from Memory + Perception + Conversation
  ├── intelligence-embeddings/ Embedding generation, similarity
  ├── intelligence-cache/      Response caching, semantic dedup
  ├── intelligence-streaming/  Streaming response pipeline
  ├── intelligence-tools/      Tool-calling abstraction
  ├── intelligence-cost/       Token accounting, budget enforcement
  ├── intelligence-safety/     Safety classifiers, content filtering
  ├── intelligence-telemetry/  Metrics, tracing, audit logging
  └── intelligence-coordinator/ Top-level orchestration
```

### Crate Structure

```
intelligence/
  +-- intelligence-core/          Foundation: ModelRequest, ModelResponse, events,
  |                                    errors, 12 base traits
  +-- intelligence-providers/     Provider registry: OpenAI, Anthropic, Google,
  |                                    Cohere, LocalLLM, Custom plugins
  +-- intelligence-models/        Model registry, capability discovery,
  |                                    ModelQuery, ModelInfo, ModelPricing
  +-- intelligence-router/        Model selection by score, fallback chain,
  |                                    RoutingPolicy (Balanced, LowestCost, etc.)
  +-- intelligence-prompts/       Prompt templates with Mustache/Handlebars/
  |                                    Jinja2/Simple rendering, variable substitution
  +-- intelligence-context/       Context assembly from Conversation (Memory),
  |                                    Episodic/Semantic memories, Perception
  |                                    observations, ToolDefinitions
  +-- intelligence-embeddings/    Embedding generation, cosine similarity,
  |                                    vector search (ANN)
  +-- intelligence-cache/         Response cache keyed by (model, input, params)
  |                                    SHA-256 key, TTL, LRU eviction,
  |                                    semantic dedup via embedding similarity
  +-- intelligence-streaming/     SSE/chunk streaming, StreamDecoder,
  |                                    tool-call delta aggregation
  +-- intelligence-tools/         Tool-calling abstraction, ToolDefinition,
  |                                    provider-format translation, schema validation
  +-- intelligence-cost/          Token accounting, CostTracker, CostBudget,
  |                                    per-request/conversation/daily budgets
  +-- intelligence-safety/        SafetyPipeline: PII, injection, jailbreak,
  |                                    toxicity detection; SafetyRule engine
  +-- intelligence-telemetry/     TelemetrySink, MetricsSnapshot, RequestLogger,
  |                                    ProviderMetrics, audit trail to Memory
  +-- intelligence-coordinator/   Top-level orchestration: request routing,
                                    pipeline assembly, Service lifecycle, EventBus
```

### Provider Plugin System

A `ModelProvider` is an `Arc<dyn ModelProvider>` loaded at runtime. The provider registry holds all registered providers. Provider adapters translate between platform-native `ModelRequest` / `ModelResponse` and provider-native message formats.

**Adding a new provider requires**:
1. Implementing `ModelProvider` trait.
2. Registering in configuration (`[intelligence.providers.<name>]`).
3. No changes to any other crate.

### Model Routing

Routing is a scoring function over candidate models:

```
score(model, request) = w1 * capability_fit + w2 * latency_score + w3 * cost_score + w4 * availability_score
```

Four built-in policies:
- **BestCapability**: Maximize capability fit, ignore cost/latency.
- **LowestCost**: Minimize cost, accept any capable model.
- **LowestLatency**: Minimize latency, accept any capable model.
- **Balanced**: Weighted scoring (default: capability 0.5, latency 0.3, cost 0.2, availability 0.0).

On failure at any stage, the router advances to the next fallback in the pre-computed fallback chain. Max fallbacks: 3.

### Safety Pipeline

Safety checks run at two points:
1. **Input check** (Stage 3): Before any model call — PII detection, prompt injection detection, jailbreak detection.
2. **Output check** (Stage 6): After model response — toxicity, PII in output, bias.

Four severity levels: `None`, `Standard`, `Strict`, `Custom`.

Six rule categories: `Pii`, `PromptInjection`, `Jailbreak`, `Toxicity`, `Bias`, `SexualContent`, `Violence`.

Four actions per rule: `Block`, `Redact`, `Warn`, `Log`, `Sanitize`.

All safety decisions emit `intelligence.safety.*` events and are persisted in the Memory Platform.

### Caching

Responses are cached by SHA-256 of `(model_id + capability + normalized_input + parameters)`. Cache TTL is per-model or global (default: 1 hour). When an embedding model is configured, semantic dedup matches near-duplicate requests (cosine similarity > 0.95) and serves cached responses.

### Events

#### Published Events

| Event Type | Payload | Stage | Description |
|---|---|---|---|
| `intelligence.provider.registered` | `ProviderRegistered` | Init | New provider |
| `intelligence.provider.health_changed` | `ProviderHealthChanged` | Monitor | Provider status change |
| `intelligence.model.registered` | `ModelRegistered` | Init | New model discovered |
| `intelligence.request.started` | `RequestStarted` | Entry | Request submitted |
| `intelligence.request.completed` | `RequestCompleted` | Return | Successful response |
| `intelligence.request.failed` | `RequestFailed` | Failure | All fallbacks exhausted |
| `intelligence.request.fallback_used` | `FallbackUsed` | Router | Fell back to secondary |
| `intelligence.request.cached` | `RequestCached` | Router | Served from cache |
| `intelligence.safety.violation` | `SafetyViolation` | Safety | Rule triggered |
| `intelligence.safety.blocked` | `SafetyBlocked` | Safety | Request/response blocked |
| `intelligence.cost.budget_warning` | `BudgetWarning` | Cost | Approaching budget |
| `intelligence.cost.budget_exceeded` | `BudgetExceeded` | Cost | Budget limit hit |
| `intelligence.cache.hit` | `CacheHit` | Router | Cache hit |
| `intelligence.cache.miss` | `CacheMiss` | Router | Cache miss |
| `intelligence.stream.chunk` | `StreamChunk` | Streaming | Streaming chunk |
| `intelligence.stream.done` | `StreamDone` | Streaming | Stream complete |
| `intelligence.coordinator.ready` | `CoordinatorReady` | Lifecycle | Coordinator started |

#### Consumed Events

| Event Type | Source | Handler | Description |
|---|---|---|---|
| `core.lifecycle.stopping` | Core Platform | Coordinator | Graceful drain |
| `core.config.reloaded` | Core Platform | All modules | Hot-reload |
| `runtime.session_closed` | Runtime Platform | Context | End conversation |
| `memory.memory_stored` | Memory Platform | Context | New memory available |
| `brain.goal.updated` | Brain Platform | Coordinator | New inference goal |
| `execution.tool_result.ready` | Execution Platform | Context | Tool result for loop |
| `perception.observation.ready` | Perception Platform | Context | New observation |

### Crate Dependency Graph

```
intelligence-core
  ├── intelligence-providers    (core, Runtime, OSAL network)
  ├── intelligence-models       (core, providers)
  ├── intelligence-router       (core, models, providers, cache, cost)
  ├── intelligence-prompts      (core)
  ├── intelligence-context      (core, Memory, Perception)
  ├── intelligence-embeddings   (core, models)
  ├── intelligence-cache        (core, memory-storage)
  ├── intelligence-streaming    (core)
  ├── intelligence-tools        (core, execution-core)
  ├── intelligence-cost         (core, telemetry)
  ├── intelligence-safety       (core, models)
  ├── intelligence-telemetry    (core, Runtime)
  └── intelligence-coordinator  (all intelligence, brain-core, memory-core, execution-core)
```

### Configuration Model

| Section | Keys | Purpose |
|---|---|---|
| Root | `enabled`, `max_concurrent_requests`, `default_safety_level`, `default_routing_policy` | Global settings |
| Providers | `api_key_env`, `api_base`, `rate_limit_requests_per_min`, `timeout_ms` | Per-provider auth and limits |
| Models | `provider`, `capabilities`, `context_window`, `pricing`, `supports_streaming`, `supports_tools` | Per-model registration |
| Routing | `default_policy`, weights, `cache_enabled`, `fallback.max_fallbacks`, `backoff_*` | Model selection |
| Prompts | `default_template_language`, `templates_path`, `auto_register` | Template management |
| Context | `truncation_strategy`, `sliding_window_size`, `max_context_tokens`, `memory_retrieval_enabled`, `perception_integration_enabled` | Context assembly |
| Safety | `default_level`, `block_on_violation`, `log_all_checks`, `rules.*` | Safety enforcement |
| Cache | `ttl_seconds`, `max_entries`, `semantic_dedup_enabled`, `similarity_threshold` | Response caching |
| Cost | `tracking_enabled`, `budgets.per_request`, `budgets.per_conversation`, `budgets.global_daily` | Cost control |
| Streaming | `enabled`, `chunk_buffer_size`, `safety_check_per_chunk` | Streaming behavior |
| Telemetry | `enabled`, `export_interval_secs`, `storage_backend`, `metrics_retention_days` | Observability |

### Threading Model

| Component | Concurrency Model |
|---|---|
| Provider registry | `RwLock<HashMap>` + `tokio::time::interval` health checks |
| Model registry | `RwLock<HashMap>` + periodic refresh |
| Router | Synchronous scoring from cached sorted candidate list |
| Prompt renderer | Synchronous; `DashMap<String, PromptTemplate>` for hot lookup |
| Context assembler | Async Memory retrieval; synchronous truncation and assembly |
| Embeddings | `Semaphore` for concurrent provider calls |
| Cache | `DashMap<String, CacheEntry>`; async eviction via `tokio::time::interval` |
| Streaming | Per-request `mpsc::Sender<StreamChunk>` spawned on `tokio::task::spawn` |
| Tools | `DashMap<String, ToolDefinition>` read-heavy |
| Cost tracker | Atomic counters for request-level; async journal writer for persistence |
| Safety | `RwLock<Vec<SafetyRule>>`; synchronous regex/AST evaluation |
| Telemetry | `mpsc::Sender<TelemetryRecord>` to async batch writer task |
| Coordinator | Single task with bounded `mpsc` channels; `Arc<dyn Trait>` composition |

### Lifecycle

**Init**:
1. Load `intelligence.toml`.
2. Register providers from config; create health check tasks.
3. Discover models from providers; populate Model Registry.
4. Load prompt templates from `templates_path`; register built-in templates.
5. Initialize Cache; restore from Memory if warm-start enabled.
6. Load Safety Rules per `default_safety_level`.
7. Initialize Cost budgets; restore daily counter from Memory.
8. Create `mpsc` channels.
9. Register EventBus subscriptions.

**Start**:
1. Start provider health check loops.
2. Start model refresh loop.
3. Start cache eviction and telemetry export loops.
4. Publish `intelligence.coordinator_ready`.

**Stop**:
1. Pause health checks.
2. Drain in-flight requests (graceful, max wait 30s).
3. Flush telemetry journals to Memory.
4. Flush cost budgets to Memory.
5. Publish `intelligence.coordinator_stopped`.

### Security Considerations

1. **API Keys** — Provider API keys are referenced by env var name. Never stored in config or logs. Loaded via secrets vault or `std::env::var` at runtime.
2. **PII Redaction** — At `Strict` safety level, PII is detected and redacted in both requests and responses using local regex-classifier hybrids. No data leaves the platform for PII checking.
3. **Prompt Injection Defense** — User input is scanned for system prompt extraction, role manipulation, and instruction override patterns. Detected injections are blocked at Standard+ safety levels.
4. **Cost Isolation** — Per-request, per-conversation, and global daily budgets are enforced before every model call. Requests exceeding limits are rejected pre-call.
5. **Audit Trail** — Every inference request is logged with provider, model, token counts, cost, latency, safety events, and outcome. Logs are persisted in the Memory Platform.
6. **Provider Isolation** — Requests are routed to providers based on configuration. No provider has implicit trust. All provider responses are safety-checked before consumption.

### Performance Targets

| Operation | Target | Notes |
|---|---|---|
| Router selection | <1ms | In-memory scoring |
| Prompt rendering | <5ms | Template + variable substitution |
| Context assembly | <10ms | Memory retrieval + truncation |
| Safety input check | <3ms | Regex + local classifier |
| Safety output check | <3ms | Regex + local classifier |
| Total non-provider overhead | <20ms | Cumulative before/after provider call |
| Cache lookup | <0.1ms | DashMap |
| Cache store | <1ms | DashMap insert |
| Embedding generation | <50ms | Local or embedding model |
| Streaming first-token | < provider TTFT + <5ms | Intelligence overhead |
| Cost accrual | <0.1ms | Atomic counter increment |
| Telemetry record | <0.5ms | Async mpsc send |

### Testing Strategy

1. **Unit tests** — Every trait method with mock implementations. Scoring function correctness. Prompt rendering edge cases (missing vars, empty templates). Safety rule evaluation (positive and negative matches). Cost calculation accuracy.
2. **Integration tests** — End-to-end request through full pipeline with a mock provider. Cache hit/miss lifecycle. Streaming pipeline with mock chunks. Fallback chain traversal on provider failure.
3. **Property-based tests** — Prompt template rendering determinism. Cache key collision resistance (SHA-256). Routing score ordering. Context truncation preserves message order.
4. **Benchmarks** — Router scoring throughput, prompt rendering latency, safety check throughput, end-to-end latency with real providers, cache hit latency.
5. **Chaos tests** — Provider timeout triggers fallback chain. Provider returns malformed response triggers error handling. Cost budget exceeded mid-conversation blocks next request. Cache corruption is handled gracefully.

## Drawbacks

1. **Routing overhead** — The routing stage adds ~1-5ms per request. For low-latency use cases, direct provider calls would be faster. Mitigation: routing is in-memory and the score is pre-computed for hot paths.
2. **Caching complexity** — Cache invalidation is hard. Cached responses may become stale if the underlying model is updated. Mitigation: per-model TTL, automatic invalidation on model metadata update.
3. **Safety false positives** — Aggressive safety rules may block legitimate content. Mitigation: configurable per-rule severity and action; Standard level uses `Warn` rather than `Block` for low-severity categories.
4. **Provider SDK maintenance** — Each provider adapter must track the provider's API changes. Mitigation: abstraction layer means only the adapter changes, not the platform. Adding a new provider is a new adapter, not a platform change.

## Alternatives Considered

| Alternative | Reason for Rejection |
|---|---|
| Direct SDK imports in Brain | Violates clean architecture; no provider agnosticism; SDK changes break Brain |
| External proxy service (e.g., LiteLLM) | Introduces external dependency; loses EventBus integration; adds network hop |
| Monolithic `intelligence` crate | Violates single-responsibility; no independent testability; hard to evolve |
| No caching (always call provider) | Unacceptable cost and latency; no offline capability |
| Provider-managed safety only | Different providers have different safety levels; no uniform policy |
| Hard-coded model maps | Cannot adapt to new models without code changes; no dynamic routing |

## Open Questions

1. Should the Intelligence Platform support multi-modal inputs (images, audio) as first-class citizens? Current design supports them with `ModelInput::Multimodal`. Full multi-modal orchestration may add a dedicated modality router.
2. Should there be a standardized fine-tuned model registry? Current design inherits capabilities from base models. Fine-tuned model metadata should be extensible.
3. Should the platform support self-hosted local models via llama.cpp or Ollama? The `LocalLLM` provider kind exists; the actual adapter will be added when needed.
4. Should tool-calling be a Brain-level concern rather than an Intelligence Platform concern? The tool-calling abstraction is included here because it requires model capability negotiation and translation. This decision can be revisited if the Brain handles tool-calling at its own level.

## Implementation Plan

1. **Phase 1 — Core** (effort: medium)
   - `intelligence-core`: All foundation types, errors, events, 12 trait definitions.
   - Focus: type correctness, serialization, comprehensive tests.
   - Estimated: 3-4 sprints

2. **Phase 2 — Providers + Models** (effort: medium)
   - `intelligence-providers`: Provider registry, health checks, OpenAI + Anthropic adapters.
   - `intelligence-models`: Model registry, capability discovery, ModelQuery.
   - Integration tests with mock HTTP providers.
   - Estimated: 3-4 sprints

3. **Phase 3 — Router + Cache** (effort: medium)
   - `intelligence-router`: Model scoring, fallback chain, RoutingPolicy.
   - `intelligence-cache`: SHA-256 keying, TTL, LRU eviction, semantic dedup.
   - Benchmarks for routing throughput and cache hit latency.
   - Estimated: 2-3 sprints

4. **Phase 4 — Prompts + Context + Embeddings** (effort: medium)
   - `intelligence-prompts`: Template store, rendering, variable substitution.
   - `intelligence-context`: Conversation retrieval, Memory retrieval (vector ANN), Perception observation integration, truncation strategies.
   - `intelligence-embeddings`: Embedding generation, cosine similarity, vector search.
   - Integration tests with Memory Platform.
   - Estimated: 3-4 sprints

5. **Phase 5 — Streaming + Tools + Safety + Cost + Telemetry** (effort: large)
   - `intelligence-streaming`: StreamDecoder, chunk forwarding, tool-call aggregation.
   - `intelligence-tools`: ToolDefinition, provider-format translation.
   - `intelligence-safety`: SafetyPipeline, PII/injection/jailbreak/toxic detection.
   - `intelligence-cost`: Token accounting, budget enforcement.
   - `intelligence-telemetry`: Metrics, audit logging.
   - Estimated: 4-5 sprints

6. **Phase 6 — Coordinator** (effort: medium)
   - `intelligence-coordinator`: Pipeline wiring, Service trait, EventBus integration.
   - Integration with Brain Platform (`ModelRequest` → `ModelResponse`).
   - Chaos tests for provider failure, cache corruption, budget exhaustion.
   - Estimated: 2-3 sprints

## Unresolved Topics

- **Multi-modal orchestration** — Dedicated modality router for image/audio inputs. Post-initial implementation.
- **Fine-tuned model metadata** — Per-tenant model registries with fine-tune lineage. Deferred.
- **Local model adapters** — llama.cpp/Ollama adapters. Deferred until demand arises.
- **Usage-based model SLA** — Dynamic routing based on per-tenant SLA tiers. Deferred to Intelligence Integration phase.
- **Cross-node model distribution** — In multi-node deployments, model routing across nodes. Deferred.

## References

- [Intelligence Architecture](../architecture/intelligence.md)
- [Intelligence Interfaces](../interfaces/intelligence.md)
- [Intelligence Configuration](../configuration/intelligence.md)
- [Intelligence Pipeline Design](../intelligence-pipeline.md)
- [Brain Platform Architecture](../architecture/brain.md)
- [Memory Platform Architecture](../architecture/memory.md)
- [Perception Platform Architecture](../architecture/perception.md)
- [Execution Platform Architecture](../architecture/execution.md)
- [OSAL Architecture](../architecture/osal.md)
- [Core Platform Architecture](../architecture/core.md)
- [Runtime Platform Architecture](../architecture/runtime.md)
- RFC-0001: System Platform
- RFC-0002: Memory Platform
- RFC-0003: Brain Platform
- RFC-0004: Perception Platform
- RFC-0005: Execution Platform
