# Perception Platform Blueprint

## Purpose

The Perception Platform (Phase 7, Planned) is the sensory layer of AI-native OS. It abstracts all input sources -- human (stdin, signals), system (file watchers, network sockets, device events), and external (IPC, WebSocket) -- into a unified stream of typed, normalized, and prioritized percepts. The Perception Platform does not interpret meaning (that is the Brain's responsibility); it handles the pipeline from raw input to classified, enriched, and attention-ranked percepts ready for consumption by higher layers.

The Perception Platform builds on the [System Platform](system.md) (system event sources) and [Core Platform](core.md) (EventBus). It feeds into the [Brain Platform](brain.md) for intent recognition and decision making.

---

## Responsibilities

- **Sensor Abstraction Layer**: Provide a uniform interface over diverse input sources. Each sensor implementation (stdin reader, file watcher, network listener, signal handler) implements the same `Sensor` trait. Sensors are registered at startup or dynamically discovered.
- **Input Normalization**: Validate input against schema definitions. Sanitize raw data (strip control characters, enforce encoding, limit size). Convert to the canonical `InputPayload` format with a consistent structure regardless of source.
- **Event Classification**: Classify normalized input into percept categories (command, query, alert, state_change, system_event, user_input). Assign a priority level based on configurable rules. Tag percepts with source metadata, confidence, and urgency.
- **Attention Mechanism**: Implement a configurable attention model that ranks percepts by priority, novelty, and recency. Use a decay function to reduce the attention score of repeated identical percepts (habituation). Support threshold-based filtering: percepts below the attention threshold are discarded or archived.
- **Context Enrichment**: Extract named entities (files, processes, sessions, users) from percept payloads using pattern matching and knowledge graph lookups. Attach temporal (wall-clock and monotonic) and spatial (filesystem path, network address, session scope) grounding to each percept.

---

## Public Interfaces

### Sensor (`perception::sensor`)

```rust
pub enum SensorType {
    Stdin,
    Signal,
    FileWatcher { path: String, events: Vec<FileEvent> },
    NetworkSocket { addr: SocketAddr, protocol: TransportProtocol },
    IpcChannel { name: String },
    SystemEvent { source: SystemEventSource },
}

pub enum TransportProtocol { Tcp, Udp, Unix }
pub enum SystemEventSource { Udev, Journal, Netlink, Process }

pub struct SensorConfig {
    pub sensor_type: SensorType,
    pub buffer_size: usize,
    pub encoding: String,
    pub max_payload_bytes: usize,
    pub enabled: bool,
}

pub trait Sensor: Debug + Send + Sync {
    fn sensor_type(&self) -> SensorType;
    fn name(&self) -> &str;
    async fn start(&self, tx: mpsc::Sender<RawInput>) -> Result<(), PerceptionError>;
    async fn stop(&self) -> Result<(), PerceptionError>;
    fn is_running(&self) -> bool;
}
```

### InputNormalizer (`perception::normalizer`)

```rust
pub struct Schema {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub max_depth: usize,
}

pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
}

pub enum FieldType { String, Integer, Float, Boolean, Object, Array }

pub struct NormalizedInput {
    pub sensor_name: String,
    pub source_type: SensorType,
    pub timestamp: DateTime<Utc>,
    pub monotonic_ns: u64,
    pub payload: InputPayload,
    pub size_bytes: usize,
    pub encoding: String,
    pub schema_version: String,
}

pub trait InputNormalizer: Debug + Send + Sync {
    async fn normalize(&self, raw: RawInput) -> Result<NormalizedInput, PerceptionError>;
    async fn validate(&self, normalized: &NormalizedInput) -> Result<(), PerceptionError>;
    async fn sanitize(&self, payload: &[u8]) -> Result<Vec<u8>, PerceptionError>;
    fn register_schema(&self, schema: Schema) -> Result<(), PerceptionError>;
}
```

### Classifier (`perception::classifier`)

```rust
pub enum PerceptCategory {
    Command,
    Query,
    Alert,
    StateChange,
    SystemEvent,
    UserInput,
    Heartbeat,
    Unknown,
}

pub enum PerceptPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

pub struct Percept {
    pub id: Uuid,
    pub category: PerceptCategory,
    pub priority: PerceptPriority,
    pub normalized: NormalizedInput,
    pub confidence: f64,
    pub tags: Vec<String>,
    pub classified_at: DateTime<Utc>,
}

pub struct ClassificationRule {
    pub name: String,
    pub pattern: String,
    pub category: PerceptCategory,
    pub priority: PerceptPriority,
    pub min_confidence: f64,
}

pub trait Classifier: Debug + Send + Sync {
    async fn classify(&self, input: NormalizedInput) -> Result<Percept, PerceptionError>;
    fn register_rule(&self, rule: ClassificationRule) -> Result<(), PerceptionError>;
    fn list_rules(&self) -> Vec<ClassificationRule>;
}
```

### AttentionMechanism (`perception::attention`)

```rust
pub struct AttentionConfig {
    pub base_threshold: f64,
    pub habituation_decay: f64,
    pub novelty_bonus: f64,
    pub recency_weight: f64,
    pub priority_weight: f64,
    pub max_queue_size: usize,
}

pub struct AttentionScore {
    pub percept_id: Uuid,
    pub score: f64,
    pub is_novel: bool,
    pub habituation_count: u32,
    pub reason: String,
}

pub trait AttentionMechanism: Debug + Send + Sync {
    async fn evaluate(&self, percept: &Percept) -> Result<AttentionScore, PerceptionError>;
    async fn should_process(&self, score: &AttentionScore) -> bool;
    fn update_habituation(&self, percept: &Percept);
    fn reset_habituation(&self, category: &PerceptCategory);
    fn config(&self) -> &AttentionConfig;
}
```

### ContextEnricher (`perception::enricher`)

```rust
pub struct EnrichedPercept {
    pub percept: Percept,
    pub entities: Vec<EntityRef>,
    pub temporal_context: TemporalContext,
    pub spatial_context: SpatialContext,
    pub session_context: Option<SessionRef>,
}

pub struct EntityRef {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub confidence: f64,
}

pub struct TemporalContext {
    pub wall_clock: DateTime<Utc>,
    pub monotonic_ns: u64,
    pub timezone: String,
    pub uptime_seconds: f64,
}

pub struct SpatialContext {
    pub cwd: Option<String>,
    pub hostname: String,
    pub session_id: Option<Uuid>,
    pub network_addr: Option<SocketAddr>,
}

pub trait ContextEnricher: Debug + Send + Sync {
    async fn enrich(&self, percept: Percept) -> Result<EnrichedPercept, PerceptionError>;
    async fn extract_entities(&self, text: &str) -> Result<Vec<EntityRef>, PerceptionError>;
    async fn resolve_session(&self, pid: u32) -> Result<Option<SessionRef>, PerceptionError>;
}
```

### PerceptionPlatform (`perception::PerceptionPlatform`)

```rust
pub struct PerceptionPlatform {
    pub sensors: Vec<Arc<dyn Sensor>>,
    pub normalizer: Arc<dyn InputNormalizer>,
    pub classifier: Arc<dyn Classifier>,
    pub attention: Arc<dyn AttentionMechanism>,
    pub enricher: Arc<dyn ContextEnricher>,
    sensor_handles: Mutex<Vec<JoinHandle<()>>>,
}
```

`PerceptionPlatform` implements Core's `Service` trait. Its `start()` method launches all registered sensors and spawns the attention evaluation loop.

---

## Dependencies

| Crate | Purpose | Subsystem |
|---|---|---|
| `ai-os-core` | EventBus, Service, Logger, LifecycleManager | All subsystems |
| `ai-os-system` | FileSystemProvider, SystemEventCollector | Sensor implementations |
| `ai-os-memory` | MemoryRetriever for entity resolution | ContextEnricher |
| `ai-os-runtime` | SessionManager for session resolution | ContextEnricher |
| `tokio` | Async runtime, I/O (stdin, TcpListener, UnixListener, signal) | Sensor layer |
| `tokio-util` | `ReaderStream` adapters for byte-level sensor I/O | Sensor layer |
| `serde` / `serde_json` | Percept serialization for event dispatch | All subsystems |
| `inotify` | File event sensors | FileWatcher sensor |
| `chrono` | Timestamps and temporal context | Normalizer, Enricher |
| `uuid` | Percept identifiers | Classifier |
| `regex` | Classification rule pattern matching | Classifier |
| `thiserror` | Error type derivation | All subsystems |

---

## Events Published

| Event | Type String | Subsystem | Trigger |
|---|---|---|---|
| `SensorStarted` | `perception.sensor.started` | Sensor | Sensor begins listening |
| `SensorStopped` | `perception.sensor.stopped` | Sensor | Sensor stops (normal or error) |
| `SensorError` | `perception.sensor.error` | Sensor | Sensor runtime failure |
| `RawInputReceived` | `perception.input.raw` | Sensor | Raw bytes received from source |
| `InputNormalized` | `perception.input.normalized` | Normalizer | Input validated and converted |
| `InputValidationFailed` | `perception.input.validation_failed` | Normalizer | Schema validation error |
| `PerceptClassified` | `perception.percept.classified` | Classifier | Percept category and priority assigned |
| `PerceptAttentionScored` | `perception.percept.attention_scored` | AttentionMechanism | Attention score computed |
| `PerceptDiscarded` | `perception.percept.discarded` | AttentionMechanism | Percept below attention threshold |
| `PerceptEnriched` | `perception.percept.enriched` | Enricher | Context entities and grounding attached |
| `EnrichedPerceptReady` | `perception.percept.ready` | Enricher | Final enriched percept published for consumers |

---

## Events Consumed

| Event | Source | Consumer | Purpose |
|---|---|---|---|
| `system.file.created` | FileSystemProvider | FileWatcher sensor | Forward to sensor input stream |
| `system.file.modified` | FileSystemProvider | FileWatcher sensor | Forward to sensor input stream |
| `system.file.deleted` | FileSystemProvider | FileWatcher sensor | Forward to sensor input stream |
| `system.device.added` | SystemEventCollector | SystemEvent sensor | Forward to sensor input stream |
| `system.device.removed` | SystemEventCollector | SystemEvent sensor | Forward to sensor input stream |
| `memory.item.retrieved` | MemoryRetriever | ContextEnricher | Warm entity cache for enrichment |

The Perception Platform bridges external events from the System Platform into its internal sensor pipeline. System events are consumed by dedicated sensor implementations that wrap the raw kernel event and re-enter the normalization-classification-enrichment pipeline.

---

## Thread Model

| Subsystem | Threading |
|---|---|
| `Sensor` instances | Each sensor runs on a dedicated Tokio task. Raw bytes are sent through an `mpsc::Sender<RawInput>` channel to the normalization stage. |
| `InputNormalizer` | Stateless. Runs on the sensor's task. Each `normalize()` call processes one `RawInput` and returns a `NormalizedInput`. |
| `Classifier` | `RwLock<Vec<ClassificationRule>>` for rule registry. Classification runs on the caller's task. Rules are evaluated sequentially (fast-path: first match wins). |
| `AttentionMechanism` | `RwLock<HashMap<(SensorType, PerceptCategory), HabituationState>>` for habituation tracking. Score computation on caller's task. |
| `ContextEnricher` | Performs entity extraction and session resolution. Entity lookups may call into Memory (async). Session resolution calls into Runtime SessionManager (async). Runs on caller's task. |

**Pipeline architecture**: Each percept flows through a sequential pipeline stages connected by `mpsc` channels:

```
Sensor Task -> [normalizer] -> [classifier] -> [attention] -> [enricher] -> EventBus
```

Each stage runs on the same Tokio task as the sensor, avoiding cross-task synchronization for a single percept's pipeline. The `attention` stage may discard percepts, terminating the pipeline early.

---

## Lifecycle

`PerceptionPlatform` implements Core's `Service` trait:

1. **Construction**: `PerceptionPlatform::new()` creates the normalizer, classifier, attention, and enricher instances. Sensors are registered via `register_sensor()` but not started.
2. **Start**: `start()` calls `sensor.start(tx)` for each registered sensor, spawning one Tokio task per sensor. Each sensor's transmit end of the `mpsc` channel feeds into the normalization pipeline. The start method then spawns the attention decay background task.
3. **Running**: Sensors produce `RawInput` asynchronously. The pipeline normalizes, classifies, scores, and enriches each percept. Enriched percepts are published to the EventBus. Discarded percepts are logged at debug level.
4. **Stop**: `stop()` signals each sensor to stop (via a cancellation token), awaits sensor task completion with a configurable grace period (default: 5 seconds), drains the pipeline channels, and unregisters all event subscriptions.

Start order within the Perception Platform:

```
1. Normalizer       (schemas loaded from configuration)
2. Classifier       (classification rules loaded)
3. Attention        (habituation state initialized)
4. Enricher         (entity cache warmed from Memory)
5. Sensors          (started last; pipeline stages must be ready)
```

---

## Error Handling

`PerceptionError` is the unified error type:

| Variant | Subsystem | Condition |
|---|---|---|
| `SensorStartFailed(String)` | Sensor | Sensor initialization error (bind, open, permissions) |
| `SensorReadFailed(String)` | Sensor | I/O error on sensor input stream |
| `SensorChannelClosed` | Sensor | Pipeline receiver dropped |
| `ValidationFailed(String)` | Normalizer | Payload fails schema validation |
| `SanitizationFailed(String)` | Normalizer | Encoding conversion error |
| `ClassificationFailed(String)` | Classifier | No matching rule, fallback to Unknown |
| `AttentionEvaluationFailed(String)` | AttentionMechanism | Internal state corruption |
| `EntityExtractionFailed(String)` | Enricher | Memory lookup timeout |
| `SessionResolutionFailed(String)` | Enricher | Session lookup error |
| `UnsupportedSensorType(String)` | Sensor | Sensor type not implemented |
| `Core(CoreError)` | All subsystems | Error from Core EventBus or Logger |
| `System(SystemError)` | FileWatcher, SystemEvent sensors | Error from System Platform |

**Recovery strategies**:

- `SensorReadFailed` retries the read operation up to 3 times with exponential backoff. If all retries fail, the sensor is stopped and a `SensorStopped` event is published.
- `SensorStartFailed` publishes a `SensorError` event. The platform continues with remaining sensors. The failed sensor can be restarted via an admin event.
- `ValidationFailed` publishes the raw input to a dead-letter channel and continues processing the next input. Validation errors are aggregated for operator review.
- `ClassificationFailed` assigns `PerceptCategory::Unknown` and `PerceptPriority::Low`. The percept continues through the pipeline but is tagged for operator review.

---

## Data Flow: Full Perception Pipeline

```
Sensor Task              Normalizer             Classifier
    |                        |                      |
    | RawInput               |                      |
    |--- (mpsc) ------------>|                      |
    |                        | validate schema      |
    |                        | sanitize payload     |
    |                        | NormalizedInput      |
    |                        |--- (return) -------->|
    |                        |                      |
    |                        |                      | match classification rules
    |                        |                      | assign category + priority
    |                        |                      | Percept
    |                        |                      |--- (return) -------->|
    |                        |                      |                      |

    AttentionMechanism       ContextEnricher        EventBus
           |                      |                    |
           |<---------------------|                    |
           |                      |                    |
           | compute score        |                    |
           | compare threshold    |                    |
           |                      |                    |
           | if score < threshold:|                    |
           |   PerceptDiscarded   |                    |
           |---- (publish) -------------------------->|
           |                      |                    |
           | if score >= threshold:|                    |
           |--- (return) -------->|                    |
           |                      | entity extraction  |
           |                      | session resolution |
           |                      | temporal/spatial   |
           |                      | EnrichedPercept    |
           |                      |--- (publish) ---->|
           |                      |                    |
```

---

## Future Extensions

- **Hot-Swappable Sensors**: Allow sensors to be registered, started, stopped, and unregistered at runtime via admin events, without restarting the Perception Platform.
- **Sensor Fusion**: Combine outputs from multiple sensors into a single fused percept with consolidated confidence scoring, reducing duplicate events from overlapping sources.
- **Adaptive Attention**: Train the attention model using reinforcement learning from Brain feedback (which percepts led to successful outcomes), dynamically adjusting thresholds and weights.
- **Multiplexed Sensors**: Support a single sensor instance serving multiple pipeline stages with different normalization rules, enabling parallel classification paths for the same input.
- **Input Streaming**: Add support for streaming inputs (long-running stdin reads, WebSocket frames) that produce multiple percepts per connection, with session-scoped state between frames.
- **Encrypted Sensor Channels**: Support TLS/DTLS for network socket sensors and authenticated IPC channels, with certificate validation and session binding.
- **Predictive Attention**: Use historical percept patterns to predict upcoming high-priority inputs and pre-warm the enrichment cache or pre-allocate processing capacity.
- **Percept Logging and Replay**: Record all raw inputs and enriched percepts to a ring buffer for debugging. Support replaying a recorded input sequence through the pipeline for testing.

---

## References

- [Architecture Overview](overview.md)
- [Core Platform Deep Dive](core.md)
- [System Platform Blueprint](system.md)
- [Memory Platform Blueprint](memory.md)
- [Brain Platform Blueprint](brain.md)
- [Execution Platform Blueprint](execution.md)
- [Design Principles](../principles.md)
- [Roadmap](../roadmap.md)
