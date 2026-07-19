# System Platform Blueprint

## Purpose

The System Platform (Phase 4) bridges the Runtime Platform's task execution and session management with the underlying Linux host. It provides system-level resource management, process supervision, filesystem abstraction, network stack management, system event collection, and service lifecycle integration with systemd. This is the layer that makes AI-native OS an actual operating platform rather than a collection of application-level services.

The System Platform is the **current development phase**. Core and Runtime (Phases 2 and 3) are completed. The System Platform builds directly on the [Runtime Platform](runtime.md) and [Core Platform](core.md).

---

## Responsibilities

- **SystemResourceManager**: Monitor and manage system-wide resources (CPU, memory, disk, network I/O). Expose aggregate and per-process metrics. Enforce system-level resource policies.
- **ProcessManager**: Spawn, monitor, and terminate external processes. Track process state (running, stopped, zombie, exited). Collect exit codes and signals.
- **FileSystemProvider**: Abstract filesystem operations (read, write, list, watch) behind an event-driven interface. Support inode metadata, directory watching, and temporary files.
- **NetworkManager**: Manage network interfaces, sockets, firewall rules, and DNS configuration. Integrate with systemd-networkd and nftables.
- **SystemEventCollector**: Subscribe to kernel events (udev, inotify, netlink) and re-publish them as platform events on the EventBus. Provide a unified system event stream.
- **ServiceManager**: Integrate with systemd for system service lifecycle management. Start, stop, enable, disable, and query unit status.

---

## Public Interfaces

The System Platform will expose the following trait interfaces. These are provisional and will be finalized during implementation.

| Trait | Methods | Description |
|---|---|---|
| `SystemResourceManager` | `snapshot()`, `watch(threshold)`, `limits()`, `set_limits()` | Collect and monitor system-wide resource metrics (CPU, memory, disk, network I/O). Supports threshold-based alerting. |
| `ProcessManager` | `spawn(config)`, `kill(pid)`, `status(pid)`, `list()`, `watch(pid)` | Manage external process lifecycle. `spawn` returns a `ProcessHandle` with a `TaskId` for integration with the Runtime TaskManager. |
| `FileSystemProvider` | `read(path)`, `write(path, data)`, `list(path)`, `watch(path)`, `temp(prefix)` | Abstract async filesystem operations. `watch` returns a stream of file events. Paths are validated against allowed roots. |
| `NetworkManager` | `interfaces()`, `interface(name)`, `apply_firewall(rules)`, `dns()`, `set_dns(servers)` | Manage network interfaces, firewall rules, and DNS configuration. |
| `SystemEventCollector` | `subscribe(source)` | Bridge kernel and system events to the EventBus. `source` selects which event types to collect (udev, inotify, netlink, journal). |
| `ServiceManager` | `start(unit)`, `stop(unit)`, `restart(unit)`, `enable(unit)`, `disable(unit)`, `status(unit)`, `list(filter)` | Manage systemd unit lifecycle. All operations return the unit's current state. |

All traits will be `dyn`-compatible (`Debug + Send + Sync + 'static`) and stored behind `Arc<dyn Trait>` in the `SystemPlatform` composite struct.

---

## Planned Subsystems

### SystemResourceManager

The `SystemResourceManager` will query `/proc` and `/sys` filesystem hierarchies to collect:

- CPU usage per core, load averages, context switches
- Memory usage (total, available, cached, buffers, swap)
- Disk usage per mount point, I/O stats per device
- Network I/O per interface (bytes, packets, errors, drops)

Data will be collected on a configurable interval and published as events. The ResourceManager from the Runtime Platform will consume these events to enforce per-task limits.

**Design approach**: Pull-based collection via Tokio periodic tasks. Data cached and diffed to compute deltas. Absolute values and rates published separately.

### ProcessManager

The `ProcessManager` will manage external process lifecycle:

- Spawn processes with configurable environment, working directory, and resource limits
- Monitor process state via `waitpid`/`SIGCHLD` signal handling
- Track process trees (parent-child relationships)
- Enforce timeout and resource limits on child processes
- Collect stdout/stderr as structured log events

**Design approach**: Use `tokio::process::Command` for spawning and managing child processes. Use a dedicated Tokio task to handle `SIGCHLD` via a signal stream. Store process state in a `HashMap<Pid, ProcessInfo>` behind an `RwLock`.

### FileSystemProvider

The `FileSystemProvider` will provide an event-driven interface to the filesystem:

- Read, write, append operations with async I/O
- Directory listing with glob pattern support
- File/directory watching via `inotify`, re-published as events
- Temporary file creation with automatic cleanup
- Path validation and sandboxing (prevent escape from allowed roots)

**Design approach**: Wrap `tokio::fs` for async file operations. Use the `inotify` crate for directory watching with an async adapter. Path sandboxing via canonicalization and prefix checking.

### NetworkManager

The `NetworkManager` will manage network configuration:

- Query network interfaces and addresses via `libc`/`netlink`
- Manage socket permissions through systemd socket activation
- Firewall rule management via `nftables` JSON API
- DNS configuration via resolved
- Network namespace management for process isolation

**Design approach**: Use `rtnetlink` crate for netlink communication. Shell out to `nft` for firewall operations (with structured JSON I/O). Integrate with systemd-resolved via D-Bus for DNS.

### SystemEventCollector

The `SystemEventCollector` will bridge kernel and system events to the EventBus:

- Hardware events via `udev` (device add/remove, state change)
- Filesystem events via `inotify` (file create/modify/delete)
- Process events via netlink `connector` (fork/exec/exit)
- Systemd journal events via `sd-journal` API
- System health events (OOM, watchdog, thermal)

**Design approach**: Each event source runs as a dedicated Tokio task that reads from a file descriptor or socket and publishes typed events on the EventBus. Event types are namespaced under `system.*`.

### ServiceManager

The `ServiceManager` will integrate with systemd:

- Start, stop, restart, reload systemd units
- Enable, disable, mask units
- Query unit state (active, inactive, failed, activating, deactivating)
- Subscribe to unit state change signals via D-Bus
- Manage systemd service templates and instantiated units

**Design approach**: Use the `zbus` crate for D-Bus communication with systemd's `org.freedesktop.systemd1` service. Wrap D-Bus method calls in async functions. Subscribe to `PropertiesChanged` signals for unit state tracking.

---

## Dependencies

| Crate | Purpose | Subsystem |
|---|---|---|
| `ai-os-core` | EventBus, Service, Logger, LifecycleManager | All subsystems |
| `ai-os-runtime` | Scheduler, TaskManager, ResourceManager, PermissionChecker | All subsystems |
| `tokio` | Async runtime, process management, fs, signal handling | All subsystems |
| `zbus` | D-Bus client for systemd and resolved integration | ServiceManager, NetworkManager |
| `inotify` | Linux inotify wrapper for file/directory watching | FileSystemProvider, SystemEventCollector |
| `rtnetlink` | Netlink protocol for network interface management | NetworkManager |
| `nftables` (JSON API) | Firewall rule management via nft | NetworkManager |
| `libc` | Direct system calls for process and resource management | ProcessManager, SystemResourceManager |
| `sd-journal` (systemd) | Journal event reading | SystemEventCollector |
| `serde` / `serde_json` | Event serialization, nftables JSON I/O | All subsystems |
| `procfs` | `/proc` filesystem parsing | SystemResourceManager |
| `thiserror` | Error type derivation | All subsystems |
| `chrono` | Timestamps for system events | SystemEventCollector |

---

## Events Published

| Event | Type String | Subsystem | Trigger |
|---|---|---|---|
| `SystemResourceSnapshot` | `system.resource.snapshot` | SystemResourceManager | Periodic collection interval |
| `SystemResourceWarning` | `system.resource.warning` | SystemResourceManager | Resource threshold crossed |
| `SystemResourceCritical` | `system.resource.critical` | SystemResourceManager | Resource hard limit reached |
| `ProcessSpawned` | `system.process.spawned` | ProcessManager | New process created |
| `ProcessExited` | `system.process.exited` | ProcessManager | Process terminates (exit code/signal) |
| `ProcessStateChanged` | `system.process.state_changed` | ProcessManager | State transition (running, stopped, zombie) |
| `FileModified` | `system.file.modified` | FileSystemProvider | Watched file content changes |
| `FileCreated` | `system.file.created` | FileSystemProvider | New file in watched directory |
| `FileDeleted` | `system.file.deleted` | FileSystemProvider | File removed from watched directory |
| `NetworkInterfaceChanged` | `system.network.interface_changed` | NetworkManager | Interface up/down, address change |
| `FirewallRuleApplied` | `system.network.firewall_rule_applied` | NetworkManager | Firewall rule added/modified/removed |
| `DeviceAdded` | `system.device.added` | SystemEventCollector | udev device added |
| `DeviceRemoved` | `system.device.removed` | SystemEventCollector | udev device removed |
| `SystemdUnitStateChanged` | `system.service.unit_state_changed` | ServiceManager | systemd unit state transition |
| `SystemdUnitFailed` | `system.service.unit_failed` | ServiceManager | systemd unit enters failed state |

---

## Events Consumed

| Event | Source | Consumer | Purpose |
|---|---|---|---|
| `runtime.task_started` | Runtime TaskManager | ProcessManager | Set resource limits on child process |
| `runtime.task_completed` | Runtime TaskManager | ProcessManager | Clean up process tracking |
| `runtime.task_failed` | Runtime TaskManager | ProcessManager | Kill associated process tree |
| `runtime.task_cancelled` | Runtime TaskManager | ProcessManager | Kill associated process tree |
| `runtime.session_destroyed` | Runtime SessionManager | ProcessManager | Clean up session-scoped processes |
| `runtime.session_destroyed` | Runtime SessionManager | FileSystemProvider | Clean up session-scoped temp files |
| `runtime.session_destroyed` | Runtime SessionManager | NetworkManager | Release session-scoped network resources |
| `system.process.exited` | ProcessManager (self) | SystemResourceManager | Update process tracking |
| `system.device.added` | SystemEventCollector | NetworkManager | Handle new network device |
| `system.device.removed` | SystemEventCollector | NetworkManager | Handle device removal |

---

## Thread Model

| Subsystem | Threading |
|---|---|
| `SystemResourceManager` | Tokio periodic task for collection. `RwLock<HashMap>` for cached metrics. |
| `ProcessManager` | Tokio task per child process (async wait). Main `HashMap<Pid, ProcessInfo>` behind `RwLock`. Signal handler via `tokio::signal`. |
| `FileSystemProvider` | Tokio tasks per inotify watch. `RwLock` for watched path registry. `tokio::fs` for async file I/O. |
| `NetworkManager` | Tokio tasks for netlink event streams. `RwLock` for interface state cache. D-Bus calls via `zbus` are async. |
| `SystemEventCollector` | One Tokio task per event source (udev, inotify, netlink, journal). Each reads from a file descriptor and publishes events. |
| `ServiceManager` | Async D-Bus connection via `zbus`. Signal handler for unit state changes. |

All subsystems run on the same multi-threaded Tokio runtime as Core and Runtime. No dedicated thread pools are needed.

---

## Lifecycle

Each System Platform subsystem implements Core's `Service` trait and registers with the `LifecycleManager`:

1. **Construction**: Subsystem instances are created with references to the EventBus, Logger, and Runtime subsystems they depend on.
2. **Start**: `start()` registers event subscriptions, spawns background Tokio tasks, and initializes system handles (e.g., D-Bus connections, inotify watches).
3. **Running**: Subsystems process events, collect metrics, and manage system resources.
4. **Stop**: `stop()` cancels background tasks, closes system handles, unregisters event subscriptions, and releases resources.

Subsystem start order within System Platform:

```
1. SystemEventCollector   (event sources need to be ready first)
2. FileSystemProvider     (needed by most other subsystems)
3. SystemResourceManager  (starts collecting immediately)
4. NetworkManager         (network needed by services)
5. ProcessManager         (can spawn processes once infrastructure is ready)
6. ServiceManager         (last, manages systemd units)
```

---

## Error Handling

Each subsystem defines its own error variants within a unified `SystemError` enum:

| Variant | Subsystem | Condition |
|---|---|---|
| `ResourceCollectionFailed(String)` | SystemResourceManager | `/proc` or `/sys` read failure |
| `ProcessSpawnFailed(String)` | ProcessManager | `fork`/`exec` failure |
| `ProcessWaitFailed(String)` | ProcessManager | `waitpid` error |
| `FileSystemPermissionDenied(String)` | FileSystemProvider | Path outside allowed root |
| `FileSystemIoError(String)` | FileSystemProvider | Underlying I/O error (wrapped from `tokio::fs`) |
| `InotifyWatchLimit(String)` | FileSystemProvider | `max_user_watches` exhaustion |
| `NetworkInterfaceNotFound(String)` | NetworkManager | Interface does not exist |
| `FirewallRuleInvalid(String)` | NetworkManager | Malformed nftables rule |
| `DbusConnectionFailed(String)` | ServiceManager, NetworkManager | D-Bus connection refused or broken |
| `SystemdUnitActionFailed(String)` | ServiceManager | systemd method call returns error |
| `JournalReadFailed(String)` | SystemEventCollector | `sd-journal` API error |
| `EventSourceDisconnected(String)` | SystemEventCollector | udev/netlink socket closed |
| `Core(CoreError)` | All subsystems | Error from Core or Runtime |
| `General(String)` | All subsystems | Unclassified error |

**Recovery strategies**:
- Transient errors (D-Bus reconnect, inotify watch re-registration) are retried with exponential backoff.
- Resource exhaustion errors (inotify watch limit, file descriptor limit) are published as critical events for operator intervention.
- Permanent configuration errors (invalid firewall rule, non-existent interface) return immediately without retry.

---

## Design Approach

The System Platform follows several consistent design patterns:

### Abstraction over Raw System Calls

All system interactions go through trait interfaces. The `FileSystemProvider` trait abstracts filesystem operations; implementations could wrap `tokio::fs`, use `io_uring` (via `tokio-uring`), or even delegate to a remote filesystem over gRPC. This follows the Clean Architecture principle: outer layers implement, inner layers define.

### Event-First Design

Every system state change produces an event. When the `SystemResourceManager` detects memory pressure, it publishes `SystemResourceWarning`. When the `ProcessManager` sees a child exit, it publishes `ProcessExited`. Upper layers (Memory, Brain) consume these events to make decisions. No system state is hidden behind synchronous polling APIs.

### Graceful Degradation

Each subsystem is designed to operate independently. If D-Bus is unavailable, the `ServiceManager` degrades to polling `systemctl` commands. If `inotify` watch limits are hit, the `FileSystemProvider` falls back to polling. The platform continues running with reduced functionality rather than failing entirely.

### Integration with Linux Primitives

Rather than reinventing process management or service supervision, the System Platform wraps existing Linux primitives:

| Platform Primitive | Linux Equivalent | Wrapper |
|---|---|---|
| Process lifecycle | `fork`/`exec`, `waitpid`, `SIGCHLD` | `tokio::process::Command` + signal handler |
| Service lifecycle | systemd units | `zbus` D-Bus client |
| File watching | inotify | `inotify` crate + async adapter |
| Network config | netlink, nftables | `rtnetlink` crate, nft JSON API |
| Resource metrics | `/proc`, `/sys` | `procfs` crate |
| System events | udev, netlink, journal | `udev` crate, `sd-journal` API |

---

## Current Status

| Subsystem | Status | Notes |
|---|---|---|
| `SystemResourceManager` | In design | Prototyping /proc parsing. Data model defined. |
| `ProcessManager` | In design | Process state model defined. Spawn API under discussion. |
| `FileSystemProvider` | Not started | Scheduled after ProcessManager. |
| `NetworkManager` | Not started | NetworkManager integration evaluated (NetworkManager vs. systemd-networkd). |
| `SystemEventCollector` | In design | udev monitor prototype working. inotify adapter designed. |
| `ServiceManager` | Not started | Depends on D-Bus integration pattern from other subsystems. |

The repository currently has a `system/` crate directory with only a `.gitkeep` file. Implementation will begin after this architecture document is finalized.

---

## Integration with Core and Runtime

The System Platform integrates with lower layers as follows:

```
                          EventBus (Core)
                              |
        +----------+----------+----------+----------+
        |          |          |          |          |
   SystemRes  Process  FileSystem  Network  Service  EventCollector
    Mgr        Mgr      Provider    Mgr      Mgr
        |          |          |          |          |
        +----------+----------+----------+----------+
                              |
                    Runtime Subsystems
                    (Scheduler, TaskManager,
                     ResourceManager, PermissionChecker)
                              |
                    Core Subsystems
                    (Logger, HealthMonitor,
                     LifecycleManager)
```

### Core Integration Points

- **EventBus**: All subsystems publish and consume events through the Core EventBus.
- **Service**: All subsystems implement the Core `Service` trait for lifecycle management.
- **Logger**: All subsystems use the Core Logger for structured output.
- **HealthMonitor**: Each subsystem registers a health check. For example, `ServiceManager` checks D-Bus connectivity; `NetworkManager` checks interface presence.
- **Container**: System platform subsystems are registered with the Core Container for DI.

### Runtime Integration Points

- **TaskManager**: `ProcessManager` uses `TaskManager` to create tracking tasks for spawned processes.
- **ResourceManager**: `SystemResourceManager` feeds aggregate resource data to `ResourceManager` for per-task enforcement.
- **PermissionChecker**: System operations (e.g., spawning a process, modifying a firewall rule) are gated by `PermissionChecker` using the caller's `PermissionContext`.
- **Scheduler**: System-level tasks (e.g., periodic resource collection) are scheduled via the Runtime `Scheduler`.
- **ContextManager**: All system events carry trace context for distributed tracing across the platform.

---

## Design Decisions

1. **Pull-based Resource Collection** -- System resources are collected on a timer rather than pushed from the kernel. This provides a consistent polling interval and avoids the complexity of kernel event subscriptions for resource metrics. Critical thresholds (OOM, disk full) are handled by `SystemEventCollector`.

2. **Process as Task Mapping** -- Every spawned OS process is tracked as a Runtime `Task`. This enables uniform supervision, resource accounting, and cancellation. When a Task is cancelled, the associated process tree is killed. This decision is documented in [ADR-0010](../adr/0010-process-task-mapping.md) (planned).

3. **inotify over fanotify** -- inotify is preferred over fanotify for file watching because it requires no elevated capabilities and is available in containerized environments. fanotify may be added later for system-wide file access monitoring.

4. **systemd Integration via D-Bus** -- Using D-Bus to communicate with systemd (rather than shelling out to `systemctl`) provides typed interfaces, signal-based state change notifications, and better performance. The `zbus` crate provides a safe, async Rust D-Bus client.

5. **Event Bridge Pattern** -- Kernel events (udev, inotify, netlink) are bridged to the EventBus by dedicated collector tasks. Each collector reads from a file descriptor via Tokio's async I/O support and publishes typed events. This pattern keeps the event bridge simple and testable.

6. **Sandboxed File Access** -- The `FileSystemProvider` enforces path sandboxing by canonicalizing all paths and verifying they fall within configured allowed roots. This prevents path traversal attacks and ensures that modules can only access files they are authorized to see.

7. **Session-Scoped Resource Cleanup** -- Resources tied to sessions (processes, temp files, network connections) are cleaned up automatically when the session is destroyed. The `SessionManager`'s `session_destroyed` event triggers cleanup across all System Platform subsystems.

---

## Future Extensions

- **Container Runtime Integration**: Add direct integration with Docker/Podman via their REST APIs, building on the Core Platform Container module. This would enable the System Platform to manage containerized workloads alongside native processes.
- **cgroups v2 Management**: Add direct cgroups v2 manipulation for fine-grained resource control of process groups, replacing the current limit-enforcement approach with kernel-enforced constraints.
- **Seccomp and Landlock Profiles**: Add support for seccomp-bpf and Landlock LSM policies to sandbox processes and filesystem access at the kernel level.
- **Auditd Integration**: Forward system audit events (via `auditd` / `netlink_audit`) to the EventBus for security monitoring and compliance.
- **Power Management**: Add ACPI event handling, sleep/wake lifecycle management, and thermal monitoring.
- **Namespace Management**: Add Linux namespace (PID, network, mount, user) creation and management for process isolation, enabling lightweight container-like sandboxing without a full container runtime.
- **BPF Observability**: Integrate eBPF programs for deep kernel observability (function tracing, latency profiling, I/O scheduling) using `libbpf` or `aya` crate.
- **Remote System Management**: Expose System Platform operations over a secure gRPC API, enabling remote management and integration with external orchestration systems.
- **Predictive Resource Scaling**: Use historical resource data (collected by `SystemResourceManager`) to predict future resource demands and proactively scale allocations. This would feed into the Intelligence Integration phase.

---

## References

- [Architecture Overview](overview.md)
- [Core Platform Deep Dive](core.md)
- [Runtime Platform Deep Dive](runtime.md)
- [Design Principles](../principles.md)
- [Roadmap](../roadmap.md)
- [Glossary](../glossary.md)
- systemd D-Bus API: https://www.freedesktop.org/wiki/Software/systemd/dbus/
- inotify(7) man page
- rtnetlink(7) man page
- udev(7) man page
