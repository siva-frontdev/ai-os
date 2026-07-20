# Perception Processing Pipeline

## Overview

The Perception Pipeline is a linear sequence of processing stages that transforms raw observations into enriched, ready-for-reasoning observations for the Brain Platform. Each stage has a single responsibility and communicates with the next stage through a bounded asynchronous channel.

```
Raw Source ──→ Observer ──→ Normalizer ──→ Attention Filter ──→ Entity Extractor
                                                                       │
                                                                       v
 Brain ←── Fusion Engine ←── Anomaly Detector ←── State Detector ←── Context Enricher
```

---

## Stage Descriptions

### Stage 1: Observer

**Input**: Raw data from OSAL events, file descriptors, network sockets, stdin, hardware sensors.

**Output**: `Observation` with raw payload, source metadata, and initial confidence.

**Behavior**:

1. An `Observer` instance subscribes to an OSAL event stream (e.g., `osal.fs_modified`) or reads directly from an I/O source.
2. On each event, the observer constructs an `Observation` with:
   - `id` = new UUID v7
   - `timestamp` = current time
   - `source` = observer identity
   - `modality` = determined by observer kind
   - `confidence` = 1.0 (will be refined by later stages)
   - `priority` = configured observer priority
   - `payload` = raw data wrapped in appropriate `ObservationPayload` variant
   - `provenance.raw_bytes` = copy of raw bytes
   - `quality` = default values (set by normalizer)
3. The observation is sent through the `mpsc` channel to the normalizer.

**Backpressure**: If the channel to the normalizer is full:
- Critical/High priority: `send().await` blocks the observer task, propagating backpressure to the OSAL event source.
- Normal priority: The oldest observation in the channel buffer is dropped (tail-drop).
- Low/Background priority: The observation is dropped immediately.

**Events emitted**: `ObservationReceived`

---

### Stage 2: Normalizer

**Input**: `Observation` from observer.

**Output**: `Observation` with validated payload, detected format, sanitized content, and quality score.

**Behavior**:

1. **Format detection**: The normalizer examines the raw bytes and determines the format (JSON, text, binary) using registered `FormatParser` instances.
2. **Schema validation**: If a schema is registered for the observation's modality, the payload is validated against it. Required fields must be present; field types must match; string lengths must not exceed limits.
3. **Sanitization**: Control characters are stripped from text payloads. Encoding is normalized to the configured default (UTF-8). Payload size is enforced — observations exceeding `max_payload_bytes` are rejected.
4. **Payload conversion**: The raw bytes are converted to the canonical `ObservationPayload` variant.
5. **Provenance update**: A `TransformationStep` is appended to the provenance log. If `strip_raw_bytes` is true, the raw bytes are dropped after normalization.
6. **Quality score**: `completeness` (fraction of required fields present), `timeliness` (age since timestamp), and `signal_to_noise` (estimated based on payload structure) are computed.

**Validation failure**:
- If validation fails, an `ObservationRejected` event is published.
- The observation is dropped. The pipeline continues with the next observation.

**Events emitted**: `ObservationNormalized` (on success), `ObservationRejected` (on failure).

---

### Stage 3: Attention Filter

**Input**: Normalized `Observation`.

**Output**: `Observation` (if score >= threshold) or dropped.

**Behavior**:

1. **Attention scoring**: The filter computes an attention score for the observation:
   ```
   score = (priority_weight * priority_score)
         + (novelty_bonus if novel)
         - (habituation_decay * habituation_count)
         + (recency_weight * recency_score)
   ```
   Where:
   - `priority_score` = (5 - priority_rank) / 5 (Critical=1.0, Background=0.2)
   - `habituation_count` = number of times this exact observation pattern has been seen
   - `recency_score` = `1.0 - (age_in_seconds / max_age)`, clamped to [0, 1]

2. **Threshold comparison**: If `score >= global_threshold` (or per-modality threshold if configured), the observation proceeds. Otherwise, it is discarded.

3. **Habituation update**: After scoring, the filter records the observation's identifying characteristics (modality + payload pattern hash) in the habituation state. The next identical observation will receive a lower score.

4. **Novelty detection**: The first occurrence of any observation pattern receives a `novelty_bonus`.

**Discard behavior**: If discarded, an `ObservationFiltered` event is published with the score and reason.

**Events emitted**: `ObservationFiltered` (on discard), `PipelineBackpressure` (on queue overflow, emitted by Coordinator).

---

### Stage 4: Entity Extractor

**Input**: Normalized, filtered `Observation`.

**Output**: `Observation` with `metadata["entities"]` populated with resolved entity references.

**Behavior**:

1. **Pattern matching**: The extractor applies registered extraction patterns (regex) to the observation payload. Each match produces a candidate `EntityRef`.

2. **Knowledge graph lookup**: For each candidate, the extractor queries the Memory Platform's knowledge graph (`memory-knowledge`) to resolve the raw identifier into a known entity. If the entity exists in the graph, the `EntityRef` is populated with the canonical ID, label, and type.

3. **Cache**: Resolved entities are cached with a configurable TTL. Cache hits bypass the knowledge graph query.

4. **Result attachment**: Resolved `EntityRef` values are serialized into `observation.metadata["entities"]` as a JSON array.

5. **Failure handling**: If a knowledge graph query times out or fails, the extractor logs the failure, publishes `EntityResolutionFailed`, and continues with an empty entity set for that candidate.

**Events emitted**: `EntitiesExtracted`, `EntityResolutionFailed`.

---

### Stage 5: Context Enricher

**Input**: `Observation` with entity references.

**Output**: `Observation` with temporal, spatial, and session context attached.

**Behavior**:

1. **Temporal context**: Wall-clock timestamp, timezone, and system uptime are attached:
   ```json
   "metadata.temporal_context": {
       "wall_clock": "<timestamp>",
       "timezone": "UTC",
       "uptime_seconds": 123456.78
   }
   ```

2. **Spatial context**: Hostname, current working directory (if resolvable), and network address are attached:
   ```json
   "metadata.spatial_context": {
       "hostname": "ai-os-host",
       "cwd": "/home/user",
       "session_id": "sess-abc123",
       "network_addr": "192.168.1.100"
   }
   ```

3. **Session resolution**: If the observation contains a PID (or the observer is associated with a session), the enricher calls Runtime's `SessionManager` to resolve the session context. The resolved session ID, user ID, and scope are attached.

4. **Context merging**: If multiple context sources provide overlapping information (e.g., both the observer's known session and a resolved PID point to the same session), the enricher merges with highest-confidence wins.

**Events emitted**: `ContextEnriched`, `SessionResolutionFailed`.

---

### Stage 6: State Detector

**Input**: Enriched `Observation`.

**Output**: `Observation` with `metadata.state_transition` if a state change was detected.

**Behavior**:

1. **Scope assignment**: The detector extracts a scope key from the observation (e.g., session ID, hostname, process ID). Each scope maintains its own state machine.

2. **State machine lookup**: The detector looks up the state machine registered for this scope. If no machine exists, a new one is created in the initial state.

3. **Transition evaluation**: The observation is evaluated against registered transition rules. Rules match based on the observation's modality, payload content, and current state.

4. **Transition execution**: If a matching rule is found:
   - The state machine transitions to the new state.
   - A `StateTransition` record is created.
   - The observation's `metadata.state_transition` is set to `{ from, to, transition }`.
   - A `StateChanged` event is published.

5. **Continuous value detection** (optional): For metric-type observations, the `ChangeDetector` evaluates whether the value has crossed a threshold or changed significantly.

**Events emitted**: `StateChanged` (on transition), `StateMachineConflict` (on invalid transition attempt).

---

### Stage 7: Anomaly Detector

**Input**: Enriched `Observation` with optional state transition.

**Output**: `Observation` with `metadata.anomaly_score` if score exceeds threshold.

**Behavior**:

1. **Modality window lookup**: The detector maintains per-modality sliding windows of recent observation values (e.g., file system event rate, network connection rate).

2. **Statistical scoring**: The configured anomaly model (Z-score, rolling window, or Holt-Winters) scores the observation against the window's statistical baseline.

3. **Threshold comparison**: If the anomaly score exceeds the configured threshold, the observation is tagged:
   ```json
   "metadata.anomaly_score": {
       "score": 0.87,
       "threshold": 2.0,
       "is_anomaly": true,
       "model": "zscore"
   }
   ```

4. **Model update**: After scoring, the observation value is fed into the statistical model for baseline maintenance.

5. **Insufficient data**: If the window does not have enough data points (configurable `min_data_points`), a neutral score (0.0) is returned. The `AnomalyModelNotReady` error is logged but does not interrupt the pipeline.

**Events emitted**: `AnomalyDetected` (when score exceeds threshold).

---

### Stage 8: Fusion Engine

**Input**: Enriched, state-tagged, anomaly-scored `Observation`.

**Output**: `FusedObservation` (if correlation completes) or passes through unchanged.

**Behavior**:

1. **Correlation key extraction**: The engine evaluates registered correlation rules against the observation. Each rule has a `correlation_key_expr` that extracts a key from the observation's metadata (e.g., `${metadata.session_id}`).

2. **Window management**: If a correlation key matches a rule, the observation is inserted into the correlation window for that key. Windows have configurable time durations. Observations within a window are grouped by correlation key.

3. **Completion check**: When all expected sources for a rule have contributed observations to the window (or `min_observations` is reached), the engine synthesizes a `FusedObservation`:
   - `fused_payload`: Combined according to the rule's `fusion_fn` (merge, highest-confidence, most-recent).
   - `confidence`: Consolidated from constituent observations (average weighted by individual confidence).
   - `provenance`: Merged transformation logs from all constituents.
   - `constituent_ids`: List of all observation IDs that contributed.

4. **Window expiry**: A background task periodically checks for expired windows. Expired windows that did not reach `min_observations` are flushed as incomplete — each constituent observation is released independently.

5. **Pass-through**: If no correlation rule matches, the observation passes through the fusion engine unchanged.

**Events emitted**: `ObservationFused` (on successful fusion), `FusionWindowExpired` (on window timeout).

---

## Data Flow: Complete Sequence

```
TerminalObserver                     Normalizer                      AttentionFilter
     |                                   |                               |
     | 1. osal.terminal_input            |                               |
     |---- (OSAL event) ---------------->|                               |
     |                                   |                               |
     | 2. Observation(                   |                               |
     |      payload: "ls -la\n",         |                               |
     |      modality: Terminal,          |                               |
     |      priority: Critical)          |                               |
     |---- (mpsc) ---------------------->|                               |
     |                                   | 3. validate schema            |
     |                                   | 4. detect format (text)       |
     |                                   | 5. sanitize, normalize        |
     |                                   | 6. ObservationNormalized      |
     |                                   |---- (EventBus) -------------->|
     |                                   |                               |
     |                                   | 7. Observation(provenance+)  |
     |                                   |---- (mpsc) ------------------>|
     |                                   |                               | 8. compute attention
     |                                   |                               | 9. score = 0.72 >= 0.5
     |                                   |                               | 10. record habituation
     |                                   |                               | 11. proceed (no event)
     |                                   |                               |
     |                                   |                               |
EntityExtractor                      ContextEnricher                  StateDetector
     |                                   |                               |
     | 12. Observation                  |                               |
     |<--- (mpsc) -----------------------|                               |
     |                                   |                               |
     | 13. extract "ls" -> no entity    |                               |
     | 14. extract PID -> Process{1234} |                               |
     | 15. EntitiesExtracted            |                               |
     |---- (EventBus) ------------------>|                               |
     |                                   |                               |
     | 16. Observation(entities+)       |                               |
     |---- (mpsc) ---------------------->|                               |
     |                                   | 17. temporal: now, UTC        |
     |                                   | 18. spatial: hostname, cwd    |
     |                                   | 19. session: resolve PID→sess |
     |                                   | 20. ContextEnriched           |
     |                                   |---- (EventBus) -------------->|
     |                                   |                               |
     |                                   | 21. Observation(context+)    |
     |                                   |---- (mpsc) ------------------>|
     |                                   |                               | 22. session scope
     |                                   |                               | 23. state: Idle→Active
     |                                   |                               | 24. StateChanged
     |                                   |                               |---- (EventBus) -->|
     |                                   |                               |                   |
     |                                   |                               |                   v
     |                                   |                               |
AnomalyDetector                       FusionEngine                    Coordinator
     |                                   |                               |
     | 25. Observation(state+)          |                               |
     |<--- (mpsc) -----------------------|                               |
     |                                   |                               |
     | 26. modality: Terminal            |                               |
     | 27. compute Z-score: 1.2 < 1.5   |                               |
     | 28. not anomalous                 |                               |
     |                                   |                               |
     | 29. Observation(no anomaly)      |                               |
     |---- (mpsc) ---------------------->|                               |
     |                                   | 30. check correlation rules   |
     |                                   | 31. no match for Terminal     |
     |                                   | 32. pass-through              |
     |                                   |                               |
     |                                   | 33. Observation(ready)       |
     |                                   |---- (mpsc) ------------------>|
     |                                   |                               | 34. ObservationReady
     |                                   |                               |---- (EventBus) -->|
     |                                   |                               |                   |
     |                                   |                               |                   v
     |                                   |                               |             Brain Platform
```

---

## Backpressure Behavior

Backpressure is a first-class concern in the pipeline design. Each `mpsc` channel has a configurable capacity. When a channel is full, the behavior depends on observation priority:

| Priority | Channel Full Behavior | Downstream Signal |
|---|---|---|
| Critical | `send().await` blocks sender | Backpressure propagates to observer task, which blocks the OSAL event handler |
| High | `send().await` blocks sender (same as Critical) | Same |
| Normal | `try_send()` drops oldest in buffer (tail-drop) | `PipelineBackpressure` event emitted once per second while dropping |
| Low | `try_send()` drops immediately | `PipelineBackpressure` event emitted |
| Background | `try_send()` drops immediately (no event) | Silently dropped |

### Stage-Level Backpressure Isolation

A slow stage does not block earlier stages indefinitely because:

1. The channel between each pair of stages is independent. If stage N is slow, its input channel fills up, but stages before N-1 continue operating until their own channels fill.
2. Observer channels have per-observer capacity. A single slow observer cannot consume capacity allocated to other observers.
3. The coordinator monitors all channel fill levels and emits warnings when any channel exceeds 80% capacity.

---

## Bypass Mode

When a pipeline stage fails repeatedly (configurable: `max_stage_restarts` within `restart_window_seconds`), the coordinator places that stage into **bypass mode**:

1. Observations skip the failed stage and pass directly to the next stage.
2. The coordinator continues attempting to restart the stage in the background (with exponential backoff: 1s, 2s, 4s, 8s, 30s max).
3. When the stage restarts successfully, it exits bypass mode and observations resume flowing through it.
4. While in bypass mode, the coordinator publishes `PipelineStageFailed` events periodically.

**Bypass behavior per stage**:

| Stage | Bypass Effect |
|---|---|
| Normalizer | Observations pass through unnormalized (raw payload kept). Quality score set to minimum. |
| Attention Filter | All observations pass through (no filtering). |
| Entity Extractor | No entity extraction. Observations proceed without entity metadata. |
| Context Enricher | Basic context (timestamp, hostname) attached inline by coordinator. Session resolution skipped. |
| State Detector | No state tracking. State transition metadata is empty. |
| Anomaly Detector | No anomaly scoring. Anomaly metadata is absent. |
| Fusion Engine | Observations pass through unfused. |

---

## Pipeline Reconfiguration

The pipeline can be reconfigured at runtime via the `core.config_changed` event:

1. The coordinator receives the event and parses the new configuration.
2. For each changed section:
   - **Observers**: New observers are started. Removed observers are drained and stopped. Changed observers are restarted with new config.
   - **Normalizer schemas**: Schemas are hot-reloaded. In-flight observations continue with the previous schema.
   - **Attention thresholds**: Updated immediately. Habituation state is preserved unless `reset_habituation = true`.
   - **Anomaly windows**: Window sizes and sensitivity updated. Existing windows are reset on size change.
   - **Fusion rules**: Correlation rules are hot-reloaded. Active windows with previous rules are flushed.
3. During reconfiguration, the pipeline continues operating. No observations are lost (channels are not drained).
4. On completion, a `PipelineReconfigured` event is published.

---

## Diagram: Stateful Stage Lifecycle

```
        +-----------+
        |  Created  |      Stage constructed, not yet registered
        +-----+-----+
              |
              v
        +-----------+
        |  Active   |      Stage registered, processing observations
        +-----+-----+
              |
        +-----+------+
        |             |
        v             v
+-----------+   +-----------+
|  Bypass   |   |  Failed   |    Bypass = observations skip stage
+-----------+   +-----------+    Failed = stage terminated, will restart
        |             |
        +------+------+
               |
               v
        +-----------+
        |  Restart  |      Exponential backoff restart attempt
        +-----+-----+
              |
        +-----+------+
        |             |
        v             v
   +--------+    +--------+
   | Active |    | Bypass |   (if max_restarts exceeded)
   +--------+    +--------+
```

---

## Performance Characteristics

| Pipeline Configuration | p50 End-to-End | p99 End-to-End | Max Throughput |
|---|---|---|---|
| All stages active, cached entities | 50 µs | 250 µs | 20K obs/s |
| All stages active, cold entities | 200 µs | 1 ms | 5K obs/s |
| Filter + Context only (no entity, anomaly, fusion) | 10 µs | 50 µs | 100K obs/s |
| Observer → Normalizer only (debug mode) | 3 µs | 10 µs | 500K obs/s |
| Bypass mode (all stages bypassed) | 1 µs | 5 µs | 1M obs/s |

Performance is dominated by:
- **Normalizer**: Payload parsing (unstructured text is 5x slower than structured JSON)
- **Entity Extractor**: Knowledge graph queries (cold cache adds ~100 µs per lookup)
- **Anomaly Detector**: Holt-Winters model is 5x slower than Z-score
- **Fusion Engine**: Window management overhead is negligible (< 1 µs per observation)
