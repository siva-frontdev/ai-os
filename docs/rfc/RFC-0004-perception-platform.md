# RFC-0004: Perception Platform

| Field | Value |
|---|---|
| **Status** | Draft |
| **Author** | AI-OS Architecture Team |
| **Phase** | 7 |
| **Created** | 2026-07-20 |
| **Updated** | 2026-07-20 |
| **Requires** | RFC-0001 (OSAL), RFC-0002 (Memory), RFC-0003 (Brain) |
| **Supersedes** | None |

## Abstract

The Perception Platform provides AI-native OS with a unified sensory pipeline that transforms raw observations from operating system sources, applications, users, files, terminals, network interfaces, and future multimodal inputs into structured, prioritized, context-enriched observations that the Brain Platform can reason about. It implements a configurable processing pipeline: normalization, filtering, entity extraction, context enrichment, state detection, anomaly detection, and observation fusion. The Perception Platform is the boundary between raw system data and cognitive processing.

## Motivation

Without a Perception Platform, the Brain Platform would need to directly ingest raw, heterogeneous data from diverse sources — raw file system events, terminal escape sequences, unstructured network packets, and binary sensor data. This coupling violates clean architecture, makes the Brain Platform dependent on low-level data format details, and prevents systematic attention filtering, entity resolution, and anomaly detection.

The Perception Platform provides:

1. **Uniform observation model** — Every perception source produces the same `Observation` type, regardless of the underlying modality or protocol.
2. **Attention-based filtering** — Prevents cognitive overload by discarding low-value observations before they reach the Brain.
3. **Context enrichment** — Resolves raw identifiers (PIDs, file descriptors, socket addresses) into meaningful entities (processes, sessions, users, files) before the Brain sees them.
4. **Anomaly detection** — Identifies statistically unusual observations at the perception layer, enabling early warning without Brain involvement.
5. **Observation fusion** — Combines correlated observations from multiple sources into a single synthesized observation, reducing noise and strengthening confidence.

## Design

### Crate Structure

```
brain/                        (Layer 6 — Consumer)
  |
  v
perception/                   (Layer 5 — This RFC)
  +-- perception-core/        Foundation types, traits, errors, observation model
  +-- perception-observer/    Sensor abstraction, raw data collection
  +-- perception-normalizer/  Schema validation, format conversion, sanitization
  +-- perception-detector/    State detection, change detection
  +-- perception-context/     Context enrichment, entity resolution
  +-- perception-entities/    Entity extraction, knowledge graph linkage
  +-- perception-anomaly/     Statistical anomaly detection, threshold monitoring
  +-- perception-fusion/      Observation correlation, synthesis, conflict resolution
  +-- perception-coordinator/ Top-level orchestration, pipeline wiring, lifecycle
  |
  v
memory/                       (Layer 4 — Entity resolution, pattern lookup)
crates/                       (Layer 3 — Runtime: sessions, context, permissions)
system/                       (Layer 2 — OSAL: filesystem, process, network, terminal)
core/                         (Layer 1 — EventBus, Service, Logger, Config)
```

### Dependency Graph

```
perception-core (no deps within perception)
  ├── perception-observer (core, Runtime, OSAL)
  ├── perception-normalizer (core)
  ├── perception-detector (core, normalizer)
  ├── perception-context (core, observer, memory)
  ├── perception-entities (core, normalizer, memory)
  ├── perception-anomaly (core, detector, normalizer)
  ├── perception-fusion (core, observer, normalizer, anomaly)
   └── perception-coordinator (all perception crates, Brain⸝, Runtime, Memory)

⸝ Brain dependency is an upward cross-layer bridge that should be eliminated during implementation via EventBus. See dependency table note.
```

### Observation Model

Every perception source produces an `Observation` — the canonical unit of perception in the system:

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

pub struct ObservationSource {
    pub observer_id: String,
    pub observer_kind: ObserverKind,
    pub instance_id: String,
    pub hostname: String,
}

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

pub enum ObservationPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

pub struct Provenance {
    pub raw_bytes: Option<Vec<u8>>,
    pub original_format: String,
    pub transformation_log: Vec<TransformationStep>,
}

pub struct QualityScore {
    pub completeness: f32,
    pub timeliness: f32,
    pub signal_to_noise: f32,
    pub overall: f32,
}
```

### Processing Pipeline

```
Raw Source ──→ Observer ──→ Normalizer ──→ Attention Filter ──→ Entity Extractor
                                                                   │
                                                                   v
Brain ←── Fusion Engine ←── Anomaly Detector ←── State Detector ←── Context Enricher
```

Each stage is a trait that consumes the previous stage's output and produces input for the next. Stages communicate through bounded `tokio::sync::mpsc` channels. The pipeline is assembled at startup by `PerceptionCoordinator` and can be reconfigured at runtime via EventBus commands.

### Published Events

| Event | Type String | Trigger |
|---|---|---|
| `ObservationReceived` | `perception.observation.received` | Raw observation ingested by Observer |
| `ObservationNormalized` | `perception.observation.normalized` | Observation passed schema validation |
| `ObservationRejected` | `perception.observation.rejected` | Observation failed validation |
| `ObservationFiltered` | `perception.observation.filtered` | Observation below attention threshold |
| `EntitiesExtracted` | `perception.entities.extracted` | Entity references resolved |
| `EntityResolutionFailed` | `perception.entities.resolution_failed` | Knowledge graph lookup failed |
| `ContextEnriched` | `perception.context.enriched` | Temporal/spatial/session context attached |
| `SessionResolutionFailed` | `perception.context.session_failed` | Session lookup failed |
| `StateChanged` | `perception.state.changed` | Detected state transition |
| `StateMachineConflict` | `perception.state.conflict` | Conflicting state transitions detected |
| `AnomalyDetected` | `perception.anomaly.detected` | Statistical outlier identified |
| `AnomalyBaselineUpdated` | `perception.anomaly.baseline` | Statistical baseline recomputed |
| `ObservationFused` | `perception.fusion.complete` | Multiple observations synthesized into one |
| `FusionWindowExpired` | `perception.fusion.window_expired` | Correlation window closed with incomplete set |
| `ObservationReady` | `perception.observation.ready` | Final enriched observation dispatched to Brain |
| `PipelineStageFailed` | `perception.pipeline.stage_failed` | Stage encountered unrecoverable error |
| `PipelineBackpressure` | `perception.pipeline.backpressure` | Channel capacity exceeded at any stage boundary |
| `PipelineReconfigured` | `perception.pipeline.reconfigured` | Pipeline config reloaded at runtime |
| `ObserverStatusChanged` | `perception.observer.status` | Observer started, stopped, or faulted |

### Consumed Events

| Source | Event | Handler |
|---|---|---|
| OSAL `osal-core` | `osal.fs_modified`, `osal.fs_created`, `osal.fs_deleted` | FileSystemObserver |
| OSAL `osal-core` | `osal.process_spawned`, `osal.process_exited` | ProcessObserver |
| OSAL `osal-core` | `osal.terminal_input`, `osal.terminal_resize` | TerminalObserver |
| OSAL `osal-core` | `osal.network_connect`, `osal.network_disconnect` | NetworkObserver |
| Core | `core.config_changed` | Coordinator | Reload pipeline configuration |
| Brain | `brain.attention_request` | Coordinator | Adjust attention thresholds |
| Runtime | `runtime.session_created`, `runtime.session_destroyed` | ContextEnricher | Warm session cache |

### Dependencies

| Crate | Depends On | Purpose |
|---|---|---|
| `perception-core` | `ai-os-core`, `memory-core` | EventBus, Service, Timestamp, MemoryId, Confidence |
| `perception-observer` | `perception-core`, `ai-os-runtime`, OSAL crates | Sensor trait, OSAL event consumers |
| `perception-normalizer` | `perception-core` | Schema validation, format detection |
| `perception-detector` | `perception-core`, `perception-normalizer` | State machine definitions, change detection |
| `perception-context` | `perception-core`, `perception-observer`, `memory-core` | Session resolution, temporal/spatial context |
| `perception-entities` | `perception-core`, `perception-normalizer`, `memory-knowledge` | Entity extraction, knowledge graph lookup |
| `perception-anomaly` | `perception-core`, `perception-detector`, `perception-normalizer` | Statistical models, threshold monitoring |
| `perception-fusion` | `perception-core`, `perception-observer`, `perception-normalizer`, `perception-anomaly` | Correlation engine, conflict resolution |
| `perception-coordinator` | All perception crates, `ai-os-runtime`, `brain-core`⸝ | Pipeline assembly, lifecycle, health |

⸝ `brain-core` is listed as a dependency for the initial bridge. This is an upward dependency (Perception Layer 5 → Brain Layer 6) and should be eliminated during implementation by using EventBus communication: `perception-coordinator` publishes `ObservationReady` events on the EventBus, and Brain subscribes. The `brain-core` dependency is a placeholder to be removed.

### Configuration

See `docs/configuration/perception.md` for the complete `perception.toml` specification.

Key configuration domains:
- `[observers]` — Observer definitions (kind, bind address, buffer sizes)
- `[normalizers]` — Schema registry, encoding rules, size limits
- `[attention]` — Global and per-modality thresholds, habituation decay
- `[entities]` — Entity extractor plugins, knowledge graph connection
- `[anomaly]` — Detection windows, sensitivity per modality
- `[fusion]` — Correlation windows, confidence thresholds
- `[pipeline]` — Stage ordering, channel sizes, timeouts

### Thread Model

| Subsystem | Concurrency Model |
|---|---|
| Observer instances | One Tokio task per observer; raw data sent through `mpsc` channel |
| Normalizer | Stateless; runs on observer's task per observation |
| Filter | `RwLock<HashMap<Modality, AttentionState>>`; runs on caller's task |
| Entity Extractor | Async calls to Memory; runs on caller's task |
| Context Enricher | Async session resolution; runs on caller's task |
| State Detector | `RwLock<HashMap<Scope, StateMachine>>`; runs on pipeline task |
| Anomaly Detector | `RwLock<SlidingWindowStats>` per modality; runs on pipeline task; heavy computation uses `spawn_blocking` |
| Fusion Engine | `RwLock<CorrelationWindow>`; background timer for window expiry |
| Coordinator | Own Tokio task for lifecycle management; delegates to pipeline tasks |

All inter-stage channels are bounded (`tokio::sync::mpsc::channel`) with configurable capacities. When a channel is full, the sender applies backpressure: either blocks (for critical-priority observers) or drops the oldest observation (for background-priority).

### Lifecycle

Start order:
```
1. perception-core types registered (no-op, types only)
2. perception-observer (schema loaded, entity caches warmed)
3. perception-normalizer (format parsers registered)
4. perception-detector (state machines initialized)
5. perception-context (session cache populated from Runtime)
6. perception-entities (entity index loaded from Memory)
7. perception-anomaly (baseline statistics computed from historical data)
8. perception-fusion (correlation windows initialized)
9. perception-coordinator (pipeline assembled, observers started)
```

Shutdown order (reverse):
```
1. perception-coordinator stops all observers, drains pipeline
2. perception-fusion flushes remaining correlation windows
3. perception-anomaly persists baseline statistics
4. perception-entities flushes entity cache
5. perception-context clears session cache
6. perception-detector persists state machines
7. perception-normalizer flushes parser registry
8. perception-observer stops sensor listeners
```

### Error Handling

| Error Variant | Subsystem | Recovery |
|---|---|---|
| `ObserverBindFailed(String)` | Observer | Retry with backoff (3 attempts), then emit `SensorError` event |
| `ObserverChannelClosed` | Observer | Recreate channel, restart observer task |
| `ValidationFailed(String)` | Normalizer | Emit `ObservationRejected` event, continue pipeline |
| `UnsupportedFormat(String)` | Normalizer | Skip observation, log warning, continue |
| `AttentionOverload` | Filter | Drop lowest-priority observation, emit `PipelineBackpressure` |
| `EntityResolutionFailed(String)` | Entities | Return empty entity set, log warning, observation continues |
| `SessionResolutionTimeout` | Context | Return session-agnostic context, observation continues |
| `StateMachineConflict(String)` | Detector | Log conflict, maintain current state |
| `AnomalyModelNotReady` | Anomaly | Return neutral score, observation continues |
| `CorrelationWindowFull` | Fusion | Force-flush oldest window, emit warning |
| `PipelineStageTimeout(String)` | Coordinator | Terminate stage task, restart with backoff |
| `Core(crate::CoreError)` | All | Propagate; fatal if EventBus unavailable |

All errors are non-fatal at the platform level. The Perception Coordinator will restart individual pipeline stages on failure. A stage that fails more than 5 times within 60 seconds transitions to a bypass mode where observations pass through without processing.

### Security Considerations

1. **Observation filtering** — Observers filter at the source: no raw observation contains data from outside the observer's permission scope. Observer permissions are enforced by the Runtime's `PermissionChecker`.
2. **Entity resolution** — Entity extractors only return entities the caller is permitted to see. Knowledge graph lookups pass through the Memory Platform's access control.
3. **Anomaly data isolation** — Anomaly detection windows are scoped per-session. No cross-session data leakage.
4. **Observation retention** — Raw observations are not retained beyond pipeline processing unless explicitly configured. The `Provenance::raw_bytes` field is stripped after normalization by default.
5. **Denial of service** — Bounded channels and per-modality attention thresholds prevent any single observer from flooding the pipeline.

### Performance Targets

| Operation | Target Latency (p50/p99) | Sustained Throughput |
|---|---|---|
| Observe (file/process) | 1 µs / 10 µs | 500K obs/s per observer |
| Normalize (structured, 1 KB) | 2 µs / 5 µs | 200K obs/s |
| Normalize (unstructured, 1 KB) | 10 µs / 50 µs | 100K obs/s |
| Filter (attention scoring) | 500 ns / 2 µs | 1M obs/s |
| Entity extract (text, 1 KB) | 20 µs / 100 µs | 50K obs/s |
| Context enrich | 5 µs / 50 µs | 100K obs/s |
| State detect | 1 µs / 5 µs | 200K obs/s |
| Anomaly detect | 5 µs / 20 µs | 100K obs/s |
| Fusion (pair) | 2 µs / 10 µs | 100K obs/s |
| End-to-end (hot path, all stages) | 50 µs / 250 µs | 20K obs/s |
| End-to-end (cold path, with entity resolution) | 200 µs / 1 ms | 5K obs/s |

### Testing Strategy

1. **Unit tests** — Every trait method tested in isolation with mock downstream stages. Attention scoring, state machine transitions, anomaly detection math, fusion correlation logic.
2. **Pipeline integration tests** — Full pipeline with mock observers: verify observation flows end-to-end, backpressure propagates correctly, errors in one stage do not crash other stages.
3. **Property-based tests** — Observation serialization round-trips, state machine transition validity, fusion commutativity.
4. **Benchmarks** — Per-stage throughput/latency benchmarks (Criterion), full pipeline end-to-end benchmarks with synthetic load at varying priority mixes.
5. **Chaos tests** — Random stage failures, channel closures, observer disconnects — verify coordinator recovers all stages.

## Drawbacks

1. **Pipeline latency** — Each stage adds latency. For latency-critical observations (e.g., real-time terminal echo), the pipeline may need a bypass path. This is addressed by configurable stage ordering — stages can be skipped or set to passthrough mode.
2. **Complexity** — Nine crates is more than a monolithic design. However, the crate boundaries align with clear architectural boundaries and enable independent testing and evolution of each concern.
3. **Memory overhead** — Observation metadata (provenance, entity lists, transformation log) can exceed the payload size. The `Provenance` raw bytes are dropped after normalization by default. Metadata size is capped per configuration.

## Alternatives Considered

| Alternative | Reason for Rejection |
|---|---|
| Monolithic `perception` crate with internal modules | Violates single-responsibility principle; no independent testability of stages; harder to evolve. |
| EventBus-only pipeline (no mpsc channels) | No backpressure mechanism; observer could flood Brain with events. Bounded channels provide natural throttling. |
| Push-based pipeline (observer pushes to next stage) | Pull-based (next stage requests from previous via Stream trait) would add complexity without benefit for the linear pipeline. Push with backpressure is simpler and sufficient. |
| In-process ML for anomaly detection | ML models add latency, dependency weight, and cold-start problems. Statistical methods (rolling mean/variance, Z-score, Holt-Winters) cover the initial use cases with lower overhead. ML-based anomaly detection can be added as a plugin later. |

## Open Questions

1. Should the pipeline support branching (one observation → multiple parallel enrichment paths that are later merged)? Current design assumes a linear pipeline. A branch-merge extension could be added in a future RFC.
2. What is the exact interface between Perception and Brain for `ObservationReady` events? Should Perception push observations to Brain, or should Brain pull from a Perception query interface? Current design pushes, but a hybrid (push for critical, poll for background) could be considered.
3. Should the fusion engine support user-defined correlation rules expressed in a DSL? The initial implementation uses configurable time-window + modality matching. A rule engine could be added later.

## Implementation Plan

1. **Phase 1 — Core types** (effort: medium)
   - `perception-core`: Observation model, error types, event types, basic traits (ObservationProvider, ObservationStore)
   - All types must round-trip through Serde
   - Estimated: 2-3 sprints

2. **Phase 2 — Observer layer** (effort: medium)
   - `perception-observer`: Observer trait, ObserverConfig, `ObservationReceived` event
   - Default observers: FileSystemObserver, ProcessObserver, TerminalObserver
   - Integration tests with mock OSAL event streams
   - Estimated: 2-3 sprints

3. **Phase 3 — Normalizer + Filter** (effort: medium)
   - `perception-normalizer`: Schema registry, format parsers (JSON, text, binary), sanitization rules
   - `perception-detector`: State machine definitions, change detection
   - Filter/attention mechanism with habituation
   - Benchmarks for throughput targets
   - Estimated: 2-3 sprints

4. **Phase 4 — Enrichment** (effort: medium)
   - `perception-context`: Temporal/spatial context attachment, session resolution
   - `perception-entities`: Entity extraction patterns, knowledge graph queries
   - Integration tests with Memory Platform
   - Estimated: 2-3 sprints

5. **Phase 5 — Anomaly + Fusion** (effort: large)
   - `perception-anomaly`: Statistical detectors (Z-score, rolling window, Holt-Winters)
   - `perception-fusion`: Correlation engine, observation synthesis, conflict resolution
   - Property-based tests for fusion correctness
   - Estimated: 3-4 sprints

6. **Phase 6 — Coordinator** (effort: medium)
   - `perception-coordinator`: Pipeline assembly, lifecycle management, health checks
   - Runtime reconfiguration via EventBus
   - Integration with Brain Platform (`ObservationReady` → Brain ingestion)
   - Chaos tests for stage failure recovery
   - Estimated: 2-3 sprints

## Unresolved Topics

- **Multimodal sensor fusion** — Combining observations from different modalities (e.g., terminal input + file system change) into a single fused observation requires a correlation rule DSL. This is deferred to post-initial implementation.
- **Observation retention policies** — How long to retain raw observations for debugging/replay. Ring-buffer storage with configurable size and retention period will be addressed in a future RFC.
- **Cross-node observation distribution** — In a multi-node deployment, observations from one node may be relevant to another. This requires distributed correlation IDs and a shared observation bus, deferred to the Intelligence Integration phase.

## References

- [Architecture Overview](../architecture/overview.md)
- [Perception Platform Architecture](../architecture/perception.md)
- [Perception Platform Interfaces](../interfaces/perception.md)
- [Perception Platform Configuration](../configuration/perception.md)
- [Perception Pipeline Design](../perception-pipeline.md)
- [Memory Platform Architecture](../architecture/memory.md)
- [Brain Platform Architecture](../architecture/brain.md)
- [OSAL Architecture](../architecture/system.md)
- RFC-0001: System Platform
- RFC-0002: Memory Platform
- RFC-0003: Brain Platform
