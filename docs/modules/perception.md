# Perception Platform Module

**Module:** `ai_os_perception`
**Status:** Planned (Phase 7)
**Crate:** Not yet created

## Purpose

The Perception Platform module is the sensory front-end of the AI-native OS. It abstracts heterogeneous input sources into a uniform stream, normalizes and validates incoming data, classifies events by type and priority, filters noise through attentional gating, and enriches events with contextual metadata. Perception ensures that the Brain and other cognitive modules receive a clean, prioritized, and contextually grounded signal.

## Status Overview

| Subsystem | Design | Implementation | Tests |
|---|---|---|---|
| SensorAbstraction | Draft | Not started | Not started |
| InputNormalizer | Draft | Not started | Not started |
| EventClassifier | Draft | Not started | Not started |
| AttentionMechanism | Draft | Not started | Not started |
| ContextEnricher | Draft | Not started | Not started |

## Public Interfaces

### `SensorAbstraction`

Planned trait and registry for managing heterogeneous input sources.

```rust
/// Planned for Phase 7
#[async_trait]
pub trait Sensor: Send + Sync {
    /// Unique identifier for this sensor.
    fn id(&self) -> &str;

    /// Sensor category (e.g., "stdin", "http", "file_watch", "dbus").
    fn kind(&self) -> &str;

    /// Read raw data. Returns None when the stream is exhausted.
    async fn read(&self) -> Result<Option<RawInput>, PerceptionError>;

    /// Process raw input into a normalized intermediate representation.
    async fn process(&self, raw: RawInput) -> Result<NormalizedInput, PerceptionError>;

    /// Publish the processed input onto the EventBus.
    async fn publish(&self, input: NormalizedInput, bus: &EventBus) -> Result<(), PerceptionError>;
}

/// Planned for Phase 7
pub struct SensorRegistry {
    sensors: RwLock<HashMap<String, Arc<dyn Sensor>>>,
}

impl SensorRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, sensor: Arc<dyn Sensor>);
    pub fn unregister(&mut self, id: &str) -> Option<Arc<dyn Sensor>>;
    pub fn get(&self, id: &str) -> Option<Arc<dyn Sensor>>;
    pub fn list(&self) -> Vec<String>;

    /// Start all registered sensors. Each sensor spawns its own read->process->publish loop.
    pub async fn start_all(&self, bus: &EventBus) -> Result<(), PerceptionError>;
}
```

### `InputNormalizer`

Planned struct for schema validation, sanitization, and type coercion.

```rust
/// Planned for Phase 7
pub struct InputNormalizer {
    schemas: HashMap<String, Arc<dyn InputSchema>>,
    sanitizers: Vec<Arc<dyn Sanitizer>>,
}

/// Planned for Phase 7
#[async_trait]
pub trait InputSchema: Send + Sync {
    fn name(&self) -> &str;
    async fn validate(&self, input: &NormalizedInput) -> Result<(), PerceptionError>;
}

/// Planned for Phase 7
#[async_trait]
pub trait Sanitizer: Send + Sync {
    async fn sanitize(&self, input: &mut NormalizedInput) -> Result<(), PerceptionError>;
}

impl InputNormalizer {
    pub fn new() -> Self;
    pub fn register_schema(&mut self, schema: Arc<dyn InputSchema>);
    pub fn register_sanitizer(&mut self, sanitizer: Arc<dyn Sanitizer>);

    /// Normalize by running sanitizers then validating against all registered schemas.
    pub async fn normalize(&self, raw: RawInput) -> Result<NormalizedInput, PerceptionError>;
}
```

`NormalizedInput` is a structured type carrying:

```rust
/// Planned for Phase 7
pub struct NormalizedInput {
    pub source: String,
    pub timestamp: SystemTime,
    pub content_type: ContentType,   // Text, Json, Yaml, Binary, Event
    pub body: Vec<u8>,
    pub metadata: HashMap<String, String>,
    pub original_raw: Option<RawInput>,
}
```

### `EventClassifier`

Planned struct for rule-based classification and priority assignment.

```rust
/// Planned for Phase 7
pub struct EventClassifier {
    rules: Vec<ClassificationRule>,
    default_category: EventCategory,
    default_priority: Priority,
}

/// Planned for Phase 7
pub enum EventCategory {
    Command,
    Query,
    Notification,
    Alert,
}

/// Planned for Phase 7
pub struct ClassificationRule {
    pub name: String,
    pub matcher: Box<dyn PatternMatcher>,        // reuses brain::PatternMatcher
    pub category: EventCategory,
    pub priority_override: Option<Priority>,
}

/// Planned for Phase 7
pub struct ClassifiedEvent {
    pub input: NormalizedInput,
    pub category: EventCategory,
    pub priority: Priority,
    pub classification_confidence: f64,
}

impl EventClassifier {
    pub fn new(default_category: EventCategory, default_priority: Priority) -> Self;
    pub fn add_rule(&mut self, rule: ClassificationRule);
    pub async fn classify(&self, input: &NormalizedInput) -> ClassifiedEvent;
}
```

Classification applies rules in order; the first matching rule wins. If no rule matches, the default category and priority are used.

### `AttentionMechanism`

Planned struct for filtering and rate-limiting noisy input streams.

```rust
/// Planned for Phase 7
pub struct AttentionMechanism {
    priority_queue: BinaryHeap<ClassifiedEvent>,
    novelty_filter: BloomFilter<String>,
    rate_limiters: HashMap<String, RateLimiter>,
    config: AttentionConfig,
}

/// Planned for Phase 7
pub struct AttentionConfig {
    pub max_queue_size: usize,
    pub novelty_bloom_capacity: usize,
    pub novelty_bloom_fp_rate: f64,   // false positive rate
    pub default_rate_per_sec: u32,
}

/// Planned for Phase 7
impl AttentionMechanism {
    pub fn new(config: AttentionConfig) -> Self;

    /// Ingest a classified event. Returns Some(event) if it passes the gate, None if suppressed.
    pub async fn ingest(&mut self, event: ClassifiedEvent) -> Option<ClassifiedEvent>;

    /// Dequeue the highest-priority event.
    pub fn dequeue(&mut self) -> Option<ClassifiedEvent>;

    /// Register a per-source rate limit.
    pub fn set_rate_limit(&mut self, source: &str, per_sec: u32);
}
```

Three gates:
1. **Novelty filter**: emits events whose content hash is novel (first seen); repeats within the bloom filter window are suppressed unless they carry a higher priority than the original.
2. **Rate limiter**: per-source token bucket; drops events that exceed the configured rate.
3. **Priority ordering**: events that pass both gates enter the `BinaryHeap`. High-priority events (Alert, Command) are dequeued before low-priority (Notification, Query).

### `ContextEnricher`

Planned struct for adding temporal, spatial, and entity metadata.

```rust
/// Planned for Phase 7
pub struct ContextEnricher {
    entity_extractors: Vec<Arc<dyn EntityExtractor>>,
    temporal_resolver: Arc<dyn TemporalResolver>,
    spatial_resolver: Arc<dyn SpatialResolver>,
}

/// Planned for Phase 7
#[async_trait]
pub trait EntityExtractor: Send + Sync {
    fn name(&self) -> &str;
    async fn extract(&self, text: &str) -> Vec<ExtractedEntity>;
}

/// Planned for Phase 7
pub struct ExtractedEntity {
    pub mention: String,
    pub entity_type: String,    // "person", "location", "date", "file_path", etc.
    pub normalized_value: String,
    pub confidence: f64,
}

/// Planned for Phase 7
#[async_trait]
pub trait TemporalResolver: Send + Sync {
    /// Resolve relative time expressions ("tomorrow", "in 5 minutes") to absolute timestamps.
    async fn resolve(&self, expression: &str, reference: SystemTime) -> Result<SystemTime, PerceptionError>;
}

/// Planned for Phase 7
#[async_trait]
pub trait SpatialResolver: Send + Sync {
    /// Resolve location names to coordinates or paths.
    async fn resolve(&self, name: &str) -> Result<Option<String>, PerceptionError>;
}

impl ContextEnricher {
    pub fn new() -> Self;
    pub fn register_extractor(&mut self, extractor: Arc<dyn EntityExtractor>);

    /// Enrich a normalized input with contextual metadata.
    pub async fn enrich(&self, input: &NormalizedInput) -> Result<EnrichedInput, PerceptionError>;
}

/// Planned for Phase 7
pub struct EnrichedInput {
    pub normalized: NormalizedInput,
    pub entities: Vec<ExtractedEntity>,
    pub resolved_time: Option<SystemTime>,
    pub resolved_location: Option<String>,
    pub enrichment_metadata: HashMap<String, String>,
}
```

## Dependencies

| Dependency | Scope | Purpose |
|---|---|---|
| `ai_os_core` | internal | EventBus, Service, Logger |
| `ai_os_brain` | internal | PatternMatcher reuse for classification rules |
| `bloom` (or `bloomfilter`) | external | Probabilistic novelty detection |
| `governor` | external | Token-bucket rate limiting |
| `regex` | external | Entity extraction patterns |
| `chrono` | external | Temporal expression parsing and resolution |
| `tokio` | runtime | Async sensor loops |

## Events Published

| Event | Trigger | Payload |
|---|---|---|
| `perception::InputEvent` | Any sensor publishes normalized input | `NormalizedInput` |
| `perception::ClassifiedEvent` | After classification and attention gating | `ClassifiedEvent` |
| `perception::EnrichedEvent` | After context enrichment | `EnrichedInput` |
| `perception::SensorOffline` | Sensor read returns error repeatedly | `(String, String)` -- sensor id, reason |
| `perception::SensorOnline` | Sensor recovers after offline | `String` -- sensor id |

## Events Consumed

| Event | Consumer | Action |
|---|---|---|
| `core::HealthStatusChanged` | `SensorRegistry` | Pause/resume non-critical sensors when degraded |
| `brain::ClarificationRequest` | `SensorRegistry` | Re-activate an input sensor for follow-up |
| `runtime::PhaseChanged` | `AttentionMechanism` | Flush queue during shutdown |

## Thread Model

- `SensorRegistry` stores sensors behind `Arc<dyn Sensor>` in an `RwLock<HashMap>`. Each sensor's `read->process->publish` loop runs on its own Tokio task.
- `InputNormalizer` and `EventClassifier` are stateless after construction; shared via `Arc`.
- `AttentionMechanism` holds mutable state (queue, bloom filter, rate limiters) behind a `Mutex`. High-throughput sensors may contend here; an `mpsc` channel could be introduced upstream as an optimization.
- `ContextEnricher` is read-only after construction; shared via `Arc`.

## Lifecycle

```
Boot sequence:
  1. InputNormalizer::new (register built-in schemas and sanitizers)
  2. EventClassifier::new (register classification rules)
  3. ContextEnricher::new (register default entity extractors)
  4. AttentionMechanism::new (configure rate limits)
  5. SensorRegistry::new -> register sensors -> start_all
```

Each sensor runs an infinite loop:

```
loop {
    let raw = sensor.read().await?;
    let normal = normalizer.normalize(raw).await?;
    let classified = classifier.classify(&normal).await;
    if let Some(gated) = attention.ingest(classified).await {
        let enriched = enricher.enrich(&gated.input).await?;
        bus.publish("perception::EnrichedEvent", enriched).await;
    }
}
```

Shutdown: `SensorRegistry::stop_all` drops sensor tasks via cancellation tokens.

## Error Handling

Error type: `ai_os_perception::PerceptionError` with variants:

- `SensorReadError(String)` -- underlying I/O or protocol error.
- `ValidationError(String)` -- schema validation failure with field-level details.
- `SanitizationError(String)` -- sanitizer could not process input.
- `ClassificationError(String)` -- rule application error.
- `RateLimited(String)` -- event dropped by rate limiter (logged, not returned to caller).
- `EnrichmentError(String)` -- entity extraction or resolution failure.

Recovery: Sensor read errors are retried 3 times before the sensor is marked `SensorOffline`. Validation errors reject the input and log a warning; the raw event is still published as an `InputEvent` with an error annotation. Rate-limited events are silently dropped (the attention mechanism logs a counter metric).

## Configuration

```toml
[attention]
max_queue_size = 1024
novelty_bloom_capacity = 1_000_000
novelty_bloom_fp_rate = 0.01
default_rate_per_sec = 10

[classifier]
default_category = "Notification"
default_priority = 50

[sensors]
# Per-sensor overrides
[sensors.stdin]
enabled = true
rate_per_sec = 100

[sensors.file_watch]
enabled = true
rate_per_sec = 5
```

Configuration file: `aios-perception.toml`.

## Testing Strategy

- **Unit tests**: InputNormalizer schema validation (reject malformed JSON, accept valid). EventClassifier rule ordering (first match wins). AttentionMechanism novelty filter (same content within window is suppressed). ContextEnricher entity extraction with golden regex patterns.
- **Integration tests**: End-to-end sensor pipeline: mock Sensor -> InputNormalizer -> EventClassifier -> AttentionMechanism -> ContextEnricher -> EventBus. Verify rate limiter drops events above threshold.
- **Fuzz tests**: Random byte sequences fed through the full pipeline; assert no panics.
- **Benchmarks**: AttentionMechanism throughput with 1M unique events (bloom filter false positive rate). Enricher latency with 1000-character input and 10 entity extractors.

## Future Extensions

- Adaptive attention: the novelty threshold and rate limits adjust based on the current cognitive load (published by the Brain).
- Multi-modal sensor fusion: combine audio, image, and text sensors before classification.
- Sensor health scoring: track sensor reliability and dynamically deregister faulty sensors.
- Privacy filter: a pre-classification sanitizer that redacts PII before enrichment.
- Correlation engine: group related events across sensors (e.g., a file change + a process spawn with the same path).
