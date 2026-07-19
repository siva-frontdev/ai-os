# Operating System Abstraction Layer (OSAL)

## Purpose

The Operating System Abstraction Layer (OSAL) is the foundational layer of the AI-native OS platform — **Layer 0**. Its single responsibility is to insulate every module above it from the specifics of the host operating system.

OSAL provides abstract interfaces for system resources, processes, filesystem, networking, system events, and service management. All Linux-specific logic (syscalls, procfs, dbus, inotify, rtnetlink, systemd libc) lives behind these trait interfaces and is confined to OSAL implementations.

The horizontal line below OSAL is the **platform boundary**. Everything above is OS-independent.

```
Brain, Memory, Perception, Execution, Intelligence
        │
        ▼
Runtime Platform
        │
        ▼
Core Platform
        │
        ▼
Operating System Abstraction Layer (OSAL)      ◄── Everything in here is Linux-specific
        │
        ▼
Linux Kernel / Hardware
```

OSAL is the **current development phase** (Phase 4). Core and Runtime (Phases 2 and 3) are completed. Unlike earlier documentation that placed OSAL above Core, the correct layering places OSAL **below** Core. Core and Runtime depend on OSAL abstractions; OSAL depends on nothing except the Linux kernel and standard Rust crates.

---

## Responsibilities

- **SystemResource**: Monitor and manage system-wide resources (CPU, memory, disk, network I/O). Expose aggregate and per-process metrics. Enforce system-level resource policies.
- **Process**: Spawn, monitor, and terminate external processes. Track process state (running, stopped, zombie, exited). Collect exit codes and signals.
- **FileSystem**: Abstract filesystem operations (read, write, list, watch) behind an event-driven interface. Support inode metadata, directory watching, and temporary files. Enforce path sandboxing.
- **Network**: Manage network interfaces, sockets, firewall rules, and DNS configuration. Integrate with systemd-networkd and nftables.
- **EventCollector**: Subscribe to kernel events (udev, inotify, netlink) and re-publish them as platform events on the EventBus. Provide a unified system event stream.
- **ServiceManager**: Integrate with systemd for system service lifecycle management. Start, stop, enable, disable, and query unit status.

---

## Public Interfaces

OSAL traits define the contract between the platform and the host OS. Because OSAL is the bottom layer, these traits must be general enough to support non-Linux hosts in the future. No Linux-specific types appear in these trait signatures.

```rust
/// System-wide resource monitoring.
pub trait SystemResource: Debug + Send + Sync {
    fn snapshot(&self) -> Result<ResourceSnapshot, OsalError>;
    fn watch(&self, thresholds: &ResourceThresholds) -> Receiver<ResourceWarning>;
    fn limits(&self) -> ResourceLimits;
    fn set_limits(&self, limits: &ResourceLimits) -> Result<(), OsalError>;
}

/// External process lifecycle management.
pub trait ProcessManager: Debug + Send + Sync {
    fn spawn(&self, config: &ProcessConfig) -> Result<ProcessHandle, OsalError>;
    fn kill(&self, pid: Pid) -> Result<(), OsalError>;
    fn status(&self, pid: Pid) -> Option<ProcessStatus>;
    fn list(&self) -> Vec<ProcessInfo>;
    fn watch(&self, pid: Pid) -> Receiver<ProcessEvent>;
}

/// Async filesystem operations with path sandboxing.
pub trait FileSystem: Debug + Send + Sync {
    async fn read(&self, path: &Path) -> Result<Vec<u8>, OsalError>;
    async fn write(&self, path: &Path, data: &[u8]) -> Result<(), OsalError>;
    async fn list(&self, path: &Path) -> Result<Vec<DirEntry>, OsalError>;
    async fn watch(&self, path: &Path) -> Result<Receiver<FileEvent>, OsalError>;
    async fn temp(&self, prefix: &str) -> Result<TempFile, OsalError>;
}

/// Network interface and configuration management.
pub trait NetworkManager: Debug + Send + Sync {
    fn interfaces(&self) -> Result<Vec<NetworkInterface>, OsalError>;
    fn interface(&self, name: &str) -> Result<NetworkInterface, OsalError>;
    fn apply_firewall(&self, rules: &[FirewallRule]) -> Result<(), OsalError>;
    fn dns(&self) -> Result<DnsConfig, OsalError>;
    fn set_dns(&self, config: &DnsConfig) -> Result<(), OsalError>;
}

/// Kernel and system event bridge to EventBus.
pub trait EventCollector: Debug + Send + Sync {
    fn subscribe(&self, source: EventSource) -> Receiver<SystemEvent>;
}

/// System service lifecycle (abstraction over systemd/init.d).
pub trait ServiceManager: Debug + Send + Sync {
    fn start(&self, unit: &str) -> Result<ServiceState, OsalError>;
    fn stop(&self, unit: &str) -> Result<ServiceState, OsalError>;
    fn restart(&self, unit: &str) -> Result<ServiceState, OsalError>;
    fn enable(&self, unit: &str) -> Result<(), OsalError>;
    fn disable(&self, unit: &str) -> Result<(), OsalError>;
    fn status(&self, unit: &str) -> Result<ServiceState, OsalError>;
    fn list(&self, filter: &ServiceFilter) -> Result<Vec<ServiceInfo>, OsalError>;
}
```

All traits are `dyn`-compatible (`Debug + Send + Sync + 'static`) and stored behind `Arc<dyn Trait>`.

---

## Planned Subsystems

### SystemResource

Queries `/proc` and `/sys` filesystem hierarchies to collect:
- CPU usage per core, load averages, context switches
- Memory usage (total, available, cached, buffers, swap)
- Disk usage per mount point, I/O stats per device
- Network I/O per interface (bytes, packets, errors, drops)

Data collected on a configurable interval and cached in-memory. Deltas computed for rate metrics. The ResourceManager (Runtime) consumes this data to enforce per-task limits.

**Implementation**: Pull-based collection via Tokio periodic tasks. Uses the `procfs` crate for parsing.

### Process

Manages external process lifecycle:
- Spawn with configurable environment, working directory, resource limits
- Monitor state via `waitpid`/`SIGCHLD`
- Track process trees (parent-child relationships)
- Enforce timeout and resource limits
- Collect stdout/stderr as structured events

**Implementation**: `tokio::process::Command` for spawning. Dedicated Tokio task for `SIGCHLD` handling. State stored in `HashMap<Pid, ProcessInfo>` behind `RwLock`.

### FileSystem

Event-driven filesystem interface:
- Async read/write/append via `tokio::fs`
- Directory listing with glob pattern support
- File/directory watching via `inotify`
- Temporary file creation with automatic cleanup
- Path canonicalization and sandboxing (prevent escape from allowed roots)

**Implementation**: `tokio::fs` for async I/O. `inotify` crate with async adapter for watching. Canonicalization via `std::fs::canonicalize` with prefix check.

### Network

Network configuration management:
- Query interfaces and addresses via `rtnetlink`
- Socket permissions via systemd socket activation
- Firewall rules via `nftables` JSON API
- DNS via systemd-resolved D-Bus
- Network namespaces for process isolation

**Implementation**: `rtnetlink` crate for netlink. Shell out to `nft` with structured JSON I/O. `zbus` for D-Bus with systemd-resolved.

### EventCollector

Bridges kernel and system events to the EventBus:
- Hardware events via `udev` (device add/remove, state change)
- Filesystem events via `inotify`
- Process events via netlink `connector`
- Systemd journal via `sd-journal` API
- System health events (OOM, watchdog, thermal)

**Implementation**: Each source runs as a dedicated Tokio task reading from a file descriptor or socket. Events namespaced under `system.*`.

### ServiceManager

Integrates with init system (systemd on Linux):
- Start, stop, restart, reload units
- Enable, disable, mask units
- Query unit state
- Subscribe to state change signals via D-Bus

**Implementation**: `zbus` crate for D-Bus with systemd's `org.freedesktop.systemd1` service.

---

## Platform Manifest Integration

OSAL subsystems are configured through `platform.toml` at the workspace root:

```toml
[system.subsystems]
system_resource = true
process = true
filesystem = true
network = true
event_collector = false
service_manager = false
```

The boot process reads `platform.toml` before any other initialization (see [Specification §5.1](../specification.md#51-boot-sequence)). Only enabled subsystems are loaded. Disabled subsystems are skipped during initialization and start phases. This allows platform deployments to omit OSAL features that are not needed for a given use case.

---

## Dependencies

OSAL sits at Layer 0. It depends on **no internal crates** — no Core, no Runtime. All dependencies are external:

| Crate | Purpose | Subsystem |
|---|---|---|
| `tokio` | Async runtime, process management, fs, signal handling | All subsystems |
| `zbus` | D-Bus client for systemd and resolved integration | ServiceManager, Network |
| `inotify` | Linux inotify wrapper | FileSystem, EventCollector |
| `rtnetlink` | Netlink protocol for network interface management | Network |
| `libc` | Direct system calls (fork, exec, waitpid, getpid, kill) | Process |
| `procfs` | `/proc` filesystem parsing | SystemResource |
| `serde` / `serde_json` | Event serialization, nft JSON I/O | All subsystems |
| `thiserror` | Error type derivation | All subsystems |
| `chrono` | Timestamps | EventCollector |

Higher layers (Core, Runtime) depend on OSAL for OS abstraction, not the reverse.

---

## Events Published

| Event | Type String | Subsystem | Trigger |
|---|---|---|---|
| `SystemResourceSnapshot` | `system.resource.snapshot` | SystemResource | Periodic collection interval |
| `SystemResourceWarning` | `system.resource.warning` | SystemResource | Threshold crossed |
| `SystemResourceCritical` | `system.resource.critical` | SystemResource | Hard limit reached |
| `ProcessSpawned` | `system.process.spawned` | Process | Process created |
| `ProcessExited` | `system.process.exited` | Process | Process terminates |
| `ProcessStateChanged` | `system.process.state_changed` | Process | State transition |
| `FileModified` | `system.file.modified` | FileSystem | Watched file changes |
| `FileCreated` | `system.file.created` | FileSystem | New file in watched dir |
| `FileDeleted` | `system.file.deleted` | FileSystem | File removed |
| `NetworkInterfaceChanged` | `system.network.interface_changed` | Network | Interface up/down |
| `FirewallRuleApplied` | `system.network.firewall_rule_applied` | Network | Rule changed |
| `DeviceAdded` | `system.device.added` | EventCollector | udev device added |
| `DeviceRemoved` | `system.device.removed` | EventCollector | udev device removed |
| `SystemdUnitStateChanged` | `system.service.unit_state_changed` | ServiceManager | Unit state transition |
| `SystemdUnitFailed` | `system.service.unit_failed` | ServiceManager | Unit enters failed state |

---

## Events Consumed

OSAL subsystems consume no internal events at the OSAL layer. Event consumption happens at the Core and Runtime layers, which subscribe to OSAL events to react to system state changes.

| Event | Consumer | Purpose |
|---|---|---|
| `system.resource.*` | Runtime ResourceManager | Enforce per-task resource limits |
| `system.process.*` | Runtime TaskManager | Track process-to-task mapping |
| `system.file.*` | Core Logger, Runtime SessionManager | File change detection, session cleanup |
| `system.network.*` | Runtime PermissionChecker | Network capability enforcement |
| `system.device.*` | Runtime (future) | Device-aware scheduling |
| `system.service.*` | Runtime Supervisor | Service health monitoring |

---

## Thread Model

| Subsystem | Threading |
|---|---|
| `SystemResource` | Tokio periodic task for collection. `RwLock<HashMap>` for cached metrics. |
| `Process` | Tokio task per child process. `HashMap<Pid, ProcessInfo>` behind `RwLock`. Signal via `tokio::signal`. |
| `FileSystem` | Tokio tasks per inotify watch. `RwLock` for watch registry. `tokio::fs` for async I/O. |
| `Network` | Tokio tasks for netlink event streams. `RwLock` for interface cache. Async D-Bus via `zbus`. |
| `EventCollector` | One Tokio task per event source. Each reads a fd and publishes events. |
| `ServiceManager` | Async D-Bus via `zbus`. Signal handler for unit state changes. |

All subsystems run on the same multi-threaded Tokio runtime. No dedicated thread pools.

---

## Lifecycle

OSAL subsystems implement the Core `Service` trait (imported from Core, which imports the trait definition from...). Since OSAL is Layer 0, the `Service` trait must be defined in a location accessible to both OSAL and Core. The trait is defined in a shared `ai-os-core` crate. OSAL imports the trait; Core depends on OSAL.

**Start order** (OSAL layer only):
```
1. EventCollector    (system event sources must be ready first)
2. FileSystem        (needed by most other subsystems and Core)
3. SystemResource    (starts collecting immediately)
4. Network           (network needed by services)
5. Process           (can spawn processes once infrastructure is ready)
6. ServiceManager    (last, manages systemd units)
```

**Stop** reverses the order. Each subsystem cancels background tasks, closes handles, and unregisters subscriptions.

---

## Error Handling

Unified `OsalError` enum via `thiserror`:

| Variant | Subsystem | Condition |
|---|---|---|
| `ResourceCollectionFailed(String)` | SystemResource | `/proc` or `/sys` read failure |
| `ProcessSpawnFailed(String)` | Process | fork/exec failure |
| `ProcessWaitFailed(String)` | Process | waitpid error |
| `ProcessTimeout(Pid)` | Process | Process exceeded timeout |
| `PermissionDenied(String)` | FileSystem | Path outside allowed root |
| `IoError(String)` | FileSystem | Underlying I/O error |
| `InotifyWatchLimit(String)` | FileSystem | max_user_watches exhaustion |
| `InterfaceNotFound(String)` | Network | Interface does not exist |
| `FirewallRuleInvalid(String)` | Network | Malformed nftables rule |
| `DbusConnectionFailed(String)` | ServiceManager, Network | D-Bus refused or broken |
| `ServiceActionFailed(String)` | ServiceManager | systemd method call error |
| `JournalReadFailed(String)` | EventCollector | sd-journal API error |
| `EventSourceDisconnected(String)` | EventCollector | udev/netlink socket closed |

**Recovery**: Transient errors (D-Bus reconnect, inotify re-registration) retry with exponential backoff. Resource exhaustion events published for operator intervention. Configuration errors fail immediately.

---

## Design Principles

### Abstraction over Raw System Calls

All system interactions go through trait interfaces. `FileSystem` abstracts filesystem operations; a Linux implementation wraps `tokio::fs` and `inotify`, but a future FreeBSD implementation could use `kqueue`. No code above OSAL references Linux-specific types.

### Event-First Design

Every system state change produces an event. Upper layers (Memory, Brain) consume these events to make decisions. No system state is hidden behind synchronous polling APIs.

### Graceful Degradation

Each subsystem operates independently. If D-Bus is unavailable, `ServiceManager` degrades to polling `systemctl`. If `inotify` limits are hit, `FileSystem` falls back to polling. The platform continues with reduced functionality.

### Linux Wrapping, Not Replacing

Rather than reinventing process supervision, OSAL wraps Linux primitives:

| OSAL Primitive | Linux Equivalent | Rust Interface |
|---|---|---|
| Process lifecycle | fork/exec, waitpid, SIGCHLD | `tokio::process::Command` |
| Service lifecycle | systemd units | `zbus` D-Bus client |
| File watching | inotify | `inotify` crate + async adapter |
| Network config | netlink, nftables | `rtnetlink`, nft JSON |
| Resource metrics | /proc, /sys | `procfs` crate |
| System events | udev, netlink, journal | `udev` crate, sd-journal |

---

## Future Extensions

- **Container Runtime**: Docker/Podman REST API integration.
- **cgroups v2**: Kernel-enforced resource constraints.
- **Seccomp + Landlock**: Kernel-level sandbox policies.
- **Auditd**: Forward system audit events to EventBus.
- **Power Management**: ACPI events, sleep/wake lifecycle.
- **Namespaces**: PID, network, mount, user namespace creation.
- **eBPF**: Deep kernel observability via `aya` or `libbpf`.
- **Predictive Scaling**: Historical resource data for proactive allocation.

---

## References

- [Architecture Overview](overview.md) — Shows OSAL in the layered stack
- [Core Platform Deep Dive](core.md) — Depends on OSAL abstractions
- [Runtime Platform Deep Dive](runtime.md) — Depends on OSAL for system operations
- [Platform Manifest](../../platform.toml) — OSAL subsystem configuration
- [Specification §5](../specification.md#51-boot-sequence) — Boot protocol referencing platform.toml
- [RFC-0001 System Platform](../rfc/RFC-0001-system-platform.md) — Original RFC (uses legacy naming)
- [Design Principles](../principles.md)
- [Roadmap](../roadmap.md)
