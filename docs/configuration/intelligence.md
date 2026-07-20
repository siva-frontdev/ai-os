# Intelligence Platform Configuration

## Overview

The Intelligence Platform is configured via `intelligence.toml` under the `[intelligence]` section of the platform configuration file. This document specifies every configuration key, its type, default, and validation rules.

---

## Full Configuration Schema

### Root

```toml
[intelligence]
enabled = true
max_concurrent_requests = 50
default_safety_level = "Standard"
default_routing_policy = "Balanced"
```

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `true` | Enable the Intelligence Platform |
| `max_concurrent_requests` | `u32` | `50` | Maximum concurrent inference requests |
| `default_safety_level` | `string` | `"Standard"` | Default safety level: None, Standard, Strict, Custom |
| `default_routing_policy` | `string` | `"Balanced"` | Default routing policy: BestCapability, LowestCost, LowestLatency, Balanced |

---

### Providers

```toml
[intelligence.providers.openai]
enabled = true
api_key_env = "OPENAI_API_KEY"
api_base = "https://api.openai.com/v1"
rate_limit_requests_per_min = 3000
rate_limit_tokens_per_min = 500000
concurrent_requests = 50
region = "us-east-1"
timeout_ms = 60000

[intelligence.providers.anthropic]
enabled = true
api_key_env = "ANTHROPIC_API_KEY"
api_base = "https://api.anthropic.com/v1"
rate_limit_requests_per_min = 1000
rate_limit_tokens_per_min = 200000
concurrent_requests = 20
timeout_ms = 60000

[intelligence.providers.local]
enabled = true
api_base = "http://localhost:8080/v1"
timeout_ms = 120000
```

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `true` | Register this provider |
| `api_key_env` | `string` | — | Environment variable containing API key |
| `api_base` | `string` | — | Base URL for API calls |
| `rate_limit_requests_per_min` | `u32` | `unlimited` | Rate limit (RPM) |
| `rate_limit_tokens_per_min` | `u32` | `unlimited` | Token rate limit (TPM) |
| `concurrent_requests` | `u32` | `10` | Max concurrent requests to this provider |
| `region` | `string` | — | Preferred region |
| `timeout_ms` | `u64` | `60000` | Request timeout in milliseconds |

---

### Models

```toml
[intelligence.models.gpt4o]
provider = "openai"
external_name = "gpt-4o"
capabilities = ["Chat", "CodeGeneration", "Embedding"]
context_window = 128000
pricing.input_per_million = 2.50
pricing.output_per_million = 10.00
max_input_tokens = 128000
max_output_tokens = 16384
supports_streaming = true
supports_tools = true
supports_vision = true
tags = ["flagship", "fast", "vision"]

[intelligence.models.claude-sonnet]
provider = "anthropic"
external_name = "claude-sonnet-4-20250514"
capabilities = ["Chat", "CodeGeneration", "Summarization"]
context_window = 200000
pricing.input_per_million = 3.00
pricing.output_per_million = 15.00
max_input_tokens = 200000
max_output_tokens = 8192
supports_streaming = true
supports_tools = true
```

| Key | Type | Default | Description |
|---|---|---|---|
| `provider` | `string` | — | Provider key from `[intelligence.providers.*]` |
| `external_name` | `string` | — | Model name as used by the provider API |
| `capabilities` | `[]string` | — | List of `CapabilityKind` values |
| `context_window` | `u32` | — | Maximum context tokens |
| `pricing.input_per_million` | `f64` | `0.0` | Cost per 1M input tokens (USD) |
| `pricing.output_per_million` | `f64` | `0.0` | Cost per 1M output tokens (USD) |
| `max_input_tokens` | `u32` | context_window | Max tokens in request |
| `max_output_tokens` | `u32` | `4096` | Max tokens in response |
| `supports_streaming` | `bool` | `false` | Whether model supports streaming |
| `supports_tools` | `bool` | `false` | Whether model supports tool calling |
| `supports_vision` | `bool` | `false` | Whether model accepts image inputs |
| `tags` | `[]string` | `[]` | Arbitrary tags for routing/selection |

---

### Routing

```toml
[intelligence.routing]
default_policy = "Balanced"
capability_weight = 0.5
latency_weight = 0.3
cost_weight = 0.2
availability_weight = 0.0
cache_enabled = true
cache_ttl_seconds = 3600
cache_semantic_dedup = true
cache_similarity_threshold = 0.95

[intelligence.routing.fallback]
max_fallbacks = 3
backoff_initial_ms = 500
backoff_multiplier = 2.0
backoff_max_ms = 10000
```

| Key | Type | Default | Description |
|---|---|---|---|
| `default_policy` | `string` | `"Balanced"` | Default routing policy name |
| `capability_weight` | `f64` | `0.5` | Weight for capability fit in scoring |
| `latency_weight` | `f64` | `0.3` | Weight for latency in scoring |
| `cost_weight` | `f64` | `0.2` | Weight for cost in scoring |
| `cache_enabled` | `bool` | `true` | Enable response caching |
| `cache_ttl_seconds` | `u64` | `3600` | Cache entry TTL |
| `max_fallbacks` | `u32` | `3` | Maximum fallback attempts |
| `backoff_initial_ms` | `u64` | `500` | Initial backoff delay |
| `backoff_multiplier` | `f64` | `2.0` | Backoff multiplier |

---

### Prompts

```toml
[intelligence.prompts]
default_template_language = "Simple"
auto_register = true
templates_path = "/etc/ai-os/prompts"

[intelligence.prompts.builtin.system_chat]
name = "Default System Chat"
template = "You are a helpful AI assistant.{{#if context}}Context: {{context}}{{/if}}"
safety_rules = ["pii-redact", "injection-block"]
tags = ["system", "chat"]
```

| Key | Type | Default | Description |
|---|---|---|---|
| `default_template_language` | `string` | `"Simple"` | Mustache, Handlebars, Jinja2, Simple |
| `auto_register` | `bool` | `true` | Auto-register templates from `templates_path` |
| `templates_path` | `string` | — | Directory containing template files |

---

### Context

```toml
[intelligence.context]
truncation_strategy = "SlidingWindow"
sliding_window_size = 50
max_context_tokens = 200000
memory_retrieval_enabled = true
memory_retrieval_top_k = 10
perception_integration_enabled = true
perception_max_recent = 20
```

| Key | Type | Default | Description |
|---|---|---|---|
| `truncation_strategy` | `string` | `"SlidingWindow"` | SlidingWindow, Summarize, Relevance |
| `sliding_window_size` | `u32` | `50` | Messages to keep |
| `max_context_tokens` | `u32` | `200000` | Max context tokens |
| `memory_retrieval_enabled` | `bool` | `true` | Retrieve relevant memories |
| `memory_retrieval_top_k` | `u32` | `10` | Number of memories |
| `perception_integration_enabled` | `bool` | `true` | Include recent observations |
| `perception_max_recent` | `u32` | `20` | Max recent observations |

---

### Safety

```toml
[intelligence.safety]
default_level = "Standard"
block_on_violation = true
log_all_checks = true
redaction_character = "[REDACTED]"

[intelligence.safety.rules.pii-redact]
category = "Pii"
action = "Redact"
severity = "High"
enabled = true

[intelligence.safety.rules.injection-block]
category = "PromptInjection"
action = "Block"
severity = "Critical"
enabled = true
```

| Key | Type | Default | Description |
|---|---|---|---|
| `default_level` | `string` | `"Standard"` | None, Standard, Strict, Custom |
| `block_on_violation` | `bool` | `true` | Block request on violation |
| `log_all_checks` | `bool` | `true` | Log all safety check results |

---

### Cache

```toml
[intelligence.cache]
enabled = true
ttl_seconds = 3600
max_entries = 100000
max_size_mb = 500
semantic_dedup_enabled = true
similarity_threshold = 0.95
eviction_policy = "LRU"
```

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `true` | Enable response caching |
| `ttl_seconds` | `u64` | `3600` | Cache entry TTL |
| `max_entries` | `u64` | `100000` | Max cache entries |
| `semantic_dedup_enabled` | `bool` | `true` | Semantic similarity matching |
| `similarity_threshold` | `f64` | `0.95` | Cosine similarity for cache hit |

---

### Cost and Budgeting

```toml
[intelligence.cost]
tracking_enabled = true
currency = "USD"

[intelligence.cost.budgets.per_request]
max_cents = 100

[intelligence.cost.budgets.per_conversation]
max_cents = 1000

[intelligence.cost.budgets.global_daily]
max_cents = 10000

[intelligence.cost.budgets.alert_threshold_percent] = 80
```

| Key | Type | Default | Description |
|---|---|---|---|
| `tracking_enabled` | `bool` | `true` | Enable cost tracking |
| `per_request.max_cents` | `f64` | `100` | Max cost per single request |
| `per_conversation.max_cents` | `f64` | `1000` | Max cost per conversation |
| `global_daily.max_cents` | `f64` | `10000` | Max cost per day globally |
| `alert_threshold_percent` | `u32` | `80` | Alert at this percent of budget |

---

### Streaming

```toml
[intelligence.streaming]
enabled = true
chunk_buffer_size = 4096
safety_check_per_chunk = false
safety_check_at_end = true
tool_call_aggregation = true
```

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `true` | Enable streaming for supported models |
| `chunk_buffer_size` | `u32` | `4096` | Bytes per chunk buffer |
| `safety_check_per_chunk` | `bool` | `false` | Check each chunk for safety |
| `safety_check_at_end` | `bool` | `true` | Check final assembled response |
| `tool_call_aggregation` | `bool` | `true` | Aggregate tool call deltas |

---

### Telemetry

```toml
[intelligence.telemetry]
enabled = true
export_interval_secs = 60
storage_backend = "memory"
log_requests = true
log_safety_events = true
metrics_retention_days = 30
```

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `true` | Enable telemetry collection |
| `export_interval_secs` | `u64` | `60` | Metrics export interval |
| `storage_backend` | `string` | `"memory"` | Where to write: memory, log, both |
| `log_requests` | `bool` | `true` | Log every inference request |
| `log_safety_events` | `bool` | `true` | Log safety event details |

---

### Configuration Lifecycle

Configuration is loaded at startup from `platform.toml`. Changes to the file are detected by the Core config watcher and trigger `core.config.reloaded` on the EventBus. The Coordinator reloads all modules in sequence:

1. Providers: re-read API keys and timeouts
2. Models: refresh discovery from providers
3. Routing: update policy
4. Safety: reload rules
5. Cache: update TTL and eviction policy
6. Cost: update budgets

Environment variable overrides take precedence over `platform.toml`:

| Env Var | Overrides |
|---|---|
| `INTELLIGENCE_ENABLED` | Root `enabled` |
| `INTELLIGENCE_MAX_CONCURRENT` | Root `max_concurrent_requests` |
| `INTELLIGENCE_DEFAULT_SAFETY` | Root `default_safety_level` |
| `INTELLIGENCE_CACHE_TTL` | Cache `ttl_seconds` |
| `INTELLIGENCE_DAILY_BUDGET_CENTS` | Cost `global_daily.max_cents` |
