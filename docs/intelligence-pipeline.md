# Intelligence Processing Pipeline

## Overview

The Intelligence Pipeline is a request processing flow that transforms a Brain `ModelRequest` into a `ModelResponse`. It orchestrates routing, context assembly, safety checks, model invocation, caching, streaming, cost tracking, and telemetry. Each stage has a single responsibility and communicates through bounded asynchronous channels or synchronous function calls.

```
Brain → Coordinator → Router → Cache Lookup ──→ (cache hit) → Telemetry → Brain
                                      │
                               (cache miss) → Safety Input Check → Context Assembly
                                                              │
                                                        Provider Call / Stream
                                                              │
                                                        Safety Output Check → Cost Tracking
                                                              │
                                                        Cache Store → Telemetry → Brain
```

---

## Stage 1: Entry — Coordinator Receive

**Input**: `ModelRequest` from Brain (via EventBus `brain.intelligence.requested` or direct API call).

**Output**: Routed to `ModelRouter`.

**Behavior**:

1. The Coordinator receives the `ModelRequest` via the `IntelligenceCoordinator::request()` method.
2. It immediately increments the request counter and emits `intelligence.request.started` event.
3. It checks `max_concurrent_requests`. If at capacity, the request waits on a semaphore (bounded wait with timeout).
4. It calls the Cost module to check `BudgetCheck`. If over budget, the request is rejected with `BudgetExceeded` before any further processing.
5. The request is handed to the Router.

**Backpressure**: If `max_concurrent_requests` is exhausted:
- Critical (system) priority: Blocks until a slot opens (max wait: 30s).
- Normal priority: Queues with backoff (exponential, max 10s).
- Low/Background priority: Rejects immediately with `Busy`.

**Events emitted**: `intelligence.request.started`, `intelligence.cost.budget_warning` (if approaching limit)

---

## Stage 2: Routing — Model Selection

**Input**: `ModelRequest` from Coordinator.

**Output**: `RoutingDecision` with primary model and fallback chain.

**Behavior**:

1. The Router receives the request and queries the Model Registry for all models matching `required_capabilities`.
2. Candidates that are `Unavailable` or `Maintenance` are excluded (unless explicitly whitelisted).
3. The scoring function is evaluated for each candidate:

```
score(model, request) = w1 * capability_fit + w2 * latency_score + w3 * cost_score + w4 * availability_score
```

4. Candidates are sorted by score descending and the top N (default: 3) form the fallback chain.
5. The Cache module is checked with a cache key derived from `(model_id, capability, normalized_input, parameters)`.
6. If a cache entry is found with similarity above threshold, the cached `ModelResponse` is returned immediately (skip stages 3-7, go to Stage 9).

**Cache hit path**: `Cache Hit → Telemetry → Return to Brain`

**Cache miss path**: Continue to Stage 3.

**Events emitted**: `intelligence.request.cached` (cache hit), `intelligence.cache.miss` (cache miss)

---

## Stage 3: Safety Input Check

**Input**: `ModelRequest` from Router.

**Output**: `SafetyResult` (pass / blocked / redacted).

**Behavior**:

1. The Safety module collects all active safety rules matching the request's `required_safety_level`.
2. **PII Detection**: All user-supplied input fields are scanned for PII patterns (email, phone, SSN, credit card). Matches are redacted or flagged based on the safety rule action.
3. **Prompt Injection Detection**: The input is scanned for prompt-injection patterns: system prompt extraction, role manipulation, instruction override markers, jailbreak frameworks. Detected injections are blocked or redacted.
4. **Jailbreak Detection**: Known jailbreak patterns (DAN, roleplay-based jailbreaks, encoded payloads) are matched.
5. If `passed == false` and action is `Block`: The request is rejected with `SafetyViolation`. A `SafetyEvent` is emitted. No model call is made.
6. If action is `Redact` or `Sanitize`: The redacted input replaces the original. The request proceeds.
7. If action is `Warn` or `Log`: The request proceeds with the original input. A `SafetyEvent` is logged.

**Events emitted**: `intelligence.safety.violation`, `intelligence.safety.blocked` (if blocked)

---

## Stage 4: Context Assembly

**Input**: `ModelRequest` (possibly redacted) and `Conversation` (if continuing a session).

**Output**: `ContextWindow`.

**Behavior**:

1. **System Prompt**: The Prompt module resolves the system prompt from the registered template matching the request's tags. Template variables are substituted. If `system_prompt_prepend` is enabled, the system prompt is prepended to messages.
2. **Conversation Messages**: If a `conversation_id` is present, prior messages are fetched from the Conversation Store (Memory Platform).
3. **Memory Retrieval**: If `memory_retrieval_enabled` is true, an embedding is generated for the current input. A vector similarity search is performed against stored Memory embeddings. The top-k (default: 10) memories with similarity above threshold are included as system-level context messages.
4. **Perception Integration**: If `perception_integration_enabled` is true, the most recent N (default: 20) observations from the Perception Platform are fetched via EventBus or Memory. They are formatted as context messages.
5. **Tool Definitions**: If the tool registry has tools matching the model's capabilities, they are formatted into the model's native tool-calling format and appended.
6. **Truncation**: The assembled context is checked against `max_context_tokens`:
   - `SlidingWindow`: Oldest messages are dropped until under the limit.
   - `Summarize`: Old messages are passed to a secondary summarization model call. The summary replaces the dropped messages.
   - `Relevance`: Messages below the similarity threshold to the current input are dropped.
7. Safety rules from the template are applied to the final assembled context.

**Events emitted**: None directly. Context assembly contributes to the request latency metric.

---

## Stage 5: Model Invocation

**Input**: `ContextWindow` and `RoutingDecision` (selected model + fallbacks).

**Output**: `ModelResponse`.

**Behavior**:

1. The Router returns the primary `ModelInfo`. The Provider corresponding to that model is selected.
2. The Provider's `chat()` (non-streaming) or `chat_stream()` (streaming) method is called with the assembled `ContextWindow`, `GenerationParameters`, and formatted `ToolDefinition` list.
3. The request is translated from platform-native `ModelRequest` to provider-native format. Provider-specific authentication headers and retry logic are handled inside the provider adapter.
4. **Rate limit check**: Before each call, the provider's rate limiter is checked. If at limit, the request waits up to `rate_limit_wait_ms` before being retried locally.
5. **Timeout**: The provider call is wrapped in a timeout (`timeout_ms` from provider config). If the timeout fires, the primary provider is marked as `Degraded` and the fallback chain is attempted.
6. **Retry**: On transient errors (rate limit, connection failure, server error), retry with the same provider up to 3 times with exponential backoff. After 3 failures, advance to the next fallback.
7. **Fallback**: The fallback chain is traversed. Each fallback model may be on a different provider. The request is re-translated for the new provider and re-submitted.
8. If all fallbacks fail, return `ModelError::NoFallbackAvailable`.

**Events emitted**: `intelligence.request.fallback_used` (when falling back), `intelligence.request.failed` (all fallbacks exhausted)

---

## Stage 5a: Streaming Sub-Pipeline

When `GenerationParameters` requests streaming (`stream = true`):

1. The provider returns a streaming response (SSE or WebSocket).
2. The `StreamDecoder` receives raw bytes and decodes them into `StreamChunk` events.
3. Chunks are forwarded to the consumer (Brain) via `mpsc::Sender<StreamChunk>` as they arrive.
4. **Tool call aggregation**: `ToolCallDelta` chunks are buffered and assembled into complete `ToolCall` objects when the tool call is complete.
5. **Per-chunk safety**: If `safety_check_per_chunk` is enabled, each content chunk is scanned for safety violations before forwarding. Violating chunks are filtered or cause stream termination.
6. **Usage updates**: Providers that stream token counts send `UsageUpdate` chunks. These update the running `TokenUsage` counter.
7. On stream completion, `StreamChunk::Done` is emitted, followed by the final `ModelResponse`.
8. Final safety check is performed on the assembled response.
9. Cost tracking records the final token usage.

**Events emitted**: `intelligence.stream.chunk` (per chunk), `intelligence.stream.done` (on completion)

---

## Stage 6: Safety Output Check

**Input**: `ModelResponse` (or assembled stream response).

**Output**: `SafetyResult`.

**Behavior**:

1. The Safety module scans the `content` field (and `structured_data` if present) for safety violations:
   - Toxicity detection
   - Bias detection
   - PII (in output)
   - Any custom rules
2. If violations are found:
   - `Block`: The entire response is rejected. `SafetyViolation` event is emitted. The Brain receives an error or a fallback is triggered.
   - `Redact`: PII matches are replaced with the `redaction_character`. The sanitized response is forwarded.
   - `Warn`: The response is forwarded as-is. A `SafetyEvent` is logged.
   - `Log`: Only a `SafetyEvent` is logged. Response forwarded as-is.
3. If `Strict` safety level is configured, all `Block` and `Redact` rules are enforced automatically.

**Events emitted**: `intelligence.safety.violation`, `intelligence.safety.blocked`

---

## Stage 7: Cost Tracking

**Input**: `ModelRequest` and `ModelResponse`.

**Output**: `CostAccount` persisted to Memory.

**Behavior**:

1. The Cost module calculates `estimated_cost_cents` from `TokenUsage` and the model's `ModelPricing`:

```
input_cost = prompt_tokens * (input_per_million / 1_000_000)
output_cost = completion_tokens * (output_per_million / 1_000_000)
total_cost = input_cost + output_cost
```

2. A `CostAccount` is created and persisted to the Memory Platform (episodic store) with full metadata.
3. Budget checks are performed:
   - Per-request: If `total_cost > max_cents_per_request`, emit `BudgetExceeded`.
   - Per-conversation: If sum of this conversation's requests exceeds `max_cents_per_conversation`, emit `BudgetExceeded`.
   - Global daily: If daily total exceeds `global_daily.max_cents`, emit `BudgetExceeded` and block future requests.
4. If any budget is exceeded, the event is emitted and the request is flagged.

**Events emitted**: `intelligence.cost.budget_warning` (approaching), `intelligence.cost.budget_exceeded` (exceeded)

---

## Stage 8: Cache Store

**Input**: `ModelRequest`, `ModelResponse`, `CacheKey`.

**Output**: Cache entry stored.

**Behavior**:

1. A cache key is computed: SHA-256 of `(model_id, capability_kind, normalized_input, generation_parameters)`.
2. The Cache module stores the `ModelResponse` keyed by the SHA-256 hash with a TTL from configuration.
3. If `semantic_dedup_enabled` is true, an embedding of the request input is generated. The embedding and similarity threshold are stored alongside the cache entry. Future requests with semantically similar inputs (similarity > threshold) can match this entry.

---

## Stage 9: Telemetry and Return

**Input**: `ModelResponse` with safety result, cost data, and telemetry metadata.

**Output**: `ModelResponse` returned to Brain.

**Behavior**:

1. A `TelemetryRecord` is created with:
   - Request ID, provider ID, model ID
   - Latency (wall-clock from request start)
   - Token counts (prompt, completion)
   - Cost (total cents)
   - Cache hit flag
   - Fallback used flag
   - Safety event count
   - Timestamp
2. The telemetry record is published as an event and written to the Memory Platform.
3. If caching was enabled and a cache hit occurred, `intelligence.request.cached` is emitted.
4. The `ModelResponse` is returned to the Brain Platform (via EventBus or direct channel).

---

## End-to-End Sequence Diagram

```
Brain           Coordinator       Router         Safety        Context       Provider       Telemetry
  |                |                |              |              |              |              |
  |---ModelRequest>|                |              |              |              |              |
  |                |--CacheLookup-->|              |              |              |              |
  |                |<--cache miss---|              |              |              |              |
  |                |--SafetyCheck-->|              |              |              |              |
  |                |<--passed------|              |              |              |              |
  |                |--AssembleContext---------------------->|              |              |
  |                |<--ContextWindow--------------------|              |              |
  |                |--ModelCall---->|              |              |              |              |
  |                |                |              |              |----call---->|              |
  |                |<-------------Response-----------|              |              |              |
  |                |--OutputCheck-->|              |              |              |              |
  |                |<--passed------|              |              |              |              |
  |                |--CostTrack---->|              |              |              |              |
  |                |--CacheStore(()>|              |              |              |              |
  |                |--RecordTel---->|              |              |              |              |
  |<--ModelResponse|                |              |              |              |              |
```

---

## References

- [Intelligence Platform Architecture](../architecture/intelligence.md)
- [Intelligence Platform Interfaces](../interfaces/intelligence.md)
- [Intelligence Platform Configuration](../configuration/intelligence.md)
- [RFC-0006: Intelligence Platform](../rfc/RFC-0006-intelligence-platform.md)
- [Brain Platform Architecture](../architecture/brain.md)
- [Memory Platform Architecture](../architecture/memory.md)
- [Perception Platform Architecture](../architecture/perception.md)
- [Execution Platform Architecture](../architecture/execution.md)
