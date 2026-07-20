# Perception Platform Configuration — `perception.toml`

## Overview

The `perception.toml` file defines all runtime configuration for the Perception Platform. It is loaded by `PerceptionCoordinator` during the `init()` phase of the Service lifecycle. Configuration can be reloaded at runtime by publishing a `core.config_changed` event with a new `perception.toml` payload.

The file uses TOML syntax and is divided into sections that map one-to-one to pipeline stages and their configuration domains.

---

## Complete Specification

### Root Section

```toml
[perception]
# Enable or disable the entire Perception Platform.
enabled = true

# Log level for perception-specific messages.
log_level = "info"                          # "trace" | "debug" | "info" | "warn" | "error"

# Default priority assigned to observations without explicit priority.
default_priority = "Normal"                 # "Critical" | "High" | "Normal" | "Low" | "Background"

# Global observation ID namespace prefix.
namespace = "default"

# Strip raw bytes from provenance after normalization (reduces memory).
strip_raw_bytes = true
```

---

### `[observers]` — Observer definitions

Each key under `[observers]` defines one observer instance. The key becomes the observer's `observer_id`.

```toml
[observers.fs_watcher]
# Observer kind. Required.
kind = "FileSystem"

# Enable or disable this observer.
enabled = true

# File system paths to watch.
paths = ["/home", "/tmp", "/var/log"]

# File event types to subscribe to.
events = ["created", "modified", "deleted"]

# Internal buffer size (number of raw observations before backpressure).
buffer_size = 1024

# Priority assigned to observations from this observer.
priority = "Normal"                         # "Critical" | "High" | "Normal" | "Low" | "Background"

[observers.process_monitor]
kind = "Process"
enabled = true
buffer_size = 512
priority = "High"

[observers.terminal]
kind = "Terminal"
enabled = true
source = "stdin"                           # "stdin" | "pts/<n>"
buffer_size = 256
priority = "Critical"

[observers.network]
kind = "Network"
enabled = true
# Network event types to subscribe to.
events = ["connect", "disconnect", "listen"]
buffer_size = 1024
priority = "Normal"

[observers.system_events]
kind = "SystemEvent"
enabled = true
# System event sources.
sources = ["udev", "journal", "process"]
buffer_size = 512
priority = "Low"
```

---

### `[normalizers]` — Normalizer configuration

```toml
[normalizers]
# Default encoding assumed when encoding is not specified.
default_encoding = "utf-8"

# Maximum payload size in bytes. Observations exceeding this are rejected.
max_payload_bytes = 1_048_576              # 1 MB

# Whether to apply strict schema validation.
strict_validation = false

# Strip control characters from text payloads.
sanitize_control_chars = true

# Maximum nesting depth for structured payloads.
max_nesting_depth = 32

# List of allowed MIME types for binary payloads. Empty = allow all.
allowed_mime_types = [
    "text/plain",
    "application/json",
    "application/octet-stream",
]
```

#### `[normalizers.schemas]` — Validation schemas

Each key defines a named schema for a specific modality. Schemas are used to validate structured payloads.

```toml
[normalizers.schemas.process_spawned]
modality = "Process"
# Schema fields
[[normalizers.schemas.process_spawned.fields]]
name = "pid"
type = "Integer"
required = true

[[normalizers.schemas.process_spawned.fields]]
name = "command"
type = "String"
required = true
max_length = 4096

[[normalizers.schemas.process_spawned.fields]]
name = "uid"
type = "Integer"
required = true

[[normalizers.schemas.process_spawned.fields]]
name = "args"
type = "Array"
required = false

[normalizers.schemas.file_modified]
modality = "FileSystem"

[[normalizers.schemas.file_modified.fields]]
name = "path"
type = "String"
required = true
max_length = 4096

[[normalizers.schemas.file_modified.fields]]
name = "size"
type = "Integer"
required = false
```

#### `[normalizers.formats]` — Format parser registration

```toml
[normalizers.formats]
# Built-in format parsers enabled by default.
# Additional parsers can be registered programmatically.
builtin_parsers = ["json", "text", "binary"]
```

---

### `[attention]` — Attention / Filter configuration

```toml
[attention]
# Global attention threshold (0.0 = let everything through, 1.0 = block everything).
global_threshold = 0.3

# Habituation decay factor per observation match.
habituation_decay = 0.95

# Bonus applied to first occurrence of a novel observation pattern.
novelty_bonus = 0.2

# Recency weight for scoring (higher = recent observations score higher).
recency_weight = 0.1

# Priority weight (higher = priority has more influence on final score).
priority_weight = 0.4

# Maximum attention queue size before backpressure.
max_queue_size = 10_000
```

#### `[attention.per_modality]` — Per-modality overrides

```toml
[attention.per_modality]
Terminal = { threshold = 0.5, decay = 0.9 }
FileSystem = { threshold = 0.2, decay = 0.98 }
Network = { threshold = 0.4, decay = 0.95 }
Process = { threshold = 0.3, decay = 0.95 }
```

---

### `[entities]` — Entity extraction configuration

```toml
[entities]
# Knowledge graph endpoint. "local" = in-process memory-knowledge.
memory_graph_endpoint = "local"

# Cache TTL for resolved entities in seconds.
cache_ttl_seconds = 300

# Maximum number of entities to cache per modality.
max_cached_entities = 10_000

# Enable entity resolution via knowledge graph.
enable_graph_lookup = true

# Enable regex-based entity extraction from text payloads.
enable_regex_extraction = true
```

#### `[entities.patterns]` — Extraction patterns

```toml
[entities.patterns.process_pid]
entity_kind = "Process"
modality = "Process"
pattern = "^pid=(\\d+)$"

[entities.patterns.file_path]
entity_kind = "File"
modality = "FileSystem"
pattern = "^path=([^\\s]+)$"

[entities.patterns.ip_address]
entity_kind = "NetworkConnection"
modality = "Network"
pattern = "\\b(?:\\d{1,3}\\.){3}\\d{1,3}\\b"
```

---

### `[anomaly]` — Anomaly detection configuration

```toml
[anomaly]
# Enable anomaly detection.
enabled = true

# Default sliding window size in seconds for statistical baseline.
default_window_seconds = 60

# Default Z-score sensitivity (standard deviations from mean).
default_sensitivity = 2.0

# Minimum data points required before anomaly scoring begins.
min_data_points = 30

# Model type for anomaly detection: "zscore", "rolling_window", "holt_winters".
default_model = "zscore"

# Interval in seconds for recomputing statistical baselines.
baseline_recompute_interval_seconds = 300
```

#### `[anomaly.per_modality]` — Per-modality overrides

```toml
[anomaly.per_modality]
Network = { window_seconds = 300, sensitivity = 3.0, model = "holt_winters" }
FileSystem = { window_seconds = 60, sensitivity = 2.0, model = "zscore" }
Process = { window_seconds = 120, sensitivity = 2.5, model = "zscore" }
Terminal = { window_seconds = 30, sensitivity = 1.5, model = "rolling_window" }
```

---

### `[fusion]` — Observation fusion configuration

```toml
[fusion]
# Enable the fusion engine.
enabled = true

# Default correlation time window in milliseconds.
default_window_ms = 100

# Minimum confidence threshold for fused observations.
confidence_threshold = 0.6

# Maximum number of concurrent correlation windows.
max_active_windows = 10_000

# Background task interval for flushing expired windows (milliseconds).
window_expiry_interval_ms = 50
```

#### `[fusion.rules]` — Correlation rules

Correlation rules define which observations from which sources should be fused.

```toml
[fusion.rules.fs_process_correlation]
# Expression that produces the correlation key from an observation's metadata.
# Supports simple metadata field references: "${source.instance_id}"
correlation_key_expr = "${metadata.session_id}"

# Expected observer kinds for this correlation.
expected_sources = ["FileSystem", "Process"]

# Correlation window in milliseconds.
window_ms = 50

# Minimum number of observations needed to complete a fusion.
min_observations = 2

# Fusion strategy: "merge_payloads", "take_highest_confidence", "take_most_recent".
fusion_fn = "merge_payloads"
```

---

### `[pipeline]` — Pipeline assembly

```toml
[pipeline]
# Stage execution order. All stages listed here are mandatory.
# Stages can be omitted to disable them (faster pipeline).
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

# Bounded channel capacity between stages (number of observations).
channel_capacity = 10_000

# Timeout in milliseconds for each stage to process a single observation.
stage_timeout_ms = 5_000

# Maximum number of consecutive stage restarts before entering bypass mode.
max_stage_restarts = 5

# Time window in seconds for counting stage restarts.
restart_window_seconds = 60

# Whether to enable bypass mode when a stage fails repeatedly.
bypass_on_failure = true

# Log level for dropped observations.
drop_log_level = "warn"
```

---

## Configuration Lifecycle

### Startup

1. `PerceptionCoordinator::init()` reads `perception.toml` from the platform's configuration directory (`/etc/ai-os/perception.toml` or path from `--config` flag).
2. Each section is validated independently. Invalid sections produce errors but do not prevent the coordinator from starting.
3. Invalid sections fall back to their default values.

### Runtime Reload

1. Some other module publishes `core.config_changed` with a new `perception.toml` payload.
2. `PerceptionCoordinator` subscribes to this event and calls `reload_config()`.
3. Each section is diffed against the current config:
   - `[observers]` — New observers are registered and started. Removed observers are drained and stopped. Changed observers are restarted.
   - `[normalizers]` — Schemas are hot-reloaded. In-flight observations continue with the previous schema.
   - `[attention]` — Thresholds and decay factors are updated immediately.
   - `[anomaly]` — Window sizes and sensitivity are updated. Windows are reset on change.
   - `[fusion]` — Correlation rules are hot-reloaded. Active windows continue with previous rules.
4. An `PipelineReconfigured` event is published on successful reload.

### Validation Rules

| Key | Validation | Default |
|---|---|---|
| `perception.enabled` | Must be boolean | `true` |
| `perception.default_priority` | Must be a valid `ObservationPriority` variant | `Normal` |
| `observers.*.kind` | Must be a known `ObserverKind` | (required) |
| `observers.*.buffer_size` | Must be > 0 and <= 1,000,000 | 1024 |
| `normalizers.max_payload_bytes` | Must be > 0 and <= 100,000,000 | 1,048,576 |
| `attention.global_threshold` | Must be 0.0..=1.0 | 0.3 |
| `attention.habituation_decay` | Must be 0.0..=1.0 | 0.95 |
| `entities.cache_ttl_seconds` | Must be >= 0 | 300 |
| `anomaly.default_window_seconds` | Must be >= 1 | 60 |
| `anomaly.default_sensitivity` | Must be > 0.0 | 2.0 |
| `fusion.default_window_ms` | Must be >= 1 | 100 |
| `fusion.confidence_threshold` | Must be 0.0..=1.0 | 0.6 |
| `pipeline.channel_capacity` | Must be > 0 and <= 1,000,000 | 10,000 |
| `pipeline.stage_timeout_ms` | Must be >= 100 | 5,000 |
| `pipeline.max_stage_restarts` | Must be >= 0 | 5 |
