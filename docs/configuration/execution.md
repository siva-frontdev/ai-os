# Execution Platform Configuration — `execution.toml`

## Overview

The `execution.toml` file defines all runtime configuration for the Execution Platform. It is loaded by `ExecutionCoordinator` during the `init()` phase of the Service lifecycle. Configuration can be reloaded at runtime by publishing a `core.config_changed` event with a new `execution.toml` payload.

The file uses TOML syntax and is divided into sections that map one-to-one to pipeline stages and their configuration domains.

---

## Complete Specification

### Root Section

```toml
[execution]
# Enable or disable the entire Execution Platform.
enabled = true

# Log level for execution-specific messages.
log_level = "info"            # "trace" | "debug" | "info" | "warn" | "error"

# Default execution priority (0-255, higher = more urgent).
default_priority = 128

# Default timeout in milliseconds for executions without explicit budget.
default_timeout_ms = 30000

# Strip raw stdout/stderr from ExecutionHistory after routing (reduces memory).
strip_output_after_routing = true
```

---

### `[pipeline]` — Pipeline assembly

```toml
[pipeline]
# Ordered list of pipeline stages.
stage_order = [
    "planner",
    "dispatcher",
    "runner",
    "monitor",
    "results",
    "recovery",
]

# Bounded channel capacity between stages.
channel_capacity = 10000

# Per-stage timeout in milliseconds.
stage_timeout_ms = 5000

# Maximum stage restarts before entering bypass mode.
max_stage_restarts = 5

# Time window in seconds for restart counting.
restart_window_seconds = 60
```

---

### `[dispatch]` — Dispatch queue

```toml
[dispatch]
# Maximum number of simultaneous executions across all backends.
max_concurrency = 50

# Maximum queue depth for pending executions.
queue_capacity = 10000

# Priority levels and their queue capacities.
# Priority values: critical(0-31), high(32-95), normal(96-191), low(192-255)
[dispatch.priority_queues]
critical_capacity = 1000
high_capacity = 3000
normal_capacity = 5000
low_capacity = 1000

# Backend selection strategy: "round_robin" | "least_loaded" | "preferred"
backend_selection = "least_loaded"
```

---

### `[backends]` — Execution backends

Each backend is optional. Disabled backends are skipped during selection.

#### Subprocess Backend

```toml
[backends.subprocess]
# Enable subprocess execution.
enabled = true

# Maximum concurrent subprocess executions.
max_concurrent = 30

# Default timeout for subprocess executions.
default_timeout_ms = 30000

# Maximum output bytes captured per stream.
max_output_bytes = 10485760        # 10 MB

# Allowed binaries (glob patterns). Empty = allow all configured tools.
allowed_binaries = ["/usr/bin/*", "/bin/*"]

# Denied binaries (glob patterns). Takes precedence over allowed_binaries.
denied_binaries = ["/usr/bin/su", "/usr/bin/sudo"]

# Working directory for executions. Empty = tool-defined or system default.
default_working_dir = "/tmp/ai-os/exec"

# Environment variables to set on every subprocess execution.
default_env = { PATH = "/usr/local/bin:/usr/bin:/bin", LANG = "C.UTF-8" }

# Environment variables to strip from inherited environment.
strip_env = ["HOME", "USER", "SSH_AUTH_SOCK"]
```

#### WASM Backend

```toml
[backends.wasm]
# Enable WASM execution.
enabled = true

# Maximum concurrent WASM executions.
max_concurrent = 40

# Default timeout for WASM executions.
default_timeout_ms = 10000

# Maximum WASM module size in bytes.
max_module_size = 10485760         # 10 MB

# WASM engine configuration.
[backends.wasm.engine]
# Fuel limit (instructions) per execution. 0 = unlimited.
fuel_limit = 1000000

# Stack size in bytes.
stack_size = 1048576              # 1 MB

# Enable WASI support.
wasi = true

# Pre-compile modules at startup for faster instantiation.
precompile = true

# Allowed WASM modules directory.
modules_dir = "/etc/ai-os/execution/wasm-modules"
```

#### Container Backend

```toml
[backends.container]
# Enable container execution.
enabled = false

# Maximum concurrent container executions.
max_concurrent = 10

# Default timeout for container executions.
default_timeout_ms = 120000

# Container runtime: "podman" | "docker" | "oci"
runtime = "podman"

# Container runtime endpoint (empty = default socket).
endpoint = "unix:///run/podman/podman.sock"

# Image pull policy: "always" | "if_not_present" | "never"
pull_policy = "if_not_present"

# Registry mirrors for image pull.
registry_mirrors = ["docker.io", "quay.io"]

# Maximum image size in bytes (0 = unlimited).
max_image_size = 0
```

---

### `[sandbox]` — Sandbox profiles

```toml
[sandbox]
# Default isolation level for tools without explicit profile.
default_isolation = "Process"     # "None" | "Process" | "Container" | "Wasm"

# Whether to enforce sandbox violation as hard error or warning.
strict_enforcement = true

# Predefined sandbox profiles.
[sandbox.profiles]

[sandbox.profiles.readonly]
isolation = "Process"
filesystem = "ReadOnly"
network = "None"
allow_process_spawn = false
capabilities = ["fs.read", "process.list", "system.info"]
seccomp_profile = "default"
read_only_rootfs = true

[sandbox.profiles.network_client]
isolation = "Process"
filesystem = "ReadOnly"
network = "OutboundOnly"
allow_process_spawn = false
capabilities = ["fs.read", "network.http", "network.dns"]
seccomp_profile = "default"

[sandbox.profiles.full_isolation]
isolation = "Container"
filesystem = "TempOnly"
network = "None"
allow_process_spawn = false
capabilities = []
seccomp_profile = "strict"
apparmor_profile = "ai-os-execution"
read_only_rootfs = true
tmpfs_size_bytes = 268435456       # 256 MB

# Per-capability sandbox mapping.
# Overrides default_isolation for specific capabilities.
[sandbox.capability_isolation]
"process.spawn" = "Container"
"network.listen" = "Container"
"dbus.call" = "Container"
"terminal.exec" = "Process"
"device.control" = "Container"
```

---

### `[tools]` — Tool registry

```toml
[tools]
# Directory to scan for tool registration files.
tools_dir = "/etc/ai-os/execution/tools"

# Auto-register tools found in tools_dir on startup.
auto_register = true

# Predefined tool registrations.
[tools.builtin]

[tools.builtin.read_file]
name = "Read File"
version = "1.0.0"
capability = "fs.read"
binding = { type = "Subprocess", binary = "/usr/bin/cat", default_args = [] }
sandbox_profile = "readonly"

[tools.builtin.list_processes]
name = "List Processes"
version = "1.0.0"
capability = "process.list"
binding = { type = "Subprocess", binary = "/usr/bin/ps", default_args = ["aux"] }
sandbox_profile = "readonly"

[tools.builtin.http_get]
name = "HTTP GET"
version = "1.0.0"
capability = "network.http"
binding = { type = "Subprocess", binary = "/usr/bin/curl", default_args = ["-s", "-S"] }
sandbox_profile = "network_client"

[tools.builtin.dns_lookup]
name = "DNS Lookup"
version = "1.0.0"
capability = "network.dns"
binding = { type = "Subprocess", binary = "/usr/bin/dig", default_args = ["+short"] }
sandbox_profile = "network_client"
```

---

### `[results]` — Result collection and routing

```toml
[results]
# Enable structured output parsing.
enable_parsing = true

# Maximum output size before parsing is skipped.
max_parse_bytes = 1048576          # 1 MB

# Registered output parsers.
parsers = ["json", "yaml", "csv", "raw"]

# Truncation behaviour: "truncate" | "error"
on_output_exceeded = "truncate"

# Default routing rules applied to all executions.
[results.routing]

[results.routing.defaults]
# Route all successful results to Memory (episodic store).
[[results.routing.defaults.rules]]
condition = "OnSuccess"
destination = { type = "Memory", ttl_ms = 86400000 }

# Route all failed results to Memory + Brain.
[[results.routing.defaults.rules]]
condition = "OnFailure"
destination = { type = "Broadcast", destinations = [
    { type = "Memory", ttl_ms = 259200000 },
    { type = "Brain" },
] }

# Route timed-out executions to Memory with escalation tag.
[[results.routing.defaults.rules]]
condition = "OnState"
state = "TimedOut"
destination = { type = "Memory", ttl_ms = 259200000 }
```

---

### `[recovery]` — Retry and rollback

```toml
[recovery]
# Enable automatic retry.
enable_retry = true

# Enable automatic rollback.
enable_rollback = true

# Default retry policy (applied when execution does not specify one).
[recovery.default_retry]
max_retries = 3
backoff_base_ms = 1000
backoff_multiplier = 2.0
backoff_max_ms = 60000
jitter = 0.25
retry_on_timeout = true
retry_on_exit_codes = [1, 127, 130, 137]
max_total_timeout_ms = 300000      # 5 minutes total across retries

# Per-capability retry overrides.
[recovery.capability_retry]
"network.http" = { max_retries = 5, backoff_base_ms = 500, retry_on_timeout = true }
"fs.write" = { max_retries = 3, backoff_base_ms = 2000, retry_on_timeout = false }
"fs.delete" = { max_retries = 1, backoff_base_ms = 1000 }

# Rollback configuration.
[recovery.rollback]
# Default rollback timeout in milliseconds.
default_timeout_ms = 30000

# What to do when rollback itself fails: "escalate" | "ignore" | "retry"
on_failure = "escalate"
```

---

### `[monitor]` — Execution monitoring

```toml
[monitor]
# Health check interval in seconds.
health_check_interval_s = 5

# Resource sampling interval in milliseconds.
resource_sample_interval_ms = 1000

# Timeout grace period as fraction of execution timeout.
timeout_grace_fraction = 0.1       # 10% of timeout, min 1s, max 10s

# Whether to enforce CPU limits.
enforce_cpu_limit = false

# Whether to enforce memory limits.
enforce_memory_limit = true

# Whether to enforce output size limits.
enforce_output_limit = true

# Warning threshold for concurrency (fraction of max).
concurrency_warning_threshold = 0.8
```

---

### `[permissions]` — Default execution permissions

```toml
[permissions]
# Default permissions granted to all executions (unless overridden by tool).
default_permissions = [
    "ReadFile(/tmp/ai-os/*)",
    "ReadFile(/var/log/*)",
    "SystemInfo",
]

# Capabilities that require explicit permission grants.
restricted_capabilities = [
    "process.spawn",
    "fs.write",
    "fs.delete",
    "network.listen",
    "device.control",
    "dbus.call",
]
```

---

## Configuration Lifecycle

1. **Load** — `execution.toml` is loaded from `/etc/ai-os/execution.toml` (default) or `EXECUTION_CONFIG_PATH` env var during coordinator `init()`.
2. **Validate** — All sections are validated. Invalid configuration is rejected with detailed error messages.
3. **Apply** — Configuration is distributed to each pipeline stage. Stages apply relevant sections.
4. **Reload** — On `core.config_changed` event, the coordinator re-reads the config file and applies changed sections. Stages are updated in order:
   - First: Pipeline topology changes (if `pipeline.stage_order` changed)
   - Second: Dispatch configuration
   - Third: Backend configuration
   - Fourth: Sandbox profiles
   - Fifth: Tool registry
   - Sixth: Routing rules
   - Seventh: Recovery policies
   - Last: Monitoring parameters
5. **Fallback** — If a reload fails validation, the previous configuration is retained. An error event is published.

---

## Environment Variable Overrides

| Variable | Overrides | Example |
|---|---|---|
| `EXECUTION_CONFIG_PATH` | config file path | `/data/execution.toml` |
| `EXECUTION_MAX_CONCURRENCY` | `dispatch.max_concurrency` | `100` |
| `EXECUTION_DEFAULT_TIMEOUT_MS` | `execution.default_timeout_ms` | `60000` |
| `EXECUTION_ENABLE_WASM` | `backends.wasm.enabled` | `true` |
| `EXECUTION_ENABLE_CONTAINER` | `backends.container.enabled` | `false` |
| `EXECUTION_STRICT_SANDBOX` | `sandbox.strict_enforcement` | `true` |

---

## References

- [Execution Platform Architecture](../architecture/execution.md)
- [Execution Platform Interfaces](../interfaces/execution.md)
- [Execution Pipeline Design](../execution-pipeline.md)
- [RFC-0005: Execution Platform](../rfc/RFC-0005-execution-platform.md)
