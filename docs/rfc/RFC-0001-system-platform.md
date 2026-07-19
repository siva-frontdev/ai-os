# RFC-0001: OSAL — Operating System Abstraction Layer

| Field | Value |
|---|---|
| **Status** | Accepted |
| **Author** | Project maintainers |
| **Phase** | Phase 4 |
| **Created** | 2025-02-01 |
| **Updated** | 2025-07-19 |
| **Requires** | RFC-0001 is the first RFC; no dependencies |
| **Supersedes** | None |

## Abstract

This RFC defines the Operating System Abstraction Layer (OSAL), Phase 4 of the AI-native OS architecture. OSAL is Layer 1 of the platform — the lowest internal layer, sitting directly above the Linux kernel. It insulates every module above it from OS-specific details through abstract trait interfaces and a capability-gated access model. OSAL provides 8 subsystem traits (FileSystem, ProcessManager, Terminal, NetworkManager, SystemMonitor, DeviceManager, UserManager, PlatformInfo) organized across 11 crates, with a single KernelFacade as the entry point. The Linux implementation lives exclusively in the `osal-linux` crate — the only crate in the platform permitted to use unsafe code and call libc directly.

## Motivation

Without OSAL, AI-native OS is an application framework, not an operating platform. The Core layer provides communication and lifecycle infrastructure; the Runtime layer provides task scheduling and agent management. Neither interacts with the host OS directly. To spawn processes, monitor system health, watch files, manage network interfaces, collect system events, or manage services, a unified abstraction over the host OS is required.

OSAL fills this gap at the correct architectural layer. It sits below Core and Runtime, not between them and Linux. Core depends on OSAL for filesystem access, process management, and system information. Runtime depends on OSAL through Core. No higher layer ever references OSAL directly — the dependency flows downward through the stack.

The critical architectural boundary: OSAL is the **only** layer that knows about Linux. Every higher-layer operation — context gathering for the Brain, sensor input for Perception, file I/O for Memory, process execution for the Execution layer — ultimately passes through OSAL trait interfaces, never through libc or syscalls directly.

Doing nothing means higher layers would need to embed raw libc calls, /proc parsing, and D-Bus invocations directly, violating clean architecture and duplicating error handling, security checks, capability enforcement, and event instrumentation across every module.

## Design

### Overview

OSAL is organized as 11 independent crates in the Cargo workspace, each with a single responsibility. The architecture follows the Kernel Facade pattern: a single `KernelFacade` struct holds `Arc<dyn Trait>` references to all 8 subsystem implementations. Higher layers interact with OSAL exclusively through `KernelFacade`, never through concrete implementations.

The 11 crates are:
- **osal-core**: Base types (Pid, Uid, Fd, etc.), OsalError hierarchy, OsalEvent enum, KernelFacade struct, and the 8 subsystem trait definitions
- **osal-capabilities**: Capability enum (37 variants), CapabilitySet, CapabilityContext
- **osal-filesystem**: FileSystem trait with 14 methods, FileMetadata, FileEvent, FilesystemError
- **osal-process**: ProcessManager trait, ProcessConfig, ProcessHandle, ProcessInfo, ProcessEvent, ProcessError
- **osal-terminal**: Terminal trait, CommandConfig, PtyConfig, PtySession, TerminalOutput, TerminalError
- **osal-network**: NetworkManager trait, NetworkInterface, DnsConfig, PingResult, NetworkEvent, NetworkError
- **osal-monitoring**: SystemMonitor trait, CpuSnapshot, MemorySnapshot, DiskSnapshot, MonitoringEvent, MonitorError
- **osal-devices**: DeviceManager trait, DeviceInfo, DeviceType, DeviceEvent, DeviceError
- **osal-users**: UserManager trait, UserInfo, GroupInfo, Credential, UserEvent, UserError
- **osal-platform**: PlatformInfo trait, OsInformation, KernelInformation, HardwareInformation, PlatformError
- **osal-linux**: LinuxKernelFacade implementing all OSAL traits. The ONLY crate permitted to use unsafe code and call libc

Every subsystem trait is defined in osal-core and implemented in osal-linux. The ten interface crates (core through platform) are `#![forbid(unsafe_code)]`. Only osal-linux allows unsafe.

### Capability Framework

Every OSAL operation is gated by a capability check. The capability framework is defined in `osal-capabilities`:

- **Capability enum** — 37 variants organized by subsystem (FileRead, FileWrite, ProcessSpawn, TerminalExecute, NetworkConfigure, MonitorCpu, etc.)
- **CapabilitySet** — A `HashSet<Capability>` with grant/revoke/check methods. Path-based capabilities support wildcard matching. The `Admin` variant grants all capabilities.
- **CapabilityContext** — Associates a subject identity with their CapabilitySet. Every OSAL trait method takes `&CapabilityContext`.

The capability check happens at the trait entry point, before any kernel access. If the capability is absent, the method returns `OsalError::CapabilityDenied(Capability)` without touching the OS. This prevents even transient kernel access for unauthorized operations.

Capabilities integrate with the Runtime's PermissionChecker (Phase 3), which maps session roles to capability sets.

```
ai_os_system
  +-- system_resource/     SystemResourceManager
  +-- process/             ProcessManager
  +-- filesystem/          FileSystemProvider
  +-- network/             NetworkManager
  +-- event_collector/     SystemEventCollector
  +-- service_manager/     ServiceManager
  +-- error.rs             SystemError enum
  +-- lib.rs               Crate root, re-exports
```

Subsystems are independent of each other by design. SystemResourceManager does not call ProcessManager; NetworkManager does not depend on FileSystemProvider. The only shared dependency is the EventBus from Core. This isolation ensures that a failure in one subsystem does not cascade to others, and that each subsystem can be tested and maintained independently.

Each subsystem follows the same internal structure:

- A `pub trait` defining its capability (dyn-compatible, `Debug + Send + Sync`)
- A `Default<Capability>` struct that implements the trait
- A `Config` struct (Deserialize) for that subsystem's configuration
- A set of event types (structs implementing `Event` from Core)
- An error variant in `SystemError`

The crate-level `SystemPlatform` facade struct holds `Arc<dyn Trait>` references to all six subsystems, implements `Service` itself, and delegates lifecycle calls. Upper layers interact with individual subsystems through their traits, not through the facade.

### Detailed Design

#### Interfaces

**SystemResourceManager**

```rust
/// Aggregate snapshot of host resource utilisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub cpu: CpuSnapshot,
    pub memory: MemorySnapshot,
    pub disk: Vec<DiskSnapshot>,
    pub network: Vec<NetworkSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuSnapshot {
    pub cores: Vec<CpuCore>,
    pub load_avg: [f64; 3],
    pub context_switches: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub total_kb: u64,
    pub available_kb: u64,
    pub cached_kb: u64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,
}

pub trait SystemResourceManager: Debug + Send + Sync {
    /// Take an immediate resource snapshot. Non-blocking; reads cached values.
    fn snapshot(&self) -> Result<ResourceSnapshot, SystemError>;

    /// Get current resource alert thresholds.
    fn thresholds(&self) -> Vec<ResourceThreshold>;

    /// Set or update a resource threshold. Triggers re-evaluation on next collection cycle.
    fn set_threshold(&self, threshold: ResourceThreshold) -> Result<(), SystemError>;

    /// Get the configured collection interval.
    fn collection_interval(&self) -> Duration;
}
```

**ProcessManager**

```rust
/// Handle representing a spawned child process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessHandle {
    pub pid: libc::pid_t,
    pub task_id: Uuid,
    pub spawned_at: DateTime<Utc>,
}

/// Configuration for spawning a child process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,
    pub resource_limits: ResourceLimits,
    pub timeout: Option<Duration>,
    pub session_id: Option<Uuid>,
}

pub trait ProcessManager: Debug + Send + Sync {
    /// Spawn a child process. Returns a ProcessHandle for tracking.
    fn spawn(&self, config: SpawnConfig) -> Result<ProcessHandle, SystemError>;

    /// Send a signal to a process.
    fn signal(&self, pid: libc::pid_t, signal: libc::c_int) -> Result<(), SystemError>;

    /// Get current process state.
    fn status(&self, pid: libc::pid_t) -> Result<ProcessStatus, SystemError>;

    /// List all tracked processes, optionally filtered by session.
    fn list(&self, session_id: Option<Uuid>) -> Result<Vec<ProcessHandle>, SystemError>;

    /// Subscribe to state changes for a specific process.
    fn watch(&self, pid: libc::pid_t) -> Result<watch::Receiver<ProcessStatus>, SystemError>;
}
```

**FileSystemProvider**

```rust
pub trait FileSystemProvider: Debug + Send + Sync {
    /// Read entire file contents as bytes. Path is canonicalized and validated.
    fn read(&self, path: &Path) -> Result<Vec<u8>, SystemError>;

    /// Write bytes to a file (creates or truncates). Path is validated.
    fn write(&self, path: &Path, data: &[u8]) -> Result<(), SystemError>;

    /// Append bytes to a file. Path is validated.
    fn append(&self, path: &Path, data: &[u8]) -> Result<(), SystemError>;

    /// Stat a file or directory.
    fn stat(&self, path: &Path) -> Result<FileMetadata, SystemError>;

    /// List directory contents with optional glob filter.
    fn list(&self, path: &Path, pattern: Option<&str>) -> Result<Vec<DirEntry>, SystemError>;

    /// Create a temporary file with automatic cleanup on drop or session end.
    fn temp_file(&self, prefix: &str, session_id: Uuid) -> Result<NamedTempFile, SystemError>;

    /// Watch a path for filesystem events. Returns a stream of FileEvent.
    fn watch(&self, path: &Path) -> Result<Pin<Box<dyn Stream<Item = FileEvent> + Send>>, SystemError>;

    /// Register an allowed root directory. Paths outside all allowed roots are rejected.
    fn add_allowed_root(&self, root: PathBuf) -> Result<(), SystemError>;

    /// Remove an allowed root directory.
    fn remove_allowed_root(&self, root: &Path) -> Result<(), SystemError>;
}
```

**NetworkManager**

```rust
pub trait NetworkManager: Debug + Send + Sync {
    /// List all network interfaces.
    fn interfaces(&self) -> Result<Vec<NetworkInterface>, SystemError>;

    /// Get detailed info about a specific interface.
    fn interface(&self, name: &str) -> Result<NetworkInterface, SystemError>;

    /// Resolve a hostname to addresses asynchronously.
    fn resolve(&self, hostname: &str) -> Result<Vec<IpAddr>, SystemError>;

    /// Apply a firewall rule set via nftables JSON API.
    fn apply_firewall_rules(&self, rules: FirewallRuleSet) -> Result<(), SystemError>;

    /// Subscribe to interface state change events.
    fn watch_interfaces(&self) -> Result<Pin<Box<dyn Stream<Item = InterfaceEvent> + Send>>, SystemError>;
}
```

**SystemEventCollector**

```rust
pub trait SystemEventCollector: Debug + Send + Sync {
    /// Enable collection from a specific system event source.
    fn enable_source(&self, source: EventSource) -> Result<(), SystemError>;

    /// Disable collection from a specific source.
    fn disable_source(&self, source: EventSource) -> Result<(), SystemError>;

    /// List currently active event sources.
    fn active_sources(&self) -> Vec<EventSource>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSource {
    Udev,
    Inotify,
    Netlink,
    DbusSystem,
    Journal,
}
```

**ServiceManager**

```rust
pub trait ServiceManager: Debug + Send + Sync {
    /// Start a systemd unit.
    fn start_unit(&self, unit: &str) -> Result<UnitState, SystemError>;

    /// Stop a systemd unit.
    fn stop_unit(&self, unit: &str) -> Result<UnitState, SystemError>;

    /// Restart a systemd unit.
    fn restart_unit(&self, unit: &str) -> Result<UnitState, SystemError>;

    /// Enable a systemd unit for automatic start on boot.
    fn enable_unit(&self, unit: &str) -> Result<(), SystemError>;

    /// Disable a systemd unit.
    fn disable_unit(&self, unit: &str) -> Result<(), SystemError>;

    /// Get current state of a systemd unit.
    fn unit_status(&self, unit: &str) -> Result<UnitState, SystemError>;

    /// List units matching an optional filter pattern.
    fn list_units(&self, pattern: Option<&str>) -> Result<Vec<UnitInfo>, SystemError>;

    /// Subscribe to state changes for a specific unit.
    fn watch_unit(&self, unit: &str) -> Result<watch::Receiver<UnitState>, SystemError>;
}
```

#### Events

**Published Events**

| Direction | Event Type | Payload | Description |
|---|---|---|---|
| Published | `system.resource.snapshot` | `ResourceSnapshot` | Emitted at the end of each collection cycle with current metrics |
| Published | `system.resource.threshold_breached` | `ThresholdAlert { threshold, current_value, direction }` | Emitted when a metric crosses a configured threshold |
| Published | `system.process.spawned` | `ProcessSpawned { pid, task_id, command, session_id }` | Emitted when a child process is successfully spawned |
| Published | `system.process.exited` | `ProcessExited { pid, exit_code, signal, runtime }` | Emitted when a tracked process exits, with exit code or signal |
| Published | `system.process.state_changed` | `ProcessStateChanged { pid, from, to }` | Emitted on state transitions (running, stopped, zombie, exited) |
| Published | `system.file.created` | `FileEvent { path, kind, timestamp }` | Emitted when a file is created in a watched directory |
| Published | `system.file.modified` | `FileEvent { path, kind, timestamp }` | Emitted when a watched file content changes |
| Published | `system.file.deleted` | `FileEvent { path, kind, timestamp }` | Emitted when a file is removed from a watched directory |
| Published | `system.network.interface_up` | `InterfaceEvent { name, addresses, flags }` | Emitted when a network interface transitions to UP |
| Published | `system.network.interface_down` | `InterfaceEvent { name, addresses, flags }` | Emitted when a network interface transitions to DOWN |
| Published | `system.network.interface_address_changed` | `InterfaceAddressEvent { name, added, removed }` | Emitted when addresses are added or removed from an interface |
| Published | `system.device.added` | `DeviceEvent { subsystem, devtype, syspath, properties }` | Emitted by SystemEventCollector on udev device add |
| Published | `system.device.removed` | `DeviceEvent { subsystem, devtype, syspath, properties }` | Emitted by SystemEventCollector on udev device remove |
| Published | `system.device.changed` | `DeviceEvent { subsystem, devtype, syspath, properties }` | Emitted by SystemEventCollector on udev device change |
| Published | `system.service.unit_state_changed` | `UnitStateEvent { unit, active_state, sub_state }` | Emitted on systemd unit state transitions |
| Published | `system.service.unit_failed` | `UnitStateEvent { unit, active_state, sub_state }` | Emitted when a systemd unit enters failed or error state |

**Consumed Events**

| Direction | Event Type | Source | Purpose |
|---|---|---|---|
| Consumed | `runtime.task_completed` | Runtime TaskManager | Clean up process tracking for associated child process |
| Consumed | `runtime.task_failed` | Runtime TaskManager | Kill and clean up associated process tree |
| Consumed | `runtime.task_cancelled` | Runtime TaskManager | Kill and clean up associated process tree |
| Consumed | `runtime.session_destroyed` | Runtime SessionManager | Clean up session-scoped processes, temp files, and network resources |
| Consumed | `system.resource.threshold_breached` | SystemResourceManager (self) | Trigger escalation if ResourceManager cannot handle threshold breach |

#### Dependencies

| Dependency | Layer | Purpose |
|---|---|---|
| `ai_os_core` | Core | EventBus, Service trait, Logger, HealthMonitor, Container |
| `ai_os_runtime` | Runtime | ResourceManager (limit enforcement), PermissionChecker (authorization), ContextManager (trace propagation) |
| `tokio` | External | Async runtime, `tokio::process::Command`, `tokio::fs`, `tokio::signal`, `tokio::sync` |
| `libc` | External | POSIX process APIs (fork, exec, waitpid, kill, getrusage), resource queries (sysinfo, getloadavg) |
| `procfs` | External | Parse `/proc` filesystem for CPU, memory, and per-process metrics |
| `inotify` | External | Linux inotify wrapper for file and directory watching (FileSystemProvider, SystemEventCollector) |
| `zbus` | External | Async D-Bus client for systemd (ServiceManager) and systemd-resolved (NetworkManager) |
| `rtnetlink` | External | Netlink protocol for network interface enumeration and monitoring (NetworkManager) |
| `trust-dns-resolver` | External | Async DNS resolution (NetworkManager) |
| `udev` | External | udev monitor for device events (SystemEventCollector) |
| `serde` / `serde_json` | External | Event serialization and deserialization |
| `thiserror` | External | Error type derivation |
| `chrono` | External | Timestamps for events and metrics |
| `uuid` | External | Task ID and session ID generation |
| `tempfile` | External | Temporary file creation with automatic cleanup (FileSystemProvider) |

The System Platform depends on nothing internal except Core and Runtime. It does not depend on Memory, Brain, Perception, or Execution layers. This is the strictest enforcement of the dependency rule: System is layer 1 (counting from 0), Core is layer 2, Runtime is layer 3, and the dependency direction is Runtime -> Core -> System.

#### Configuration

All configuration is loaded from the platform's layered config system (defaults, config file, environment variables) under the `system` key prefix.

| Key | Type | Default | Description |
|---|---|---|---|
| `system.resource.collection_interval_ms` | `u64` | `5000` | Interval between resource metric collections (milliseconds) |
| `system.resource.cpu_warn_threshold` | `f64` | `90.0` | CPU utilisation percentage that triggers a warning event |
| `system.resource.memory_warn_threshold` | `f64` | `85.0` | Memory utilisation percentage that triggers a warning event |
| `system.resource.disk_warn_threshold` | `f64` | `90.0` | Disk utilisation percentage that triggers a warning event |
| `system.filesystem.allowed_roots` | `Vec<String>` | `["/tmp", "/var/lib/ai-os"]` | Directories allowed for file read/write operations |
| `system.filesystem.max_watch_fds` | `usize` | `128` | Maximum number of concurrent inotify watch descriptors |
| `system.network.dns_servers` | `Vec<String>` | `[]` | Custom DNS servers; empty uses system defaults |
| `system.network.interface_whitelist` | `Vec<String>` | `[]` | Explicitly managed interfaces; empty allows all |
| `system.event_collector.sources` | `Vec<String>` | `["udev"]` | Enabled event sources (udev, inotify, netlink, dbus, journal) |
| `system.service_manager.dbus_timeout_ms` | `u64` | `5000` | D-Bus method call timeout for systemd operations (milliseconds) |

#### Thread Model

The System Platform runs entirely on the existing multi-threaded Tokio runtime shared with Core and Runtime. No dedicated OS threads are created.

| Subsystem | Thread Model | Details |
|---|---|---|
| SystemResourceManager | Periodic Tokio task | A single `tokio::spawn` loop with `tokio::time::interval`. Reads `/proc` and `/sys` via `spawn_blocking` for file I/O, then publishes events. Cached metrics behind `RwLock`. |
| ProcessManager | Tokio task per process + signal handler | `tokio::process::Command` manages child processes asynchronously. A dedicated task handles `SIGCHLD` via `tokio::signal::unix::Signal`. Process map behind `RwLock<HashMap<Pid, ProcessInfo>>`. |
| FileSystemProvider | Tokio task per inotify watch | One task per `inotify::WatchDescriptor` reads events from the inotify fd and publishes FileEvent. Watch registry behind `RwLock`. File I/O uses `tokio::fs`. |
| NetworkManager | Tokio task for netlink event stream | `rtnetlink` connection runs in a dedicated task. Interface cache behind `RwLock`. D-Bus calls via `zbus` are async and non-blocking. DNS resolution via `trust-dns-resolver`. |
| SystemEventCollector | One Tokio task per event source | Each source (udev, inotify, netlink, dbus, journal) gets its own spawned task that reads from a fd or socket and publishes normalized events. Sources are independent; one failing does not affect others. |
| ServiceManager | Shared D-Bus connection | A single async D-Bus connection to `org.freedesktop.systemd1`. `zbus` handles reconnection. Signal handler task for `PropertiesChanged` signals. No explicit locks; `zbus` manages connection state internally. |

All blocking system calls (readdir, read from /proc, libc sysinfo) are dispatched via `tokio::task::spawn_blocking` to avoid starving the async runtime. The `spawn_blocking` pool is sized to handle the maximum number of concurrent blocking operations across all subsystems (estimated at 16 threads).

#### Lifecycle

Each subsystem implements `Service` from Core. The `SystemPlatform` facade implements `Service` by delegating to its subsystems in a defined order.

**Init order** (dependencies declared first):

1. `SystemEventCollector` -- must be ready first since it provides device events that other subsystems consume during their own init.
2. `FileSystemProvider` -- provides filesystem access needed by config validation and temp file creation for other subsystems.
3. `SystemResourceManager` -- starts collecting immediately; no dependencies on other system subsystems.
4. `NetworkManager` -- requires udev events (from SystemEventCollector) to detect interface changes.
5. `ProcessManager` -- no dependencies on other system subsystems; spawns processes as requested.
6. `ServiceManager` -- last because it manages systemd units that higher layers may depend on.

**Init phase**: Each subsystem validates its configuration, opens necessary handles (D-Bus connection, inotify fd, netlink socket), registers event subscriptions, and registers health checks. All handles are opened during init to fail early.

**Start phase**: Background collection tasks are spawned. The subsystem begins publishing events.

**Stop phase** (reverse order): Background tasks are cancelled via their cancel token. Handles are closed gracefully (D-Bus disconnect, inotify watch removal, process group cleanup). The subsystem unregisters its event subscriptions.

**Health checks per subsystem**:

| Subsystem | Health Check | Probe |
|---|---|---|
| SystemResourceManager | Can read /proc/stat | Spawn blocking read of first 1 KB of /proc/stat |
| ProcessManager | Process map accessible | Lock and read process count |
| FileSystemProvider | Allowed roots accessible | Stat each allowed root directory |
| NetworkManager | D-Bus and netlink connectivity | Send netlink RTM_GETLINK dump, verify response |
| SystemEventCollector | Event source fds healthy | Check each source fd with poll(2) via `tokio::io::Interest::READABLE` |
| ServiceManager | D-Bus connectivity | Call `org.freedesktop.DBus.Peer.Ping` on systemd bus |

#### Error Handling

Errors are unified in `SystemError`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("resource collection failed: {0}")]
    ResourceCollectionFailed(String),

    #[error("process spawn failed: {0}")]
    ProcessSpawnFailed(String),

    #[error("process signal failed (pid={pid}, signal={signal}): {details}")]
    ProcessSignalFailed { pid: libc::pid_t, signal: libc::c_int, details: String },

    #[error("process not found: {0}")]
    ProcessNotFound(libc::pid_t),

    #[error("filesystem permission denied: {0}")]
    FileSystemPermissionDenied(String),

    #[error("filesystem I/O error: {0}")]
    FileSystemIoError(#[from] std::io::Error),

    #[error("path traversal blocked: {0}")]
    PathTraversalBlocked(String),

    #[error("inotify watch limit reached: current={current}, max={max}")]
    InotifyWatchLimitReached { current: usize, max: usize },

    #[error("network interface not found: {0}")]
    NetworkInterfaceNotFound(String),

    #[error("firewall rule invalid: {0}")]
    FirewallRuleInvalid(String),

    #[error("DNS resolution failed: {0}")]
    DnsResolutionFailed(String),

    #[error("D-Bus connection failed: {0}")]
    DbusConnectionFailed(String),

    #[error("D-Bus method call failed ({destination}.{path}.{interface}.{method}): {details}")]
    DbusMethodCallFailed { destination: String, path: String, interface: String, method: String, details: String },

    #[error("systemd unit action failed: {0}")]
    SystemdUnitActionFailed(String),

    #[error("udev monitor error: {0}")]
    UdevError(String),

    #[error("netlink error: {0}")]
    NetlinkError(String),

    #[error("source disconnected: {0}")]
    EventSourceDisconnected(String),

    #[error("core error: {0}")]
    Core(#[from] CoreError),

    #[error("runtime error: {0}")]
    Runtime(#[from] RuntimeError),

    #[error("{0}")]
    General(String),
}
```

**Failure classification and recovery**:

| Error Class | Examples | Classification | Recovery |
|---|---|---|---|
| Resource collection failure | /proc/stat unreadable, sysinfo returns -1 | Transient | Retry on next collection interval. Publish warning. After 5 consecutive failures, mark subsystem unhealthy. |
| Process spawn/exec failure | fork returns -1 (EAGAIN), execve returns ENOENT | Persistent | Return error to caller. Log at ERROR. No retry at platform level; caller decides. |
| Path traversal blocked | path canonicalizes outside allowed root | Persistent | Return `FileSystemPermissionDenied` immediately. No retry. Log at WARN with security context. |
| Inotify watch limit | max_user_watches exhausted | Persistent | Publish critical event for operator. Fall back to polling for new watches. |
| D-Bus connection loss | dbus-daemon restart, timeout | Transient | Exponential backoff reconnect (100ms, 500ms, 2s, 10s). Queue operations during reconnect (bounded queue of 64 entries). |
| Netlink socket error | buffer overflow, socket closed | Transient | Reopen socket. Re-request interface dump. |
| udev monitor disconnect | udev daemon restart | Transient | Reopen udev socket. Re-enumerate existing devices. |
| DNS resolution timeout | resolver cannot reach upstream | Transient | Return error to caller with `DnsResolutionFailed`. Retry with backoff internally. |

### Security Considerations

**Path traversal prevention**: The FileSystemProvider canonicalizes every requested path via `std::fs::canonicalize` and verifies the resolved path is a prefix of at least one registered allowed root. This prevents symlink-based escapes. Allowed roots are set at startup via configuration and can be updated at runtime with `PermissionChecker` authorization.

**Process sandboxing**: The ProcessManager applies resource limits via `setrlimit` (RLIMIT_CPU, RLIMIT_AS, RLIMIT_NOFILE, RLIMIT_NPROC) before `exec`. Linux namespace isolation (user, pid, net) is planned but deferred to a follow-up RFC. All child processes are tracked and forcefully killed if their parent task is cancelled.

**D-Bus authorization**: The ServiceManager uses the system bus, which enforces polkit authorization. D-Bus calls to systemd are scoped to the user's systemd instance (`--user` bus) by default. For system-level operations, the platform requires explicit configuration and appropriate polkit rules.

**Netlink sandboxing**: Netlink sockets for RTM_GETLINK and RTM_NEWLINK are unprivileged and readable by any process. Firewall operations via nftables JSON API require `CAP_NET_ADMIN`. The NetworkManager checks `PermissionChecker` before issuing any nftables command.

**Event data minimisation**: System events (udev, netlink) are filtered to include only fields defined in the event schema. Raw kernel payloads are discarded after parsing. This prevents accidental leakage of kernel heap data through event serialization.

**No secrets in events**: Process environment variables are stored in the process map but never included in published events. Only the command name, PID, and exit status are published.

### Performance Considerations

**Collection overhead**: SystemResourceManager reads /proc/stat and /proc/meminfo on every collection cycle. These are sequential reads of small files (< 4 KB each). At the default 5-second interval, this adds negligible overhead (< 0.1% CPU on idle system). Disk and network collection adds one read per mount point and interface.

**spawn_blocking pool sizing**: The default Tokio blocking pool of 512 threads is more than adequate. At peak, the System Platform concurrently uses at most: 1 (SystemResourceManager) + N processes (ProcessManager) + M inotify watches (FileSystemProvider) + 1 (NetworkManager netlink) + S event sources (SystemEventCollector) = typically under 50 concurrent blocking operations.

**Event throughput**: System events are high-volume (especially udev during device enumeration, inotify under heavy file churn). Each event is small (< 1 KB serialized). EventBus dispatch is non-blocking; the bottleneck is serialization and subscriber processing. Performance targets for system events: publish-to-dispatch latency < 500 µs p50, < 5 ms p99 at 1,000 events/second.

**Memory overhead per resource**: Resource snapshots are allocated, serialized to JSON for the event payload, and then dropped. Peak memory per collection cycle is approximately 64 KB. Cached state (current and previous snapshot for delta computation) is approximately 4 KB.

**D-Bus performance**: systemd D-Bus calls typically complete in 1-10 ms. The ServiceManager batches status queries where possible. Signal subscriptions are preferred over polling for unit state changes.

### Testing Strategy

Each subsystem follows a consistent testing approach. Kernel interfaces are abstracted behind traits that can be mocked for unit tests. Integration tests use the real interfaces against a controlled environment.

**Unit tests**:

- Every trait method has a happy-path test and at least one error-path test.
- Configuration parsing tests verify defaults, valid values, and invalid values.
- Event serialization round-trip tests verify that events survive serialize + deserialize with identical fields.
- Mock implementations of kernel interfaces (e.g., `MockProcFs`, `MockDbusConnection`) are used to test error handling without root privileges.

**Mock kernel interfaces**:

| Subsystem | Mock Strategy |
|---|---|
| SystemResourceManager | `ProcFsReader` trait with `RealProcFsReader` and `MockProcFsReader`. Mock returns canned data for /proc/stat, /proc/meminfo, /proc/diskstats, /proc/net/dev. |
| ProcessManager | `ProcessSpawner` trait wraps `tokio::process::Command`. Mock records spawn calls and returns configurable results. `SigChldHandler` trait abstracts signal stream. |
| FileSystemProvider | `FsBackend` trait with `TokioFsBackend` and `MockFsBackend` (in-memory filesystem). `InotifyBackend` trait with mock that returns FileEvent streams directly. |
| NetworkManager | `NetlinkClient` trait with mock that returns canned interface lists. `DnsResolver` trait with `TrustDnsResolver` and `MockDnsResolver`. |
| SystemEventCollector | `UdevMonitor` trait with mock that emits DeviceEvent sequences. `DbusSignalStream` trait with mock. |
| ServiceManager | `SystemdDbusClient` trait with mock that records method calls and returns configurable UnitState values. |

**Integration tests**:

- SystemResourceManager: Run against a real /proc filesystem in CI container. Verify that snapshots contain non-zero values for expected fields.
- ProcessManager: Spawn `sleep 1`, verify ProcessSpawned and ProcessExited events are published correctly.
- FileSystemProvider: Create files in /tmp, verify read/write/list round-trip. Verify path traversal blocking with a symlink attack test.
- NetworkManager: Verify interface listing works on a real network stack (CI container with at least loopback). DNS resolution against localhost.
- SystemEventCollector: Verify udev source connects and receives events (requires udev running in CI).
- ServiceManager: Run inside a systemd user session. Start a user service, verify UnitStateEvent.

**Cargo features for tests**:

- `test-mocks` (dev-dependency only): Enables mock implementations for all kernel interfaces. Used by all unit tests.
- `integration-real` (dev-dependency only): Enables integration tests that require real kernel interfaces. Skipped in CI unless explicitly enabled.

**Property-based tests**:

- Path canonicalization: `proptest` generates random paths with symlink components; verify that canonicalized path is always within an allowed root or `PathTraversalBlocked` is returned.
- Resource snapshot delta computation: Generate random previous and current snapshots; verify that deltas are non-negative and rates are within expected bounds.

## Drawbacks

1. **Increased complexity**: Adding six subsystems increases the platform's codebase size by approximately 15,000-20,000 lines of Rust. Each subsystem introduces new failure modes, configuration surface, and testing burden.

2. **Linux-specific interfaces**: The System Platform is tightly coupled to Linux kernel interfaces (/proc, /sys, inotify, netlink, udev, D-Bus, systemd). Porting to another OS would require a complete rewrite of this layer. This is acceptable because the project targets Arch Linux exclusively, but it does reduce future flexibility.

3. **D-Bus dependency**: D-Bus is a complex inter-process communication system with its own failure modes (timeouts, disconnections, policy misconfigurations). Adding D-Bus as a dependency introduces a significant external system that must be running and correctly configured for the platform to function fully.

4. **Elevated privileges**: Some operations (nftables firewall rules, system-level systemd unit management) require elevated privileges. Running the platform with CAP_NET_ADMIN or as a systemd system unit increases the attack surface.

## Alternatives Considered

| Alternative | Reason for Rejection |
|---|---|
| **Integrate system operations directly into Core and Runtime** | Violates clean architecture. Core and Runtime would need to know about /proc, inotify, and D-Bus. Each would need its own error handling, configuration, and security checks for kernel interfaces. |
| **Use a single monolithic SystemService instead of six subsystems** | A single large service would violate single responsibility. Failures in resource collection would affect process management. Testing would be more complex. Configuration would be harder to reason about. |
| **Delegate all system operations to external CLI tools (procps, ip, systemctl)** | Shelling out to CLI tools is slower (process spawn overhead), less reliable (string parsing of output), and less secure (argument injection risk). D-Bus and library interfaces provide typed, async, and safe alternatives. |
| **Use io_uring instead of tokio::fs for all filesystem operations** | io_uring offers better I/O performance but requires the `tokio-uring` crate, which is less mature than `tokio::fs`. The `tokio::fs` implementation already uses `spawn_blocking` for blocking calls. io_uring can be added later as a backend without changing the FileSystemProvider trait. |
| **Use fanotify instead of inotify for file watching** | fanotify provides system-wide file access monitoring but requires CAP_SYS_ADMIN. inotify works without elevated privileges and is available in containerized environments. fanotify can be added as an additional watch backend later. |
| **Use NetworkManager D-Bus API instead of rtnetlink for interface management** | NetworkManager adds a heavyweight dependency with its own state machine and configuration model. rtnetlink is kernel-direct, faster, and has fewer moving parts. NetworkManager integration can be added as an alternative backend. |

## Open Questions

1. Should the ProcessManager support cgroups v2 for resource enforcement on child processes, or should that be deferred to a follow-up RFC? The current design uses setrlimit only, which is coarser than cgroups.

2. Should the ServiceManager manage both system-level and user-level systemd instances, or only user-level? Managing system-level units requires privilege escalation (polkit or root).

3. What is the exact event volume that the SystemEventCollector must handle under peak load (e.g., USB storm during device enumeration)? This determines buffer sizes and backpressure strategy.

4. Should the NetworkManager support wireguard interface management directly, or delegate to systemd-networkd/wg-quick? Direct support would require additional netlink and crypto dependencies.

## Implementation Plan

1. **Crate scaffolding and error types** (estimated effort: small)
   - Create `system/` crate in workspace with `Cargo.toml`, `lib.rs`, `error.rs`.
   - Define `SystemError` enum with all variants.
   - Define `SystemPlatform` facade struct with `Arc<dyn Trait>` fields.
   - Add `ai-os-system` to workspace `members` in root `Cargo.toml`.

2. **SystemResourceManager** (estimated effort: medium)
   - Implement `ProcFsReader` trait with real and mock backends.
   - Implement `CpuSnapshot`, `MemorySnapshot`, `DiskSnapshot`, `NetworkSnapshot` collection.
   - Implement collection loop with configurable interval and `spawn_blocking`.
   - Implement threshold evaluation and alert publishing.
   - Integration test against real /proc.

3. **ProcessManager** (estimated effort: medium)
   - Implement process spawn, signal, wait, and status tracking.
   - Integrate with `tokio::process::Command` and `tokio::signal::unix::Signal` for SIGCHLD.
   - Implement process map with `RwLock<HashMap<Pid, ProcessInfo>>`.
   - Implement session-scoped process cleanup (consume `session_destroyed` event).
   - Integration test spawning and tracking a child process.

4. **FileSystemProvider** (estimated effort: medium)
   - Implement async file I/O (read, write, append, stat, list) via `tokio::fs`.
   - Implement path canonicalization and allowed-root validation.
   - Implement inotify-based file watching with `inotify` crate and async adapter.
   - Implement temporary file creation with session-scoped cleanup.
   - Unit test path traversal blocking with symlink scenarios.

5. **NetworkManager** (estimated effort: large)
   - Implement interface enumeration via `rtnetlink`.
   - Implement async DNS resolution via `trust-dns-resolver`.
   - Implement D-Bus integration with systemd-resolved (optional, for DNS configuration).
   - Implement firewall rule application via nftables JSON API (requires `serde_json`).
   - Integration test against loopback interface and localhost DNS.

6. **SystemEventCollector** (estimated effort: medium)
   - Implement udev monitor source with device event normalization.
   - Implement D-Bus system bus signal source (property changes, device manager signals).
   - Implement netlink event source for link and address changes.
   - Implement source lifecycle (enable, disable, reconnect).
   - Unit test each source with mock event streams.

7. **ServiceManager** (estimated effort: medium)
   - Implement D-Bus connection to `org.freedesktop.systemd1`.
   - Implement start, stop, restart, enable, disable, status methods.
   - Implement unit state change signal subscription.
   - Implement retry logic with exponential backoff for D-Bus reconnection.
   - Integration test against systemd user instance.

8. **Integration and documentation** (estimated effort: small)
   - Write cross-crate integration tests in workspace `tests/`.
   - Write Criterion benchmarks for resource collection and event serialization.
   - Complete module-level documentation and rustdoc.
   - Update architecture blueprint and specification compliance matrix.

## Unresolved Topics

The following topics are explicitly out of scope for this RFC and will be addressed in separate RFCs:

- **Linux namespace sandboxing** (user, pid, net, mount namespaces for process isolation) -- deferred to RFC-0002.
- **cgroups v2 integration** for fine-grained resource enforcement -- deferred to RFC-0003.
- **seccomp-bpf and Landlock LSM profiles** for kernel-level process sandboxing -- deferred to RFC-0004.
- **eBPF-based observability** via `aya` or `libbpf` for deep kernel profiling -- deferred until after core platform is stable.
- **Remote system management API** (gRPC or REST) for external orchestration -- deferred to RFC-0005.
- **Predictive resource scaling** using historical metrics and ML models -- deferred to the Intelligence Integration phase.

## References

- [Architecture Overview](../architecture/overview.md) -- layered architecture and module dependency graph
- [System Platform Blueprint](../architecture/system.md) -- detailed design document for Phase 4
- [Core Platform Deep Dive](../architecture/core.md) -- EventBus, Service, LifecycleManager
- [Runtime Platform Deep Dive](../architecture/runtime.md) -- Scheduler, Supervisor, ResourceManager, PermissionChecker
- [Master Specification](../specification.md) -- compliance requirements for all modules (sections 5, 8, 11, 12)
- [Design Principles](../principles.md) -- clean architecture, event-driven design, security principles
- [Roadmap](../roadmap.md) -- phase timeline and dependencies
- [ADR-0001: Project Vision](../adr/0001-project-vision.md)
- [ADR-0002: Clean Architecture](../adr/0002-clean-architecture.md)
- [ADR-0003: Rust](../adr/0003-rust.md)
- [ADR-0004: Event-Driven](../adr/0004-event-driven.md)
- [ADR-0005: Runtime Platform](../adr/0005-runtime-platform.md)
- systemd D-Bus API: https://www.freedesktop.org/wiki/Software/systemd/dbus/
- inotify(7): https://man7.org/linux/man-pages/man7/inotify.7.html
- rtnetlink(7): https://man7.org/linux/man-pages/man7/rtnetlink.7.html
- udev(7): https://man7.org/linux/man-pages/man7/udev.7.html
