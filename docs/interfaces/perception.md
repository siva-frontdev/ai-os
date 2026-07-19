# Perception Platform Interface — `perception` *(planned, Phase 7)*

## Purpose

The Perception layer implements the sensor ingestion pipeline. It receives raw data from multiple sources (stdin, file watchers, network sockets, hardware sensors), normalizes and classifies each input, applies attentional filtering to avoid overload, and enriches the result with context before passing it to the Brain layer. Perception is the system's sensory periphery — it converts undifferentiated data streams into structured, prioritized observations.

## Public APIs

### Sensor — raw data source abstraction

```rust
#[async_trait]
pub trait Sensor: Debug + Send + Sync {
    fn id(&self) -> SensorId;
    fn kind(&self) -> SensorKind;
    async fn read(&self) -> Result<SensorReading, SensorError>;
    async fn open(&self, config: SensorConfig) -> Result<(), SensorError>;
    async fn close(&self) -> Result<(), SensorError>;
    fn stream(&self) -> Box<dyn Stream<Item = Result<SensorReading, SensorError>> + Unpin + Send>;
}
```

### InputNormalizer — raw bytes to structured data

```rust
pub trait InputNormalizer: Debug + Send + Sync {
    async fn normalize(&self, reading: SensorReading) -> Result<NormalizedInput, NormalizerError>;
    async fn normalize_batch(&self, readings: Vec<SensorReading>) -> Result<Vec<NormalizedInput>, NormalizerError>;
    fn supported_mime_types(&self) -> Vec<MimeType>;
    fn register_format(&self, mime: MimeType, parser: Box<dyn InputParser>);
}
```

### EventClassifier — input categorization and priority

```rust
#[async_trait]
pub trait EventClassifier: Debug + Send + Sync {
    async fn classify(&self, input: NormalizedInput) -> Result<ClassifiedEvent, ClassifierError>;
    async fn classify_batch(&self, inputs: Vec<NormalizedInput>) -> Result<Vec<ClassifiedEvent>, ClassifierError>;
    async fn priority(&self, event: &ClassifiedEvent) -> EventPriority;
    async fn reload_rules(&self) -> Result<(), ClassifierError>;
    fn events(&self) -> Box<dyn Stream<Item = ClassificationEvent> + Unpin + Send>;
}
```

### AttentionMechanism — salience-based filtering

```rust
#[async_trait]
pub trait AttentionMechanism: Debug + Send + Sync {
    async fn should_process(&self, event: &ClassifiedEvent, ctx: &AttentionContext) -> Result<bool, AttentionError>;
    async fn focus(&self, ctx: &AttentionContext) -> Result<AttentionFocus, AttentionError>;
    async fn set_threshold(&self, threshold: f32) -> Result<(), AttentionError>;
    async fn stats(&self) -> Result<AttentionStats, AttentionError>;
    fn stream(&self) -> Box<dyn Stream<Item = AttentionEvent> + Unpin + Send>;
}
```

### ContextEnricher — augment classified events with metadata

```rust
#[async_trait]
pub trait ContextEnricher: Debug + Send + Sync {
    async fn enrich(&self, event: ClassifiedEvent) -> Result<EnrichedEvent, EnricherError>;
    async fn enrich_batch(&self, events: Vec<ClassifiedEvent>) -> Result<Vec<EnrichedEvent>, EnricherError>;
    async fn register_enricher(&self, kind: EnricherKind, enricher: Box<dyn Enricher>) -> Result<(), EnricherError>;
    async fn reload(&self) -> Result<(), EnricherError>;
}
```

## Dependencies

- [Core](core.md) — `EventBus`, `Config`, `Logger`, `HealthMonitor`
- [OSAL](osal.md) — `FileSystem` (file watcher sensor), `Terminal` (stdin sensor), `NetworkManager` (socket sensor)
- `mime` (MIME type detection)
- `encoding_rs` (text encoding normalization)

## Lifecycle

1. **Init** — Sensors are registered via `Config`. Each sensor's `open` is called. Default `InputNormalizer` parsers are loaded (text, JSON, binary). `EventClassifier` loads classification rules. `AttentionMechanism` loads salience thresholds.
2. **Start** — Sensor event streams begin. Each sensor's `stream` is polled. Normalize → Classify → Filter → Enrich pipeline activates.
3. **Stop** — All sensor streams are dropped. Each sensor's `close` is called. Pending classifications are flushed.

## Events Published

| Event | Payload | Description |
|-------|---------|-------------|
| `perception.sensor_reading` | `{ sensor_id, kind, timestamp, size }` | Raw sensor reading received |
| `perception.event_classified` | `{ event_id, class, priority, normalized }` | Input classified and prioritized |
| `perception.event_enriched` | `{ event_id, context, enrichment_sources }` | Classified event augmented with context |
| `perception.input_dropped` | `{ sensor_id, reason, priority }` | Input below attention threshold |
| `perception.sensor_faulted` | `{ sensor_id, error }` | Sensor encountered an error |
| `perception.attention_shift` | `{ from_focus, to_focus, reason }` | Attention focus changed |

## Events Consumed

| Source | Event | Handling |
|--------|-------|----------|
| [OSAL](osal.md) | `osal.fs_modified` | File watcher sensor emits `SensorReading` |
| [OSAL](osal.md) | `osal.device_added` | Hotplug sensor activation |
| [Core](core.md) | `core.config_changed` | Reload classification rules, attention thresholds, sensor config |
| [Brain](brain.md) *(planned)* | `brain.attention_request` | `AttentionMechanism::should_process` with specific query |

## Error Model

| Error | Variant | Recovery |
|-------|---------|----------|
| `SensorError::Unavailable` | Sensor source not accessible | Retry with backoff, emit alert |
| `SensorError::Disconnected` | Sensor peer closed | Reconnect if configured |
| `NormalizerError::UnsupportedFormat` | MIME type has no parser | Skip reading, log warning |
| `ClassifierError::NoRules` | Classification rule set empty | Run in passthrough mode |
| `AttentionError::Overload` | Input rate exceeds capacity | Drop low-priority events |
| `EnricherError::SourceDown` | Context source unavailable | Return event without enrichment |

## Performance Expectations

| Operation | Target Latency (p99) | Throughput |
|-----------|---------------------|------------|
| Sensor read (file) | < 10 µs | 1M+ readings/s |
| Normalize (text, 1 KB) | < 5 µs | 200K+/s |
| Classify (rule-based) | < 2 µs | 500K+/s |
| Classify (ML-based) | < 5 ms | 200+/s |
| Attention filter check | < 1 µs | 1M+/s |
| Enrich (context lookup) | < 50 µs | 20K+/s |
| End-to-end pipeline (hot path) | < 100 µs | 10K+/s |

## Thread Model

- Each `Sensor` is polled from a dedicated tokio task. Sensors that are I/O-bound use `tokio::io`.
- `InputNormalizer` parsing is CPU-bound — heavy formats (PDF, images) use `spawn_blocking`.
- `EventClassifier` rule-based classification is lock-free. ML-based classification uses `spawn_blocking`.
- `AttentionMechanism` uses `Arc<RwLock<AttentionState>>`. The salience model is read on every check.
- `ContextEnricher` enrichment lookups may be async (calling [Memory](memory.md) or [Brain](brain.md) via EventBus).
- All pipeline stages communicate via bounded tokio channels for backpressure.

## Portability

- `Sensor` is a trait — platform-specific sensors (udev, Windows Raw Input, macOS IOKit) are implementations behind the trait.
- `InputNormalizer` parsers are backend-agnostic — they operate on `&[u8]`.
- `EventClassifier` rules are expressed in a portable DSL (JSON or TOML).
- `AttentionMechanism` uses a portable salience model (ONNX).
- No raw file descriptors, platform handles, or OS types appear in any trait signature.
- The pipeline (Sensor → Normalizer → Classifier → Attention → Enricher) is composed at startup from config. Stages can be skipped or reordered via config.
- Each pipeline stage is isolated — a fault in the normalizer does not crash the classifier or sensor.
- Pipeline throughput is regulated by a token bucket rate limiter configurable per sensor kind.
- Sensors can be dynamically registered and deregistered at runtime via the EventBus.
- Each sensor has a priority level that determines its position in the polling order.
- The attention mechanism exposes an adjustable threshold that can be tuned per workload.
- The classifier supports both rule-based and ML-based classification; backends are swappable per channel.
