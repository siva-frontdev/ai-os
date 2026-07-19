# OSAL Module — Operating System Abstraction Layer

**Module:** `ai_os_system`
**Layer:** Layer 0 (lowest internal layer)
**Status:** In Progress (Phase 4)
**Crate:** Not yet created

## Purpose

The OSAL module is the lowest internal layer of the AI-native OS platform. It bridges all higher layers to the underlying Linux kernel and hardware through abstract trait interfaces. It provides managed access to system resources (CPU, memory, disk, network), process lifecycle, filesystem operations, device events, and systemd unit management. This module is the sole gateway through which platform-level system calls are issued. No code in any layer above OSAL may reference Linux-specific types or call libc directly.

## Status Overview

| Subsystem | Design | Implementation | Tests |
|---|---|---|---|
| SystemResourceManager | Draft | Not started | Not started |
| ProcessManager | Draft | Not started | Not started |
| FileSystemProvider | Draft | Not started | Not started |
| NetworkManager | Draft | Not started | Not started |
| SystemEventCollector | Draft | Not started | Not started |
| ServiceManager | Draft | Not started | Not started |

## Public Interfaces

### `SystemResourceManager`

Planned trait for polling host-level resource usage.

```rust
/// Planned for Phase 4
pub struct SystemResources {
    pub cpu_percent: f64,
    pub memory_used_kb: u64,
    pub memory_total_kb: u64,
    pub disk_used_kb: u64,
    pub disk_total_kb: u64,
    pub load_avg_1m: f64,
    pub load_avg_5m: f64,
    pub load_avg_15m: f64,
}

/// Planned for Phase 4
#[async_trait]
pub trait SystemResourceProvider: Send + Sync {
    async fn sample(&self) -> Result<SystemResources, SystemError>;
    fn poll_interval(&self) -> Duration;
}
```

Planned impl reads `/proc/stat`, `/proc/meminfo`, `/proc/loadavg`, and disk stats via `statvfs`. The external crate `sysinfo` is the leading candidate for cross-platform fallback.

### `ProcessManager`

Planned trait for spawning, signaling, and querying external processes.

```rust
/// Planned for Phase 4
pub struct ProcessHandle {
    pub pid: u32,
    pub exit_code: Option<i32>,
    pub state: ProcessState,
}

/// Planned for Phase 4
#[async_trait]
pub trait ProcessManager: Send + Sync {
    async fn spawn(&self, cmd: &str, args: &[&str]) -> Result<ProcessHandle, SystemError>;
    async fn signal(&self, pid: u32, signal: i32) -> Result<(), SystemError>;
    async fn wait(&self, pid: u32) -> Result<ProcessHandle, SystemError>;
    fn getpid(&self) -> u32;
}
```

Planned impl delegates to `libc` for `fork`/`exec`/`kill`/`waitpid`. A pure-Rust variant using `std::process::Command` with `tokio::process` is the initial prototype; `libc` bindings replace it for signal-level control.

### `FileSystemProvider`

Planned trait for managed filesystem access.

```rust
/// Planned for Phase 4
#[async_trait]
pub trait FileSystemProvider: Send + Sync {
    async fn read(&self, path: &Path) -> Result<Vec<u8>, SystemError>;
    async fn write(&self, path: &Path, data: &[u8]) -> Result<(), SystemError>;
    async fn stat(&self, path: &Path) -> Result<FileMetadata, SystemError>;
    async fn watch(&self, path: &Path, pattern: &str) -> Result<WatchHandle, SystemError>;
    async fn list(&self, path: &Path) -> Result<Vec<PathBuf>, SystemError>;
}
```

Planned impl uses `tokio::fs` for async I/O, `std::fs::metadata` for stat, and `inotify` (via `inotify-rs`) for recursive directory watching. Paths are canonicalized and checked against a sandbox root to prevent escape.

### `NetworkManager`

Planned trait for network interface and DNS operations.

```rust
/// Planned for Phase 4
pub struct InterfaceInfo {
    pub name: String,
    pub mac: [u8; 6],
    pub ips: Vec<IpAddr>,
    pub flags: u32,
}

/// Planned for Phase 4
#[async_trait]
pub trait NetworkManager: Send + Sync {
    async fn list_interfaces(&self) -> Result<Vec<InterfaceInfo>, SystemError>;
    async fn resolve_dns(&self, host: &str) -> Result<Vec<IpAddr>, SystemError>;
    async fn socket_stats(&self) -> Result<Vec<SocketInfo>, SystemError>;
}
```

Planned impl uses `rtnetlink` for interface enumeration and `trust-dns-resolver` for async DNS resolution.

### `SystemEventCollector`

Planned struct that bridges platform event sources to the Core EventBus.

```rust
/// Planned for Phase 4
pub struct SystemEventCollector {
    bus: Arc<EventBus>,
    udev_monitor: Option<UdevMonitor>,
    dbus_connection: Option<DbusConnection>,
}

impl SystemEventCollector {
    pub fn new(bus: Arc<EventBus>) -> Self;
    pub async fn start(&mut self) -> Result<(), SystemError>;
    pub async fn stop(&mut self) -> Result<(), SystemError>;
}
```

Internally spawns two background tasks:
- One polls `udev` for device add/remove/change events, maps them to `SystemEvent::DeviceEvent`.
- One listens on the system D-Bus for `org.freedesktop.systemd1` signals.

### `ServiceManager`

Planned trait for systemd unit lifecycle management.

```rust
/// Planned for Phase 4
pub struct UnitInfo {
    pub name: String,
    pub active_state: String,
    pub sub_state: String,
    pub description: String,
}

/// Planned for Phase 4
#[async_trait]
pub trait ServiceManager: Send + Sync {
    async fn start_unit(&self, name: &str) -> Result<(), SystemError>;
    async fn stop_unit(&self, name: &str) -> Result<(), SystemError>;
    async fn restart_unit(&self, name: &str) -> Result<(), SystemError>;
    async fn list_units(&self) -> Result<Vec<UnitInfo>, SystemError>;
    async fn unit_status(&self, name: &str) -> Result<UnitInfo, SystemError>;
    async fn enable_unit(&self, name: &str) -> Result<(), SystemError>;
    async fn disable_unit(&self, name: &str) -> Result<(), SystemError>;
}
```

Planned impl communicates over D-Bus using `zbus` (or `dbus-rs`) to interface with `org.freedesktop.systemd1.Manager`.

## Dependencies

| Dependency | Scope | Purpose |
|---|---|---|
| `ai_os_core` | internal | EventBus, Service trait, logging |
| `ai_os_runtime` | internal | PermissionChecker, ContextManager |
| `sysinfo` | external | Cross-platform system resource polling |
| `libc` | external | POSIX process and signal primitives |
| `inotify-rs` | external | Inotify-based filesystem watching |
| `rtnetlink` | external | Netlink-based network interface enumeration |
| `trust-dns-resolver` | external | Async DNS resolution |
| `zbus` | external | D-Bus client for systemd interaction |
| `tokio` | runtime | Async I/O, timers |

## Events Published

| Event | Trigger | Payload |
|---|---|---|
| `system::ResourceExhausted` | Resource usage exceeds `alert_threshold` | `(SystemResources, String)` -- resource and reason |
| `system::DeviceAdded` | udev device add | `(String, String)` -- devnode, subsystem |
| `system::DeviceRemoved` | udev device remove | `(String, String)` -- devnode, subsystem |
| `system::ProcessSpawned` | ProcessManager::spawn | `(u32, String)` -- pid, command |
| `system::ProcessExited` | ProcessManager::wait returns | `(u32, i32)` -- pid, exit code |
| `system::FileChanged` | inotify event | `(PathBuf, u32)` -- path, mask |
| `system::UnitStateChanged` | systemd unit state change | `(String, String, String)` -- unit, old_state, new_state |

## Events Consumed

| Event | Consumer | Action |
|---|---|---|
| `runtime::TaskStarted` | `ResourceManager` | Begin tracking CPU/memory for the task PID |
| `runtime::TaskCompleted` | `ResourceManager` | Stop tracking and log final usage |
| `runtime::TaskCancelled` | `ProcessManager` | Send SIGTERM to child process |

## Thread Model

All providers are `Send + Sync` and stored behind `Arc` in the container. `SystemResourceManager` spawns a dedicated Tokio task for periodic sampling; samples are written to an `Arc<RwLock<SystemResources>>` that consumers read without awaiting I/O.

- `ProcessManager` uses `tokio::process::Child` which is `Send` but not `Sync`; access is serialized through a `tokio::sync::Mutex`.
- `FileSystemProvider` operations are inherently async I/O; `tokio::fs` handles are `Send + Sync`.
- `SystemEventCollector` bridges synchronous udev/D-Bus event loops into async channels via `tokio::sync::mpsc`.

## Lifecycle

All subsystems follow the `Service` trait (see [Core](core.md)):

1. `init()` -- open D-Bus connections, bind inotify watches, open netlink sockets.
2. `start()` -- spawn background polling tasks, register udev listeners.
3. `stop()` -- signal tasks to drain, close file descriptors, drop connections.

Because these interfaces wrap OS resources, double-`init` must be prevented (guarded by a boolean or state check). The `Failed` state indicates a dangling OS resource that should be cleaned up via `stop()`.

## Error Handling

Error type planned as `ai_os_system::SystemError` with variants:

- `PermissionDenied(String)` -- operation blocked by OS permissions.
- `ResourceBusy(String)` -- device or file locked.
- `NotFound(String)` -- unit, interface, or file not found.
- `IoError(io::Error)` -- wrapped std I/O error.
- `DbusError(String)` -- D-Bus call failure.
- `ProtocolError(String)` -- netlink/DNS parse failure.

Recovery: Transient I/O errors are retried up to 3 times with exponential backoff. D-Bus connection drops trigger automatic reconnection with a `Degraded` health status reported to the health monitor.

## Configuration

```toml
[resources]
poll_interval_ms = 2000
alert_cpu_pct = 90.0
alert_memory_pct = 85.0

[filesystem]
sandbox_root = "/var/lib/ai-os"
max_read_bytes = 10_485_760  # 10 MiB per read

[events]
udev = true
dbus = true
```

Configuration file: `aios-system.toml`.

## Testing Strategy

- **Unit tests**: Mock-based tests for all traits using `mockall`. Verify `SystemResourceProvider::sample` returns plausible values on a real Linux host.
- **Integration tests**: Spawn a real child process via `ProcessManager`, signal it, verify exit code. Write and read files through `FileSystemProvider`, verify permissions are enforced.
- **Property-based tests**: Round-trip file read/write with random binary payloads. DNS resolution against `localhost`.
- **Contract tests**: For every system-level operation, verify that `PermissionDenied` is returned when the runtime permission checker denies the action.

## Future Extensions

- Container-level resource accounting via cgroup v2 (integrate with ResourceManager in Runtime).
- A virtual filesystem layer with interceptable read/write hooks (for sandboxing and auditing).
- Hardware acceleration discovery (GPU via NVML, TPU via `libtpu`).
- Power management interface (suspend/resume, CPU frequency scaling).
- A `/dev`-like virtual device tree exposed to user-mode agent code.
- Seccomp-bpf profile management for spawned processes.
