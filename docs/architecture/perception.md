# Perception Platform Architecture

## Purpose

The Perception Platform (Phase 7) is the sensory layer of the AI-native OS. It transforms raw observations from operating system sources, applications, users, files, terminals, network interfaces, hardware sensors, and future multimodal inputs into structured, prioritized, context-enriched observations that the Brain Platform can reason about.

The Perception Platform sits between the **Execution Platform / OSAL** (below) and the **Brain Platform** (above). It depends on the **Memory Platform** for entity resolution and pattern lookup, on **Runtime** for session and context management, and on **Core** for EventBus, Service lifecycle, logging, and configuration.

---

## Design Principles

1. **Structured Observation Model** — Every perception source produces the same `Observation` type regardless of underlying modality. All observations share a canonical structure: timestamp, source, confidence, modality, priority, payload, provenance, and quality metadata.

2. **Linear Pipeline with Backpressure** — Observations flow through a linear sequence of processing stages connected by bounded `mpsc` channels. Each stage has a configurable capacity. When a channel is full, backpressure propagates to the observer, which either blocks (critical priority) or drops (background priority).

3. **Attention-Based Filtering** — Not every observation reaches the Brain. Configurable per-modality attention thresholds with habituation decay prevent cognitive overload. Repeated identical observations receive decreasing attention scores.

4. **Fail-Stage, Not Fail-Pipeline** — A failure in any single pipeline stage does not crash the pipeline. The coordinator restarts the failed stage. If a stage fails repeatedly, the pipeline enters bypass mode for that stage — observations pass through unprocessed.

5. **Enrich Before Reasoning** — Entity resolution (PID → process name, FD → file path, socket → connection) and context attachment (session, temporal, spatial) happen in the Perception layer so the Brain receives self-contained, fully-resolved observations.

6. **Observable Pipeline** — Every stage emits diagnostic events (`perception.*`). Stage latencies, backpressure events, and drop rates are exposed as metrics. The pipeline is self-monitoring.

---

## Crate Structure

```
perception/
  +-- perception-core/           Foundation: Observation, errors, events, base traits
  +-- perception-observer/       Observer trait, OSAL event adapters, raw collection
  +-- perception-normalizer/     Schema validation, format detection, sanitization
  +-- perception-detector/       State machines, change detection, transition events
  +-- perception-context/        Session resolution, temporal/spatial context attachment
  +-- perception-entities/       Entity extraction patterns, knowledge graph lookup
  +-- perception-anomaly/        Statistical anomaly detection, threshold monitoring
  +-- perception-fusion/         Observation correlation, synthesis, conflict resolution
  +-- perception-coordinator/    Pipeline assembly, lifecycle, health, reconfiguration
```

### Dependency Graph

```
perception-core
  ├── perception-observer (core, Runtime session, OSAL event types)
  ├── perception-normalizer (core)
  ├── perception-detector (core, normalizer)
  ├── perception-context (core, observer, memory retrieval)
  ├── perception-entities (core, normalizer, memory knowledge graph)
  ├── perception-anomaly (core, detector, normalizer)
  ├── perception-fusion (core, observer, normalizer, anomaly)
  └── perception-coordinator (all perception, brain-core, runtime, memory retrieval)
```

External dependencies:

| Crate | Depends On | Purpose |
|---|---|---|
| `perception-core` | `ai-os-core`, `memory-core` | EventBus, Service, Timestamp, MemoryId, Confidence, Serde |
| `perception-observer` | `perception-core`, `ai-os-runtime`, `osal-core`, `osal-filesystem`, `osal-network`, `osal-terminal` | Observer trait, OSAL event consumers, PermissionChecker |
| `perception-normalizer` | `perception-core` | Schema validation, format detection, encoding conversion |
| `perception-detector` | `perception-core`, `perception-normalizer` | State machine definitions, change detection logic |
| `perception-context` | `perception-core`, `perception-observer`, `memory-retrieval` | Session resolution, temporal/spatial context |
| `perception-entities` | `perception-core`, `perception-normalizer`, `memory-knowledge` | Entity extraction, knowledge graph queries |
| `perception-anomaly` | `perception-core`, `perception-detector`, `perception-normalizer` | Statistical models, sliding windows, threshold management |
| `perception-fusion` | `perception-core`, `perception-observer`, `perception-normalizer`, `perception-anomaly` | Correlation engine, time-window management, synthesis |
| `perception-coordinator` | All perception crates, `ai-os-runtime`, `brain-core` | Pipeline assembly, lifecycle, health, reconfiguration |

---

## Observation Model

### Canonical Observation Type

The `Observation` is the universal data structure produced by every pipeline stage and consumed by the Brain Platform.

```rust
pub struct Observation {
    pub id: ObservationId,
    pub timestamp: Timestamp,
    pub source: ObservationSource,
    pub modality: Modality,
    pub confidence: Confidence,
    pub priority: ObservationPriority,
    pub correlation_id: Option<CorrelationId>,
    pub parent_id: Option<ObservationId>,
    pub derived_ids: Vec<ObservationId>,
    pub payload: ObservationPayload,
    pub metadata: HashMap<String, String>,
    pub provenance: Provenance,
    pub quality: QualityScore,
}
```

**ObservationId** — UUID v7 (time-sortable), globally unique. Enables chronological ordering without a separate timestamp index.

**ObservationSource** — Identity of the observer that created the observation.

```rust
pub struct ObservationSource {
    pub observer_id: String,
    pub observer_kind: ObserverKind,
    pub instance_id: String,
    pub hostname: String,
}

pub enum ObserverKind {
    FileSystem,
    Process,
    Terminal,
    Network,
    SystemEvent,
    Hardware,
    TimeSeries,
    Custom(String),
}
```

**Modality** — The sensory channel or data domain the observation belongs to.

```rust
pub enum Modality {
    Terminal,
    FileSystem,
    Network,
    Process,
    UserInput,
    SystemEvent,
    Hardware,
    TimeSeries,
    Custom(String),
}
```

**ObservationPriority** — Urgency level that determines pipeline processing order and Brain attention.

```rust
pub enum ObservationPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}
```

**ObservationPayload** — The actual data, encoded as a structured type with a discriminator.

```rust
pub enum ObservationPayload {
    Text { content: String, encoding: String },
    Binary { content: Vec<u8>, mime_type: String },
    Structured { fields: HashMap<String, serde_json::Value> },
    Event { event_type: String, data: serde_json::Value },
    State { key: String, old_value: Option<String>, new_value: String },
    Metric { name: String, value: f64, unit: String },
}
```

**Provenance** — Chain of custody tracking every transformation applied to the observation.

```rust
pub struct Provenance {
    pub raw_bytes: Option<Vec<u8>>,
    pub original_format: String,
    pub transformation_log: Vec<TransformationStep>,
}

pub struct TransformationStep {
    pub stage: String,
    pub timestamp: Timestamp,
    pub description: String,
}
```

**QualityScore** — Multi-dimensional quality assessment computed during normalization.

```rust
pub struct QualityScore {
    pub completeness: f32,
    pub timeliness: f32,
    pub signal_to_noise: f32,
    pub overall: f32,
}
```

---

## Crate Responsibilities

### perception-core

**Responsibility**: Foundation types, error definitions, event types, and base traits that all other perception crates depend on. No behavioral implementation — types and traits only.

**Key traits**:
- `ObservationProvider` — Trait for sources that produce observations
- `ObservationStore` — Trait for persistence of observations

**Key types**:
- `Observation`, `ObservationId`, `ObservationSource`, `ObservationPayload`
- `ObservationPriority`, `Modality`, `ObserverKind`
- `Provenance`, `TransformationStep`, `QualityScore`
- `CorrelationId`, `ObservationFilter`
- `PerceptionError` — Unified error enum
- All event types (`ObservationReceived`, `ObservationNormalized`, etc.)

### perception-observer

**Responsibility**: Interface to external data sources. Each observer implementation subscribes to OSAL events or directly reads from a source (stdin, file watcher, network socket) and produces `Observation` instances.

**Key traits**:
- `Observer` — Register, start, stop, status query
- `ObservationPublisher` — Publish observations into the pipeline

**Key types**:
- `ObserverConfig`, `ObserverStatus`, `ObserverKind`
- `FileSystemObserver` — Wraps `osal.fs_*` events into observations
- `ProcessObserver` — Wraps `osal.process_*` events into observations
- `TerminalObserver` — Wraps `osal.terminal_*` events into observations
- `NetworkObserver` — Wraps `osal.network_*` events into observations

### perception-normalizer

**Responsibility**: Validate, sanitize, and convert raw observations into the canonical format. Schema validation ensures required fields are present. Format detection identifies the payload structure. Sanitization strips dangerous content, normalizes encoding, and enforces size limits.

**Key traits**:
- `Normalizer` — Validate and normalize observations
- `FormatDetector` — Detect payload format from raw bytes
- `SchemaRegistry` — Register and query validation schemas

**Key types**:
- `NormalizerConfig`, `Schema`, `FieldDef`, `FieldType`
- `ValidationResult` — Success or structured failure reason
- `FormatDescriptor` — MIME type, expected encoding, parser reference

### perception-detector

**Responsibility**: Track system state over time and emit observations when state transitions occur. Maintains per-scope state machines. Detects both discrete state changes and continuous value threshold crossings.

**Key traits**:
- `StateDetector` — Register scopes, detect transitions, query current state
- `ChangeDetector` — Detect value changes over time windows

**Key types**:
- `StateMachine`, `StateTransition`, `ScopeId`
- `ChangeEvent` — Before/after values, rate of change
- `DetectorConfig` — Per-scope state machine definitions

### perception-context

**Responsibility**: Enrich observations with temporal context (wall-clock, monotonic time, timezone, uptime), spatial context (hostname, working directory, network address), and session context (session ID, user identity, permission scope).

**Key traits**:
- `ContextEnricher` — Attach context to observations
- `SessionResolver` — Resolve PID or thread ID to session

**Key types**:
- `TemporalContext`, `SpatialContext`, `SessionContext`
- `ContextEnricherConfig`

### perception-entities

**Responsibility**: Extract references to known entities from observation payloads. Uses pattern matching (regex for file paths, PIDs, socket addresses) and knowledge graph queries to resolve raw identifiers into structured entity references.

**Key traits**:
- `EntityExtractor` — Extract entity references from payloads
- `EntityResolver` — Resolve entity references against knowledge graph

**Key types**:
- `EntityRef`, `EntityKind`, `EntityResolution`
- `ExtractionPattern` — Regex or structural pattern with entity type mapping
- `EntityExtractorConfig`

### perception-anomaly

**Responsibility**: Detect statistically unusual observations using configurable detection methods. Maintains per-modality sliding windows of recent observations. Computes anomaly scores based on deviation from rolling statistics. Emits `AnomalyDetected` observations when scores exceed thresholds.

**Key traits**:
- `AnomalyDetector` — Score observations for anomalousness
- `AnomalyModel` — Statistical model (Z-score, rolling window, Holt-Winters)

**Key types**:
- `AnomalyScore`, `AnomalyReport`, `AnomalyThreshold`
- `SlidingWindowStats` — Rolling mean, variance, min, max, count
- `AnomalyDetectorConfig`

### perception-fusion

**Responsibility**: Correlate related observations from multiple observers or modalities into a single fused observation. Maintains time-windowed correlation buffers. When a correlation completes (all expected observations received within the window), synthesizes a fused observation with consolidated confidence, merged provenance, and deduplicated entities.

**Key traits**:
- `FusionEngine` — Correlate, synthesize, resolve conflicts
- `CorrelationRule` — Define what observations should be fused

**Key types**:
- `FusedObservation`, `CorrelationKey`, `CorrelationWindow`
- `FusionConfig`

### perception-coordinator

**Responsibility**: Assemble and manage the perception pipeline. Registers all stages, wires them together with bounded channels, manages lifecycle (start/stop/reload), monitors stage health, handles reconfiguration, and provides the bridge to the Brain Platform.

**Key traits**:
- `PerceptionCoordinator` — Start, stop, reload, status
- `PipelineManager` — Stage registration, channel wiring, backpressure monitoring

**Key types**:
- `PipelineConfig`, `StageHandle`, `PipelineStatus`
- `CoordinatorConfig`

---

## Event Model

### Published Events

| Event | Type String | Stage | Trigger |
|---|---|---|---|
| `ObservationReceived` | `perception.observation.received` | Observer | Raw data ingested from source |
| `ObservationNormalized` | `perception.observation.normalized` | Normalizer | Passed schema validation and format conversion |
| `ObservationRejected` | `perception.observation.rejected` | Normalizer | Failed validation; includes rejection reason |
| `ObservationFiltered` | `perception.observation.filtered` | Filter | Attention score below threshold; includes score and reason |
| `EntitiesExtracted` | `perception.entities.extracted` | Entities | Entity references resolved from payload |
| `EntityResolutionFailed` | `perception.entities.resolution_failed` | Entities | Knowledge graph lookup failed or timeout |
| `ContextEnriched` | `perception.context.enriched` | Context | Temporal/spatial/session context attached |
| `SessionResolutionFailed` | `perception.context.session_failed` | Context | Session lookup failed |
| `StateChanged` | `perception.state.changed` | Detector | State machine transition detected |
| `StateMachineConflict` | `perception.state.conflict` | Detector | Conflicting state transitions detected |
| `AnomalyDetected` | `perception.anomaly.detected` | Anomaly | Observation exceeds anomaly threshold |
| `AnomalyBaselineUpdated` | `perception.anomaly.baseline` | Anomaly | Statistical baseline recomputed |
| `ObservationFused` | `perception.fusion.complete` | Fusion | Multiple observations synthesized into one |
| `FusionWindowExpired` | `perception.fusion.window_expired` | Fusion | Correlation window closed with incomplete set |
| `ObservationReady` | `perception.observation.ready` | Coordinator | Final observation dispatched to Brain |
| `PipelineStageFailed` | `perception.pipeline.stage_failed` | Coordinator | Stage encountered unrecoverable error |
| `PipelineBackpressure` | `perception.pipeline.backpressure` | Coordinator | Channel capacity exceeded at any stage boundary |
| `PipelineReconfigured` | `perception.pipeline.reconfigured` | Coordinator | Pipeline config reloaded at runtime |
| `ObserverStatusChanged` | `perception.observer.status` | Coordinator | Observer started, stopped, or faulted |

### Consumed Events

| Source | Event | Consumer | Purpose |
|---|---|---|---|
| `osal-core` | `osal.fs_created` | FileSystemObserver | Forward to observer input stream |
| `osal-core` | `osal.fs_modified` | FileSystemObserver | Forward to observer input stream |
| `osal-core` | `osal.fs_deleted` | FileSystemObserver | Forward to observer input stream |
| `osal-core` | `osal.process_spawned` | ProcessObserver | Forward to observer input stream |
| `osal-core` | `osal.process_exited` | ProcessObserver | Forward to observer input stream |
| `osal-core` | `osal.terminal_input` | TerminalObserver | Forward to observer input stream |
| `osal-core` | `osal.terminal_resize` | TerminalObserver | Forward to observer input stream |
| `osal-core` | `osal.network_connect` | NetworkObserver | Forward to observer input stream |
| `osal-core` | `osal.network_disconnect` | NetworkObserver | Forward to observer input stream |
| `core` | `core.config_changed` | Coordinator | Reload pipeline configuration |
| `brain` | `brain.attention_request` | Coordinator | Adjust attention thresholds per Brain feedback |
| `runtime` | `runtime.session_created` | Context | Warm session resolution cache |
| `runtime` | `runtime.session_destroyed` | Context | Evict session from cache |

---

## Thread Model

| Subsystem | Concurrency Model |
|---|---|
| Observer instances | One dedicated Tokio task per observer. Observers that wrap OSAL event subscriptions are driven by the OSAL event stream. Observers that directly read I/O (stdin, sockets) use `tokio::io` async reads. Raw observations sent via `mpsc::Sender` to the next stage. |
| Normalizer | Stateless per-observation transformation. Runs on the observer's task (inline after receive). No locks. Heavy format parsing (unstructured text > 64 KB) uses `spawn_blocking`. |
| Filter | `std::sync::RwLock<HashMap<Modality, AttentionState>>`. Habituation state updated per-observation. Lock held briefly for read-modify-write of the attention score. Runs on pipeline task. |
| Entity Extractor | Pattern matching is CPU-bound and lock-free. Knowledge graph queries are async (call to `memory-knowledge` via EventBus or direct trait call). Runs on pipeline task. |
| Context Enricher | Session resolution is async (call to Runtime `SessionManager`). Temporal/spatial context computation is lock-free. Runs on pipeline task. |
| State Detector | `std::sync::RwLock<HashMap<ScopeId, StateMachine>>`. Lock held for state transition validation and update. Runs on pipeline task. |
| Anomaly Detector | `std::sync::RwLock<HashMap<Modality, SlidingWindowStats>>`. Window updates are O(1) amortized. Heavy computations (Holt-Winters model fit) use `spawn_blocking` on a configurable interval. |
| Fusion Engine | `std::sync::RwLock<HashMap<CorrelationKey, CorrelationWindow>>`. Window insertions and lookups are O(1). Background Tokio task handles window expiry with `tokio::time::interval`. |
| Coordinator | Own Tokio task for lifecycle management. Monitors stage health via `watch` channels. Reconfiguration commands received via EventBus subscription. |

**Pipeline threading**: The pipeline is assembled from `mpsc` channels:

```
Observer (task 1) → mpsc → Normalizer (task 1, inline) → mpsc → Filter (task 1, inline) → mpsc
  → Entity Extractor (task 2) → mpsc → Context Enricher (task 2, inline) → mpsc
  → State Detector (task 3) → mpsc → Anomaly Detector (task 3, inline) → mpsc
  → Fusion Engine (task 4) → mpsc → Coordinator (ready dispatch)
```

Stages that are CPU-bound or that perform async I/O (entity extraction, context enrichment, anomaly detection) run on dedicated Tokio tasks. Lightweight stages (normalizer, filter, state detector) run inline on the preceding task to avoid unnecessary task switching.

**Backpressure**: All `mpsc` channels have configurable capacities (default: 10,000). When full:
- Critical/High priority: `send().await` blocks the sender (backpressure propagates to observer).
- Normal priority: Oldest observation in the buffer is dropped (SLI policy).
- Low/Background priority: Observation is dropped immediately with `try_send()`.

---

## Lifecycle

### Start Order

```
  1. perception-core                    (types registered with Container — no-op)
  2. perception-observer                (observer schemas loaded from config)
  3. perception-normalizer              (format parsers registered, schemas loaded)
  4. perception-detector                (state machines initialized from config)
  5. perception-context                 (session cache warmed from Runtime)
  6. perception-entities                (entity index loaded from Memory knowledge graph)
  7. perception-anomaly                 (baseline statistics computed from history)
  8. perception-fusion                  (correlation windows initialized)
  9. perception-coordinator             (pipeline assembled, observers started)
```

The coordinator performs pipeline assembly in its `start()` method:

1. Create bounded `mpsc` channels between each consecutive stage pair.
2. Spawn Tokio tasks for stages that require dedicated tasks (entity extraction, anomaly detection, fusion expiry).
3. Register OSAL event subscriptions and wire them to observer instances.
4. Start all observers.
5. Publish `platform.perception.started` event.

### Shutdown Order

Shutdown proceeds in reverse registration order with graceful drains:

```
  1. perception-coordinator            (stop all observers, drain pipeline, await task completion)
  2. perception-fusion                 (flush remaining correlation windows)
  3. perception-anomaly                (persist baseline statistics)
  4. perception-entities               (flush entity cache)
  5. perception-context                (clear session cache)
  6. perception-detector               (persist state machines)
  7. perception-normalizer             (flush parser registry)
  8. perception-observer               (close sensor listeners)
```

Each stage has a configurable drain timeout (default: 5 seconds). If a stage fails to drain within the timeout, remaining observations are discarded and the stage is force-stopped. The coordinator logs all discarded observations at warning level.

### Health Checks

The coordinator registers a health check with Core's `HealthMonitor`:

| Check | Probe | Frequency | Failure Action |
|---|---|---|---|
| Observer liveness | Verify observer task is not completed | 5 seconds | Restart observer |
| Channel capacity | Check any channel > 80% full | 5 seconds | Emit `PipelineBackpressure` warning |
| Stage latency | Measure p99 latency per stage | 10 seconds | Log warning if exceeded |
| Pipeline throughput | Count observations processed per second | 10 seconds | Log warning if below threshold |

---

## Configuration

The `perception.toml` file is loaded by the coordinator at startup. See `docs/configuration/perception.md` for the complete specification.

Key configuration sections:

```toml
[observers]
# One section per observer instance

[observers.fs_watcher]
kind = "FileSystem"
enabled = true
paths = ["/home", "/tmp", "/var/log"]
events = ["created", "modified", "deleted"]
buffer_size = 1024

[observers.terminal]
kind = "Terminal"
enabled = true
source = "stdin"

[normalizers]
default_encoding = "utf-8"
max_payload_bytes = 1048576
strict_validation = false

[normalizers.schemas]
# Schema definitions for structured payloads

[attention]
global_threshold = 0.3
habituation_decay = 0.95
novelty_bonus = 0.2

[attention.per_modality]
Terminal = { threshold = 0.5, decay = 0.9 }
FileSystem = { threshold = 0.2, decay = 0.98 }

[entities]
memory_graph_endpoint = "local"
cache_ttl_seconds = 300

[anomaly]
enabled = true
default_window_seconds = 60
default_sensitivity = 2.0

[anomaly.per_modality]
Network = { window_seconds = 300, sensitivity = 3.0 }
FileSystem = { window_seconds = 60, sensitivity = 2.0 }

[fusion]
enabled = true
default_window_ms = 100
confidence_threshold = 0.6

[pipeline]
stage_order = [
    "observer",
    "normalizer",
    "filter",
    "entity_extractor",
    "context_enricher",
    "state_detector",
    "anomaly_detector",
    "fusion",
]
channel_capacity = 10000
stage_timeout_ms = 5000
max_stage_restarts = 5
restart_window_seconds = 60
```

---

## Error Handling

### PerceptionError Hierarchy

```rust
pub enum PerceptionError {
    // Observer errors
    ObserverBindFailed(String),
    ObserverChannelClosed,
    ObserverPermissionDenied(String),
    ObserverReadFailed(String),

    // Normalizer errors
    ValidationFailed { observation_id: ObservationId, reason: String },
    UnsupportedFormat(String),
    SanitizationFailed(String),
    PayloadTooBig { size: u64, max: u64 },

    // Filter / Attention errors
    AttentionOverload,
    HabituationStateCorrupt(String),

    // Entity errors
    EntityResolutionFailed { entity_type: String, raw_value: String },
    EntityPatternNotFound(String),
    KnowledgeGraphTimeout,

    // Context errors
    SessionResolutionTimeout,
    SessionNotFound(String),
    TemporalContextError(String),

    // Detector errors
    StateMachineNotFound(ScopeId),
    StateMachineConflict { scope: ScopeId, from: String, to: String },
    InvalidStateTransition { scope: ScopeId, from: String, to: String },

    // Anomaly errors
    AnomalyModelNotReady(String),
    InsufficientData { modality: Modality, required: usize, available: usize },
    InvalidSensitivity(f64),

    // Fusion errors
    CorrelationWindowFull,
    FusionConflict { correlation_key: CorrelationKey, reason: String },
    CorrelationTimeout(CorrelationKey),

    // Coordinator / Pipeline errors
    PipelineStageTimeout(String),
    PipelineAssemblyFailed(String),
    ChannelClosed(String),
    ConfigurationError(String),

    // Wrapped errors
    Core(core::CoreError),
    Runtime(runtime::RuntimeError),
    Memory(memory_core::MemoryError),
    Osal(osal_core::OsalError),
}
```

### Recovery Strategies

| Error | Recovery |
|---|---|
| `ObserverBindFailed` | Retry with exponential backoff (100ms, 500ms, 2s). After 3 failures, emit `SensorError` and mark observer disabled. |
| `ObserverChannelClosed` | Recreate the channel pair, restart the observer task. Observations in flight are lost. |
| `ValidationFailed` | Emit `ObservationRejected` event with reason. Do not retry. Pipeline continues to next observation. |
| `UnsupportedFormat` | Skip observation, log warning, emit `ObservationRejected`. Pipeline continues. |
| `AttentionOverload` | Drop lowest-priority observation from the filter queue. Emit `PipelineBackpressure` warning. |
| `EntityResolutionFailed` | Return empty entity set. Observation continues through pipeline without entity enrichment. |
| `SessionResolutionTimeout` | Return session-agnostic context. Observation continues. |
| `StateMachineNotFound` | Create a new state machine in the default state for the scope. Observation is tagged with initial state. |
| `InvalidStateTransition` | Log warning, reject the transition, keep current state. Observation tagged with current state. |
| `AnomalyModelNotReady` | Return neutral score (0.5). Observation continues. Model initializes in background. |
| `InsufficientData` | Return neutral score. Continue accumulating data. |
| `CorrelationWindowFull` | Force-flush the oldest uncompleted window. Emit `FusionWindowExpired`. |
| `PipelineStageTimeout` | Terminate the stage task, restart with exponential backoff. If restarts exceed threshold (5 within 60s), enter bypass mode. |
| `ConfigurationError` | Reject configuration change, keep previous config. Emit error event. |

All errors are non-fatal at the platform level. The coordinator ensures the pipeline continues operating even when individual stages fail.

---

## Performance Targets

| Operation | p50 Latency | p99 Latency | Sustained Throughput |
|---|---|---|---|
| Observe (file/process event) | 1 µs | 10 µs | 500K obs/s per observer |
| Normalize (structured, 1 KB) | 2 µs | 5 µs | 200K obs/s |
| Normalize (unstructured, 1 KB) | 10 µs | 50 µs | 100K obs/s |
| Filter (attention scoring) | 500 ns | 2 µs | 1M obs/s |
| Entity extract (text, 1 KB, cached) | 5 µs | 20 µs | 100K obs/s |
| Entity extract (text, 1 KB, graph query) | 20 µs | 100 µs | 50K obs/s |
| Context enrich (cached session) | 2 µs | 10 µs | 200K obs/s |
| Context enrich (session lookup) | 5 µs | 50 µs | 100K obs/s |
| State detect (discrete) | 1 µs | 5 µs | 200K obs/s |
| State detect (continuous, threshold) | 2 µs | 10 µs | 100K obs/s |
| Anomaly detect (Z-score) | 2 µs | 10 µs | 200K obs/s |
| Anomaly detect (Holt-Winters) | 10 µs | 50 µs | 50K obs/s |
| Fusion (pair correlation) | 2 µs | 10 µs | 100K obs/s |
| Fusion (multi-observation synthesis) | 10 µs | 50 µs | 50K obs/s |
| **End-to-end (hot path, all stages, cached)** | 50 µs | 250 µs | 20K obs/s |
| **End-to-end (cold path, entity + session)** | 200 µs | 1 ms | 5K obs/s |

**Resource envelope** (per million observations/sec):
- CPU: ~2 cores (mostly normalization and entity extraction)
- Memory: ~500 MB (observation buffers, sliding windows, correlation windows)
- Network: 0 (all in-process)
- Channel capacity per stage: 10,000 observations (~50 MB at 5 KB avg)

---

## References

- [Architecture Overview](overview.md)
- [Perception Platform Interfaces](../interfaces/perception.md)
- [Perception Platform Configuration](../configuration/perception.md)
- [Perception Pipeline Design](../perception-pipeline.md)
- [RFC-0004: Perception Platform](../rfc/RFC-0004-perception-platform.md)
- [Memory Platform Architecture](memory.md)
- [Brain Platform Architecture](brain.md)
- [OSAL Architecture](system.md)
- [Runtime Platform Architecture](runtime.md)
- [Core Platform Architecture](core.md)
