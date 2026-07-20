# Perception Platform Interface — `perception` *(Phase 7)*

## Purpose

This document defines the public API surfaces of all Perception Platform crates. Each section covers one trait or major type group, its supporting types, and its complete definition.

The Perception Platform transforms raw observations from diverse sources into structured, prioritized, context-enriched observations for the Brain Platform. Its interfaces are designed for `dyn` compatibility (`Send + Sync + Debug`), async operation, and loose coupling through the EventBus.

---

## Data Model

### Observation — universal perception unit

```rust
/// Globally unique observation identifier (UUID v7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationId(uuid::Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Identity of the originating observer instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSource {
    pub observer_id: String,
    pub observer_kind: ObserverKind,
    pub instance_id: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ObservationPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

/// Structured payload discriminated by variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservationPayload {
    Text { content: String, encoding: String },
    Binary { content: Vec<u8>, mime_type: String },
    Structured { fields: HashMap<String, serde_json::Value> },
    Event { event_type: String, data: serde_json::Value },
    State { key: String, old_value: Option<String>, new_value: String },
    Metric { name: String, value: f64, unit: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub raw_bytes: Option<Vec<u8>>,
    pub original_format: String,
    pub transformation_log: Vec<TransformationStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformationStep {
    pub stage: String,
    pub timestamp: Timestamp,
    pub description: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QualityScore {
    pub completeness: f32,
    pub timeliness: f32,
    pub signal_to_noise: f32,
    pub overall: f32,
}

/// UUID v7 used for correlation between related observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(uuid::Uuid);

/// Alternative to ObservationId for querying raw observations from a store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationFilter {
    pub modality: Option<Modality>,
    pub min_priority: Option<ObservationPriority>,
    pub since: Option<Timestamp>,
    pub until: Option<Timestamp>,
    pub source_id: Option<String>,
    pub correlation_id: Option<CorrelationId>,
    pub limit: Option<usize>,
}
```

---

## 1. ObservationProvider — produce observations from sources

**Crate**: `perception-core`

**Purpose**: Trait implemented by any source that produces observations. Used by observer implementations to inject raw data into the pipeline.

```rust
#[async_trait]
pub trait ObservationProvider: Debug + Send + Sync {
    /// Return the provider's identity.
    fn source(&self) -> ObservationSource;

    /// Publish a single observation into the pipeline.
    /// Returns an error if the pipeline channel is full and the
    /// observation's priority does not qualify for blocking send.
    async fn publish(&self, observation: Observation) -> Result<(), PerceptionError>;

    /// Publish a batch of observations (more efficient for burst sources).
    async fn publish_batch(&self, observations: Vec<Observation>) -> Result<(), PerceptionError>;

    /// Register an observer kind with a pipeline sender.
    fn register_pipeline(&self, kind: ObserverKind, tx: mpsc::Sender<Observation>);
}
```

---

## 2. ObservationStore — persist observations

**Crate**: `perception-core`

**Purpose**: Optional persistence interface for observations that need to survive pipeline processing. Used by the coordinator to store observations for replay, debugging, or historical analysis.

```rust
#[async_trait]
pub trait ObservationStore: Debug + Send + Sync {
    /// Store a single observation.
    async fn store(&self, observation: &Observation) -> Result<(), PerceptionError>;

    /// Store a batch of observations atomically.
    async fn store_batch(&self, observations: &[Observation]) -> Result<(), PerceptionError>;

    /// Retrieve an observation by ID.
    async fn get(&self, id: &ObservationId) -> Result<Option<Observation>, PerceptionError>;

    /// Query observations matching the given filter.
    async fn query(&self, filter: &ObservationFilter) -> Result<Vec<Observation>, PerceptionError>;

    /// Delete observations older than the given timestamp.
    async fn prune(&self, older_than: Timestamp) -> Result<u64, PerceptionError>;
}
```

---

## 3. Observer — ingest raw data from external sources

**Crate**: `perception-observer`

**Purpose**: Trait for components that connect to external data sources (OSAL events, file watchers, network sockets, stdin) and produce `Observation` instances.

```rust
#[async_trait]
pub trait Observer: Debug + Send + Sync {
    /// Unique identifier for this observer instance.
    fn observer_id(&self) -> &str;

    /// The kind of observer.
    fn kind(&self) -> ObserverKind;

    /// Configuration for this observer.
    fn config(&self) -> &ObserverConfig;

    /// Start ingesting data. The observer begins producing observations
    /// and publishing them through the pipeline sender.
    async fn start(&self, tx: mpsc::Sender<Observation>) -> Result<(), PerceptionError>;

    /// Stop ingesting data. Drains any in-flight observations.
    async fn stop(&self) -> Result<(), PerceptionError>;

    /// Return current status.
    fn status(&self) -> ObserverStatus;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserverConfig {
    pub observer_id: String,
    pub kind: ObserverKind,
    pub enabled: bool,
    pub buffer_size: usize,
    pub priority: ObservationPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObserverStatus {
    Initialized,
    Running,
    Draining,
    Stopped,
    Faulted,
}
```

### ObservationPublisher — trait for pipeline injection

**Crate**: `perception-observer`

**Purpose**: Internal trait used by observers to publish observations into the pipeline. Implemented by the coordinator and injected into each observer.

```rust
#[async_trait]
pub trait ObservationPublisher: Debug + Send + Sync {
    /// Publish a single observation into the pipeline.
    async fn publish(&self, observation: Observation) -> Result<(), PerceptionError>;

    /// Publish a batch of observations.
    async fn publish_batch(&self, observations: Vec<Observation>) -> Result<(), PerceptionError>;

    /// Returns true if the pipeline channel is ready to accept observations.
    fn is_ready(&self) -> bool;
}
```

### Default observers (same crate)

```rust
/// Observes filesystem events via OSAL.
pub struct FileSystemObserver { /* ... */ }

/// Observes process lifecycle events via OSAL.
pub struct ProcessObserver { /* ... */ }

/// Observes terminal input events via OSAL.
pub struct TerminalObserver { /* ... */ }

/// Observes network connection events via OSAL.
pub struct NetworkObserver { /* ... */ }
```

Each default observer implements `Observer` and wraps the corresponding OSAL event stream (`osal.fs_*`, `osal.process_*`, `osal.terminal_*`, `osal.network_*`).

---

## 4. Normalizer — validate and canonicalize observations

**Crate**: `perception-normalizer`

**Purpose**: Validate observations against schema definitions, detect payload format, sanitize content, and convert to canonical representation.

```rust
#[async_trait]
pub trait Normalizer: Debug + Send + Sync {
    /// Normalize a single observation: validate, sanitize, convert.
    async fn normalize(&self, observation: Observation) -> Result<Observation, PerceptionError>;

    /// Normalize a batch of observations.
    async fn normalize_batch(&self, observations: Vec<Observation>) -> Result<Vec<Observation>, PerceptionError>;

    /// Register a validation schema for a specific modality.
    fn register_schema(&self, modality: Modality, schema: Schema) -> Result<(), PerceptionError>;

    /// Register a format parser for a MIME type.
    fn register_parser(&self, mime: &str, parser: Box<dyn FormatParser>);
}

/// A validation schema for structured observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub modality: Modality,
    pub fields: Vec<FieldDef>,
    pub max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    String, Integer, Float, Boolean, Object, Array,
}

/// Detects and parses a specific format from raw bytes.
#[async_trait]
pub trait FormatParser: Debug + Send + Sync {
    fn mime_type(&self) -> &str;
    async fn detect(&self, bytes: &[u8]) -> bool;
    async fn parse(&self, bytes: &[u8]) -> Result<ObservationPayload, PerceptionError>;
}

### FormatDetector — detect payload format from raw bytes

**Crate**: `perception-normalizer`

**Purpose**: Examines raw observation bytes and determines the payload format (JSON, text, binary). Used by the normalizer to select the appropriate parser.

```rust
#[async_trait]
pub trait FormatDetector: Debug + Send + Sync {
    /// Detect the format of raw bytes.
    /// Returns the MIME type and a confidence score.
    async fn detect(&self, bytes: &[u8]) -> Result<(String, f64), PerceptionError>;

    /// Register a format detection heuristic.
    fn register_detector(&self, mime: &str, detector: Box<dyn FormatDetectorFn>);
}

#[async_trait]
pub trait FormatDetectorFn: Debug + Send + Sync {
    async fn detect(&self, bytes: &[u8]) -> bool;
}
```

### SchemaRegistry — register and query validation schemas

**Crate**: `perception-normalizer`

**Purpose**: Manages a registry of validation schemas keyed by modality. Allows the normalizer to look up the schema for a given observation modality.

```rust
#[async_trait]
pub trait SchemaRegistry: Debug + Send + Sync {
    /// Register a schema for a modality.
    fn register(&self, modality: Modality, schema: Schema) -> Result<(), PerceptionError>;

    /// Look up the schema for a modality.
    fn get(&self, modality: &Modality) -> Option<Schema>;

    /// Remove a schema registration.
    fn unregister(&self, modality: &Modality) -> Result<(), PerceptionError>;

    /// List all registered modalities.
    fn modalities(&self) -> Vec<Modality>;
}
```

---

## 5. AttentionFilter — salience-based observation filtering

**Crate**: `perception-coordinator` (implemented in `perception-core` trait, logic in a dedicated submodule)

**Purpose**: Score observations by salience, applying habituation decay to suppress repeated identical observations. Observations below the configurable threshold are dropped before reaching later pipeline stages.

```rust
#[async_trait]
pub trait AttentionFilter: Debug + Send + Sync {
    /// Evaluate an observation's attention score.
    /// Returns Some(score) if the observation should proceed,
    /// None if it should be discarded.
    async fn evaluate(&self, observation: &Observation) -> Result<Option<AttentionScore>, PerceptionError>;

    /// Update habituation state for a given observation pattern.
    /// Called after evaluation to record that this observation was seen.
    fn record(&self, observation: &Observation);

    /// Reset habituation for a specific modality.
    fn reset_modality(&self, modality: &Modality);

    /// Return current filter configuration.
    fn config(&self) -> &AttentionConfig;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionScore {
    pub score: f64,
    pub is_novel: bool,
    pub habituation_count: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionConfig {
    pub global_threshold: f64,
    pub habituation_decay: f64,
    pub novelty_bonus: f64,
    pub per_modality: HashMap<Modality, PerModalityAttention>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerModalityAttention {
    pub threshold: f64,
    pub decay: f64,
}
```

---

## 6. EntityExtractor — resolve entities from observation payloads

**Crate**: `perception-entities`

**Purpose**: Extract references to known system entities (processes, files, users, sessions, network connections) from observation payloads using pattern matching and knowledge graph lookups.

```rust
#[async_trait]
pub trait EntityExtractor: Debug + Send + Sync {
    /// Extract entity references from an observation.
    /// Returns a list of resolved entity references.
    async fn extract(&self, observation: &Observation) -> Result<Vec<EntityRef>, PerceptionError>;

    /// Resolve a single raw identifier to an entity reference.
    async fn resolve(&self, kind: EntityKind, raw: &str) -> Result<Option<EntityRef>, PerceptionError>;

    /// Register an extraction pattern.
    fn register_pattern(&self, pattern: ExtractionPattern) -> Result<(), PerceptionError>;

    /// Warm the entity cache from the knowledge graph.
    async fn warm_cache(&self) -> Result<(), PerceptionError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    pub entity_id: String,
    pub entity_kind: EntityKind,
    pub label: String,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityKind {
    Process,
    File,
    User,
    Session,
    NetworkConnection,
    Device,
    Service,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionPattern {
    pub name: String,
    pub entity_kind: EntityKind,
    pub pattern: String,        // Regex
    pub modality: Modality,
}
```

### EntityResolver — resolve entities against the knowledge graph

**Crate**: `perception-entities`

**Purpose**: Resolves raw identifiers (PIDs, file paths, socket addresses) into canonical entity references by querying the Memory Platform's knowledge graph.

```rust
#[async_trait]
pub trait EntityResolver: Debug + Send + Sync {
    /// Resolve a raw identifier to an entity reference.
    async fn resolve(&self, kind: EntityKind, raw: &str) -> Result<Option<EntityRef>, PerceptionError>;

    /// Batch resolve multiple identifiers.
    async fn resolve_batch(&self, ids: Vec<(EntityKind, String)>) -> Result<Vec<Option<EntityRef>>, PerceptionError>;

    /// Warm the resolution cache from the knowledge graph.
    async fn warm_cache(&self) -> Result<(), PerceptionError>;
}
```

---

## 7. ContextEnricher — attach temporal, spatial, and session context

**Crate**: `perception-context`

**Purpose**: Augment observations with environmental context: wall-clock time, timezone, hostname, working directory, network address, and resolved session/identity information.

```rust
#[async_trait]
pub trait ContextEnricher: Debug + Send + Sync {
    /// Enrich an observation with temporal, spatial, and session context.
    async fn enrich(&self, observation: Observation) -> Result<Observation, PerceptionError>;

    /// Resolve a PID to a session reference.
    async fn resolve_session(&self, pid: u32) -> Result<Option<SessionContext>, PerceptionError>;

    /// Get the current temporal context.
    fn temporal_context(&self) -> TemporalContext;

    /// Get the current spatial context.
    fn spatial_context(&self) -> SpatialContext;
}

/// Temporal context attached to every enriched observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalContext {
    pub wall_clock: Timestamp,
    pub timezone: String,
    pub uptime_seconds: f64,
}

/// Spatial context attached to every enriched observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialContext {
    pub hostname: String,
    pub cwd: Option<String>,
    pub session_id: Option<String>,
    pub network_addr: Option<String>,
}

/// Session context resolved from the Runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub user_id: String,
    pub scope: String,
}
```

### SessionResolver — resolve operating system identity to session context

**Crate**: `perception-context`

**Purpose**: Resolves a process ID (PID) or thread ID to a Runtime session, providing user identity, permission scope, and associated session metadata.

```rust
#[async_trait]
pub trait SessionResolver: Debug + Send + Sync {
    /// Resolve a PID to a session context.
    async fn resolve_pid(&self, pid: u32) -> Result<Option<SessionContext>, PerceptionError>;

    /// Resolve a thread ID to a session context.
    async fn resolve_tid(&self, tid: u32) -> Result<Option<SessionContext>, PerceptionError>;

    /// Warm the session resolution cache from Runtime.
    async fn warm_cache(&self) -> Result<(), PerceptionError>;
}
```

---

## 8. StateDetector — track state transitions

**Crate**: `perception-detector`

**Purpose**: Track per-scope state machines. Detect and emit observations for state transitions. Supports both discrete state machines and continuous value change detection.

```rust
#[async_trait]
pub trait StateDetector: Debug + Send + Sync {
    /// Register a state machine definition for a scope.
    fn register_machine(&self, scope: ScopeId, machine: StateMachine) -> Result<(), PerceptionError>;

    /// Process an observation and detect state transitions.
    /// Returns the detected state transition, if any.
    async fn detect(&self, observation: &Observation) -> Result<Option<StateTransition>, PerceptionError>;

    /// Get the current state for a scope.
    fn current_state(&self, scope: &ScopeId) -> Result<Option<String>, PerceptionError>;

    /// Reset a scope's state machine to its initial state.
    fn reset(&self, scope: &ScopeId) -> Result<(), PerceptionError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScopeId(String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachine {
    pub name: String,
    pub initial_state: String,
    pub states: Vec<StateDefinition>,
    pub transitions: Vec<TransitionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDefinition {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionRule {
    pub from: String,
    pub to: String,
    pub condition: String,
    pub on_transition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub scope: ScopeId,
    pub from: String,
    pub to: String,
    pub triggered_by: ObservationId,
    pub timestamp: Timestamp,
}
```

---

## 9. ChangeDetector — continuous value change detection

**Crate**: `perception-detector`

**Purpose**: Detect changes in continuous values over time windows. Emit observations when values cross thresholds, change by a significant rate, or exhibit unusual patterns.

```rust
#[async_trait]
pub trait ChangeDetector: Debug + Send + Sync {
    /// Register a metric to monitor for changes.
    fn register_metric(&self, key: String, config: ChangeDetectorConfig) -> Result<(), PerceptionError>;

    /// Feed a value observation to the detector.
    /// Returns a ChangeEvent if a change is detected.
    async fn feed(&self, key: &str, value: f64, timestamp: Timestamp) -> Result<Option<ChangeEvent>, PerceptionError>;

    /// Get current statistics for a metric.
    fn stats(&self, key: &str) -> Result<Option<MetricStats>, PerceptionError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetectorConfig {
    pub window_seconds: u64,
    pub absolute_threshold: Option<f64>,
    pub relative_threshold: Option<f64>,
    pub rate_threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub key: String,
    pub old_value: f64,
    pub new_value: f64,
    pub rate: f64,
    pub detected_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub variance: f64,
    pub count: u64,
}
```

---

## 10. AnomalyDetector — statistical anomaly detection

**Crate**: `perception-anomaly`

**Purpose**: Score observations for anomalousness using configurable statistical models. Maintain per-modality sliding windows. Emit `AnomalyDetected` events when scores exceed thresholds.

```rust
#[async_trait]
pub trait AnomalyDetector: Debug + Send + Sync {
    /// Score an observation for anomalousness.
    /// Returns an AnomalyScore (0.0 = normal, 1.0 = definitely anomalous).
    async fn score(&self, observation: &Observation) -> Result<AnomalyScore, PerceptionError>;

    /// Feed an observation into the statistical model for baseline computation.
    async fn feed(&self, observation: &Observation) -> Result<(), PerceptionError>;

    /// Register a detection model for a modality.
    fn register_model(&self, modality: Modality, model: Box<dyn AnomalyModel>) -> Result<(), PerceptionError>;

    /// Get current anomaly statistics for a modality.
    fn stats(&self, modality: &Modality) -> Result<Option<AnomalyStats>, PerceptionError>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AnomalyScore {
    pub score: f64,
    pub threshold: f64,
    pub is_anomaly: bool,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyStats {
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub count: u64,
    pub anomaly_count: u64,
}

/// Pluggable anomaly detection model.
#[async_trait]
pub trait AnomalyModel: Debug + Send + Sync {
    fn name(&self) -> &str;
    async fn score(&self, value: f64) -> f64;
    async fn feed(&self, value: f64);
    async fn reset(&self);
}
```

---

## 11. FusionEngine — correlate and synthesize observations

**Crate**: `perception-fusion`

**Purpose**: Correlate related observations from multiple observers or modalities within a configurable time window. When all expected observations for a correlation key arrive, synthesize a single fused observation with consolidated confidence and merged provenance.

```rust
#[async_trait]
pub trait FusionEngine: Debug + Send + Sync {
    /// Register a correlation rule.
    fn register_rule(&self, rule: CorrelationRule) -> Result<(), PerceptionError>;

    /// Feed an observation into the fusion engine.
    /// Returns a FusedObservation if correlation completes.
    async fn feed(&self, observation: Observation) -> Result<Option<FusedObservation>, PerceptionError>;

    /// Flush all pending correlation windows (e.g., on shutdown).
    async fn flush(&self) -> Result<Vec<FusedObservation>, PerceptionError>;

    /// Get fusion statistics.
    fn stats(&self) -> FusionStats;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationRule {
    pub name: String,
    pub correlation_key_expr: String,
    pub expected_sources: Vec<ObserverKind>,
    pub window_ms: u64,
    pub min_observations: usize,
    pub fusion_fn: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusedObservation {
    pub correlation_key: CorrelationKey,
    pub sources: Vec<ObservationSource>,
    pub fused_payload: ObservationPayload,
    pub confidence: Confidence,
    pub synthesized_at: Timestamp,
    pub constituent_ids: Vec<ObservationId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationKey(String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionStats {
    pub active_windows: u64,
    pub completed_fusions: u64,
    pub expired_windows: u64,
    pub conflicts: u64,
}
```

---

## 12. PerceptionCoordinator — top-level orchestration

**Crate**: `perception-coordinator`

**Purpose**: Assemble the pipeline, manage lifecycle, monitor health, handle reconfiguration, and bridge to the Brain Platform.

```rust
/// The top-level coordinator implements Core's Service trait.
#[async_trait]
pub trait PerceptionCoordinator: Debug + Send + Sync {
    /// Register an observer with the coordinator.
    async fn register_observer(&self, observer: Arc<dyn Observer>) -> Result<(), PerceptionError>;

    /// Register a normalizer.
    async fn register_normalizer(&self, normalizer: Arc<dyn Normalizer>) -> Result<(), PerceptionError>;

    /// Start the perception pipeline.
    async fn start(&self) -> Result<(), PerceptionError>;

    /// Stop the perception pipeline gracefully.
    async fn stop(&self) -> Result<(), PerceptionError>;

    /// Reload configuration at runtime.
    async fn reload_config(&self, config: CoordinatorConfig) -> Result<(), PerceptionError>;

    /// Get pipeline status.
    async fn status(&self) -> PipelineStatus;

    /// Subscribe to the stream of enriched, ready-to-process observations.
    async fn observation_stream(&self) -> mpsc::Receiver<Observation>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub running: bool,
    pub stages: Vec<StageStatus>,
    pub total_observations_processed: u64,
    pub total_observations_dropped: u64,
    pub backpressure_events: u64,
    pub uptime_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageStatus {
    pub name: String,
    pub active: bool,
    pub bypass: bool,
    pub observations_in: u64,
    pub observations_out: u64,
    pub errors: u64,
    pub p99_latency_us: f64,
}
```

### PipelineManager — stage registration and channel wiring

**Crate**: `perception-coordinator`

**Purpose**: Manages the internal pipeline topology: creates bounded channels between stages, registers stage handles, monitors channel fill levels, and handles bypass mode transitions.

```rust
#[async_trait]
pub trait PipelineManager: Debug + Send + Sync {
    /// Register a pipeline stage with its input/output channel capacities.
    fn register_stage(&self, name: &str, config: StageChannelConfig) -> Result<(), PerceptionError>;

    /// Connect two stages with a bounded mpsc channel.
    fn connect(&self, from: &str, to: &str, capacity: usize) -> Result<(), PerceptionError>;

    /// Get the current fill level of all channels.
    fn channel_fill_levels(&self) -> HashMap<String, (usize, usize)>;

    /// Place a stage into bypass mode (observations skip this stage).
    fn set_bypass(&self, stage: &str, bypass: bool) -> Result<(), PerceptionError>;

    /// Remove a stage from the pipeline.
    fn remove_stage(&self, name: &str) -> Result<(), PerceptionError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageChannelConfig {
    pub name: String,
    pub input_capacity: usize,
    pub output_capacity: usize,
    pub timeout_ms: u64,
}
```

---

## Error Model

```rust
#[derive(Debug, thiserror::Error)]
pub enum PerceptionError {
    // Observer (1xxx)
    #[error("observer bind failed: {0}")]
    ObserverBindFailed(String),
    #[error("observer channel closed")]
    ObserverChannelClosed,
    #[error("observer permission denied: {0}")]
    ObserverPermissionDenied(String),
    #[error("observer read failed: {0}")]
    ObserverReadFailed(String),

    // Normalizer (2xxx)
    #[error("validation failed for {observation_id}: {reason}")]
    ValidationFailed { observation_id: ObservationId, reason: String },
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("sanitization failed: {0}")]
    SanitizationFailed(String),
    #[error("payload too big: {size} > {max}")]
    PayloadTooBig { size: u64, max: u64 },

    // Attention/Filter (3xxx)
    #[error("attention overload")]
    AttentionOverload,
    #[error("habituation state corrupt: {0}")]
    HabituationStateCorrupt(String),

    // Entity (4xxx)
    #[error("entity resolution failed: {entity_type} = {raw_value}")]
    EntityResolutionFailed { entity_type: String, raw_value: String },
    #[error("entity pattern not found: {0}")]
    EntityPatternNotFound(String),
    #[error("knowledge graph query timeout")]
    KnowledgeGraphTimeout,

    // Context (5xxx)
    #[error("session resolution timeout")]
    SessionResolutionTimeout,
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("temporal context error: {0}")]
    TemporalContextError(String),

    // Detector (6xxx)
    #[error("state machine not found: {0:?}")]
    StateMachineNotFound(ScopeId),
    #[error("state machine conflict on {scope:?}: {from} -> {to}")]
    StateMachineConflict { scope: ScopeId, from: String, to: String },
    #[error("invalid state transition on {scope:?}: {from} -> {to}")]
    InvalidStateTransition { scope: ScopeId, from: String, to: String },

    // Anomaly (7xxx)
    #[error("anomaly model not ready: {0}")]
    AnomalyModelNotReady(String),
    #[error("insufficient data for {modality:?}: need {required}, have {available}")]
    InsufficientData { modality: Modality, required: usize, available: usize },
    #[error("invalid sensitivity: {0}")]
    InvalidSensitivity(f64),

    // Fusion (8xxx)
    #[error("correlation window full")]
    CorrelationWindowFull,
    #[error("fusion conflict for {correlation_key:?}: {reason}")]
    FusionConflict { correlation_key: CorrelationKey, reason: String },
    #[error("correlation timeout: {0:?}")]
    CorrelationTimeout(CorrelationKey),

    // Coordinator/Pipeline (9xxx)
    #[error("pipeline stage timeout: {0}")]
    PipelineStageTimeout(String),
    #[error("pipeline assembly failed: {0}")]
    PipelineAssemblyFailed(String),
    #[error("channel closed: {0}")]
    ChannelClosed(String),
    #[error("configuration error: {0}")]
    ConfigurationError(String),

    // Wrapped
    #[error("core error: {0}")]
    Core(#[from] core::CoreError),
    #[error("runtime error: {0}")]
    Runtime(#[from] runtime::RuntimeError),
    #[error("memory error: {0}")]
    Memory(#[from] memory_core::MemoryError),
    #[error("osal error: {0}")]
    Osal(#[from] osal_core::OsalError),
}
```

---

## Events

### Published Events

| Event | Type String | Payload | Stage |
|---|---|---|---|
| `ObservationReceived` | `perception.observation.received` | `{ observation_id, source, modality, size }` | Observer |
| `ObservationNormalized` | `perception.observation.normalized` | `{ observation_id, format, schema_version }` | Normalizer |
| `ObservationRejected` | `perception.observation.rejected` | `{ observation_id, reason }` | Normalizer |
| `ObservationFiltered` | `perception.observation.filtered` | `{ observation_id, score, reason }` | AttentionFilter |
| `EntitiesExtracted` | `perception.entities.extracted` | `{ observation_id, entity_count }` | EntityExtractor |
| `EntityResolutionFailed` | `perception.entities.resolution_failed` | `{ entity_type, raw_value }` | EntityExtractor |
| `ContextEnriched` | `perception.context.enriched` | `{ observation_id, session_id }` | ContextEnricher |
| `SessionResolutionFailed` | `perception.context.session_failed` | `{ session_id, reason }` | ContextEnricher |
| `StateChanged` | `perception.state.changed` | `{ scope, from, to }` | StateDetector |
| `StateMachineConflict` | `perception.state.conflict` | `{ scope, from, to, reason }` | StateDetector |
| `AnomalyDetected` | `perception.anomaly.detected` | `{ observation_id, score, threshold, model }` | AnomalyDetector |
| `AnomalyBaselineUpdated` | `perception.anomaly.baseline` | `{ modality, window_size, sample_count }` | AnomalyDetector |
| `ObservationFused` | `perception.fusion.complete` | `{ correlation_key, sources, confidence }` | FusionEngine |
| `FusionWindowExpired` | `perception.fusion.window_expired` | `{ correlation_key, expected_sources, received }` | FusionEngine |
| `ObservationReady` | `perception.observation.ready` | `{ observation_id, modality, priority }` | Coordinator |
| `PipelineStageFailed` | `perception.pipeline.stage_failed` | `{ stage, error, restarts }` | Coordinator |
| `PipelineBackpressure` | `perception.pipeline.backpressure` | `{ stage, channel_capacity, drop_count }` | Coordinator |
| `PipelineReconfigured` | `perception.pipeline.reconfigured` | `{ sections_changed }` | Coordinator |
| `ObserverStatusChanged` | `perception.observer.status` | `{ observer_id, old_status, new_status }` | Coordinator |

### Consumed Events

| Source | Event | Consumer | Purpose |
|---|---|---|---|
| OSAL | `osal.fs_*` | FileSystemObserver | Forward to observer pipeline |
| OSAL | `osal.process_*` | ProcessObserver | Forward to observer pipeline |
| OSAL | `osal.terminal_*` | TerminalObserver | Forward to observer pipeline |
| OSAL | `osal.network_*` | NetworkObserver | Forward to observer pipeline |
| Core | `core.config_changed` | Coordinator | Reload pipeline configuration |
| Brain | `brain.attention_request` | Coordinator | Adjust attention thresholds |
| Runtime | `runtime.session_*` | ContextEnricher | Warm/evict session cache |

---

## Performance Targets

| Operation | p50 | p99 | Sustained |
|---|---|---|---|
| Observe (event wrap) | 1 µs | 10 µs | 500K/s |
| Normalize (1 KB) | 2 µs | 5 µs | 200K/s |
| Attention filter | 500 ns | 2 µs | 1M/s |
| Entity extract (cached) | 5 µs | 20 µs | 100K/s |
| Context enrich | 2 µs | 10 µs | 200K/s |
| State detect | 1 µs | 5 µs | 200K/s |
| Anomaly detect | 2 µs | 10 µs | 200K/s |
| Fusion (pair) | 2 µs | 10 µs | 100K/s |
| **End-to-end (hot path)** | 50 µs | 250 µs | 20K/s |
| **End-to-end (cold path)** | 200 µs | 1 ms | 5K/s |

---

## Dependencies

| Crate | External Dependencies |
|---|---|
| `perception-core` | `ai-os-core`, `memory-core`, `serde`, `uuid`, `thiserror` |
| `perception-observer` | `perception-core`, `ai-os-runtime`, `osal-core`, `osal-filesystem`, `osal-network`, `osal-terminal`, `tokio` |
| `perception-normalizer` | `perception-core`, `encoding_rs`, `serde_json`, `regex` |
| `perception-detector` | `perception-core` |
| `perception-context` | `perception-core`, `perception-observer`, `memory-retrieval` |
| `perception-entities` | `perception-core`, `perception-normalizer`, `memory-knowledge`, `regex` |
| `perception-anomaly` | `perception-core`, `perception-detector` |
| `perception-fusion` | `perception-core` |
| `perception-coordinator` | All perception crates, `ai-os-runtime`, `brain-core`⸝, `tokio` |

⸝ `brain-core` is a placeholder dependency for the Perception→Brain bridge. Should be replaced by EventBus communication during implementation (Perception publishes `ObservationReady`, Brain subscribes). See RFC-0004 §Dependencies.
