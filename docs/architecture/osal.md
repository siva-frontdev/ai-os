# OSAL — Operating System Abstraction Layer

## Purpose

OSAL is Layer 1 of the ai-os architecture — the lowest internal layer. It depends on nothing inside the platform and insulates every higher layer from operating-system-specific details. The platform boundary is absolute: everything above OSAL is OS-independent. OSAL translates platform intent into OS operations and surfaces OS events into the platform event bus.

OSAL exists to make ai-os portable across Unix-like kernels without modifying a single line of code in Core, Runtime, Memory, Brain, Perception, Execution, or Intelligence. It is the only layer that links against libc, uses unsafe code, or calls Linux kernel interfaces directly.

## Architectural Role

OSAL sits directly between the host operating system (Linux) and the Core layer.

### Layer Diagram

```
┌──────────────────────────────────────────────────┐
│                  Intelligence                     │
│                  Execution                        │
│                  Brain                            │
│                  Perception                       │
│                  Memory                           │
│                  Runtime                          │
│                  Core                             │
├──────────────────────────────────────────────────┤
│                   OSAL                            │
│         (KernelFacade + subsystem traits)         │
├──────────────────────────────────────────────────┤
│                Linux Kernel                       │
│           (syscalls, /proc, /sys, udev, libc)     │
└──────────────────────────────────────────────────┘
```

### KernelFacade Pattern

OSAL exposes a single entry point — `KernelFacade` — holding references to every subsystem implementation. Higher layers receive a `&KernelFacade` or `Arc<KernelFacade>` injected during construction.

```
Core Service
    │
    ▼
KernelFacade  ←── Arc<dyn FileSystem>
    │              Arc<dyn ProcessManager>
    │              Arc<dyn Terminal>
    │              Arc<dyn NetworkManager>
    │              Arc<dyn SystemMonitor>
    │              Arc<dyn DeviceManager>
    │              Arc<dyn UserManager>
    │              Arc<dyn PlatformInfo>
    │
    ├──► osal-linux (LinuxKernelFacade)
    └──► test-mocks (MockKernelFacade)
```

### Dependency Inversion

Core, Runtime, and higher layers depend on OSAL traits — never on OSAL implementations. `osal-linux` is a leaf in the dependency graph; nothing imports it except boot-time wiring.

```rust
use osal_core::filesystem::FileSystem;
use osal_linux::LinuxKernelFacade;

let facade = LinuxKernelFacade::new().await;
let core = CoreService::new(facade);
```

## Design Principles

### 1. Interface-first
Every OS capability is a trait before it is an implementation. Traits live in `osal-core`; implementations live in `osal-linux`. Higher-layer code can be developed, compiled, and tested on any host OS with mock implementations.

### 2. Capability-gated
Every public OSAL method accepts `&CapabilityContext`. The first operation checks whether the caller's capability set permits the action. If not, returns `Err(OsalError::CapabilityDenied(...))` without touching any OS resource.

### 3. Kernel Facade pattern
A single `KernelFacade` struct aggregates references to all subsystems. Constructed once during boot, shared via `Arc<KernelFacade>`.

### 4. Event-first
Every meaningful OS state change produces a typed event flowing from OSAL watchers into the platform event bus.

### 5. Zero Linux outside osal-linux
`osal-linux` is the only crate linking `libc`, using `unsafe`, calling syscalls, or depending on `nix`/`inotify`/`epoll`. Enforced by `cargo deny`.

### 6. Async-native
All OSAL operations are async on Tokio. Blocking OS calls use `tokio::task::spawn_blocking`. Signal handling uses `tokio::signal`.

```rust
#[async_trait]
pub trait FileSystem: Send + Sync {
    async fn read(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<Vec<u8>>;
    async fn write(&self, path: &Path, data: &[u8], ctx: &CapabilityContext) -> OsalResult<()>;
    async fn watch(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<Receiver<FsEvent>>;
}
```

## Workspace Structure

11 crates:

```
ai-os/
├── Cargo.toml
└── osal/
    ├── osal-core/              # traits, KernelFacade, OsalError
    ├── osal-capabilities/      # Capability, CapabilitySet, CapabilityContext
    ├── osal-filesystem/        # FileSystem trait impl helpers
    ├── osal-process/           # ProcessManager trait impl helpers
    ├── osal-terminal/          # Terminal trait impl helpers
    ├── osal-network/           # NetworkManager trait impl helpers
    ├── osal-monitoring/        # SystemMonitor trait impl helpers
    ├── osal-devices/           # DeviceManager trait impl helpers
    ├── osal-users/             # UserManager trait impl helpers
    ├── osal-platform/          # PlatformInfo trait impl helpers
    └── osal-linux/             # Linux implementations (only unsafe crate)
```

### Dependency Graph

```
            ┌─────────────────────────────┐
            │     osal-linux              │
            │  (Linux implementation)     │
            └────────────┬────────────────┘
                         │ implements
            ┌────────────▼────────────────┐
            │  osal-core  │ KernelFacade │
            │  osal-capabilities          │
            └────────────┬────────────────┘
                         │ depends on
    ┌───────┬───────┬───┴───┬───────┬───────┐
    │ fs    │ proc  │ term  │ net   │ mon   │
    │       │       │       │       │       │
    │devices│ users │platform│       │       │
    └───────┴───────┴───────┴───────┴───────┘
```

No crate depends on `osal-linux`. It is a leaf implementation crate.

### Dependency Rules

| Direction | Rule |
|---|---|
| Upward | No OSAL crate depends on Core, Runtime, or any higher layer |
| Lateral | Subsystem crates depend only on `osal-core` and `osal-capabilities` |
| Lateral | Subsystem crates must NOT depend on each other |
| Downward | Only `osal-linux` depends on subsystem crates (implements their traits) |
| External | Only `osal-linux` depends on `nix`, `libc`, `inotify`, `tokio` |

## Crate Catalog

### osal-core

**Purpose:** Foundational crate defining all OSAL traits, `KernelFacade`, `OsalError`, subsystem event enums, and type aliases.

**Key types:** `KernelFacade`, `OsalError`, `OsalResult<T>`, `trait FileSystem`, `trait ProcessManager`, `trait Terminal`, `trait NetworkManager`, `trait SystemMonitor`, `trait DeviceManager`, `trait UserManager`, `trait PlatformInfo`, `FsEvent`, `ProcessEvent`, `NetworkEvent`, `DeviceEvent`, `MonitorEvent`, `UserEvent`, `SystemEvent`

**Dependencies:** None (foundational)

---

### osal-capabilities

**Purpose:** Defines the capability model: `Capability`, `CapabilitySet`, `CapabilityContext`.

**Key types:** `Capability` (30+ variants), `CapabilitySet` (wildcard path matching), `CapabilityContext`, `Subject`, `CapabilityError`, `AuditRecord`

**Dependencies:** `osal-core`

---

### osal-filesystem

**Purpose:** Async filesystem operations: read, write, create, delete, rename, metadata, directory listing, recursive operations, file watching.

**Key types:** `FileType`, `FileMetadata`, `DirectoryEntry`, `DiskUsage`, `FileSystemWatcher`, `FileSystemError`

**Dependencies:** `osal-core`, `osal-capabilities`

---

### osal-process

**Purpose:** Async process lifecycle: spawn, kill, signal, query, enumerate, resource usage, process tree walking.

**Key types:** `ProcessHandle`, `ProcessInfo`, `ProcessFilter`, `ProcessState`, `Signal`, `ProcessManagerError`

**Dependencies:** `osal-core`, `osal-capabilities`

---

### osal-terminal

**Purpose:** PTY management: create PTY pairs, resize, read/write master/slave, configure termios.

**Key types:** `PtyHandle`, `TerminalSize`, `TerminalCapabilities`, `WindowChangeEvent`, `TerminalError`

**Dependencies:** `osal-core`, `osal-capabilities`

---

### osal-network

**Purpose:** Network interface enumeration, link state monitoring, DNS resolution, socket statistics.

**Key types:** `NetworkInterface`, `InterfaceState`, `ConnectionMetadata`, `DnsResult`, `DnsResolver`, `SocketStat`, `NetworkEvent`, `NetworkError`

**Dependencies:** `osal-core`, `osal-capabilities`

---

### osal-monitoring

**Purpose:** System resource monitoring: CPU, memory, disk, network, load average, threshold-based alerting.

**Key types:** `CpuStats`, `MemoryStats`, `LoadAverage`, `DiskStats`, `NetworkStats`, `SystemInfo`, `MonitorConfig`, `MonitorError`

**Dependencies:** `osal-core`, `osal-capabilities`

---

### osal-devices

**Purpose:** Hardware device enumeration (udev), device queries, state monitoring.

**Key types:** `DeviceId`, `DeviceInfo`, `DeviceProperty`, `DeviceEvent`, `DeviceError`

**Dependencies:** `osal-core`, `osal-capabilities`

---

### osal-users

**Purpose:** User/group database queries, authentication, session tracking, password policy checks.

**Key types:** `UserInfo`, `GroupInfo`, `SessionInfo`, `AuthenticationToken`, `UserEvent`, `UserError`

**Dependencies:** `osal-core`, `osal-capabilities`

---

### osal-platform

**Purpose:** Platform metadata: kernel version, OS distribution, architecture, boot time, hostname, virtualization detection.

**Key types:** `KernelVersion`, `OsRelease`, `PlatformInfo`, `VirtualizationType`, `PlatformError`

**Dependencies:** `osal-core`, `osal-capabilities`

---

### osal-linux

**Purpose:** The only crate with Linux-specific code. Implements every OSAL trait against Linux using libc, nix, inotify, epoll, netlink, procfs, sysfs, udev.

**Key types:** `LinuxKernelFacade`, `LinuxFileSystem`, `LinuxProcessManager`, `LinuxTerminal`, `LinuxNetworkManager`, `LinuxSystemMonitor`, `LinuxDeviceManager`, `LinuxUserManager`, `LinuxPlatformInfo`

**Linux APIs used:**

| API | Purpose |
|---|---|
| `libc::open/read/write/close/stat/lstat` | Core file IO |
| `libc::fork/execvp/waitpid/kill/getpid` | Process lifecycle |
| `libc::openpty/ioctl/termios` | PTY management |
| `inotify` | File system event monitoring |
| `netlink` / `rtnetlink` | Network interface and route queries |
| `libudev` | Device enumeration and monitoring |
| `signalfd` (via `nix`) | Async signal handling |
| `/proc` parsing | Process, CPU, memory, disk statistics |
| `/sys` parsing | Device properties and driver info |
| `epoll` / `tokio::io::unix::AsyncFd` | Async IO readiness |

**Dependencies:** All OSAL crates (implements their traits)

---

## Kernel Facade Design

```rust
pub struct KernelFacade {
    pub filesystem: Arc<dyn FileSystem>,
    pub process: Arc<dyn ProcessManager>,
    pub terminal: Arc<dyn Terminal>,
    pub network: Arc<dyn NetworkManager>,
    pub monitoring: Arc<dyn SystemMonitor>,
    pub devices: Arc<dyn DeviceManager>,
    pub users: Arc<dyn UserManager>,
    pub platform: Arc<dyn PlatformInfo>,
}

impl KernelFacade {
    pub fn new(
        filesystem: Arc<dyn FileSystem>,
        process: Arc<dyn ProcessManager>,
        terminal: Arc<dyn Terminal>,
        network: Arc<dyn NetworkManager>,
        monitoring: Arc<dyn SystemMonitor>,
        devices: Arc<dyn DeviceManager>,
        users: Arc<dyn UserManager>,
        platform: Arc<dyn PlatformInfo>,
    ) -> Self {
        Self { filesystem, process, terminal, network, monitoring, devices, users, platform }
    }
}
```

**All subsystem traits use `Arc<dyn Trait>`** for polymorphic dispatch and testability. `Arc` ensures thread-safe shared ownership across tokio tasks.

**In production**, `LinuxKernelFacade::new()` constructs and wraps each Linux implementation:

```rust
let facade = KernelFacade::new(
    Arc::new(LinuxFileSystem::new()),
    Arc::new(LinuxProcessManager::new()),
    Arc::new(LinuxTerminal::new()),
    Arc::new(LinuxNetworkManager::new()),
    Arc::new(LinuxSystemMonitor::new()),
    Arc::new(LinuxDeviceManager::new()),
    Arc::new(LinuxUserManager::new()),
    Arc::new(LinuxPlatformInfo::new()),
);
```

**In tests**, mock implementations are injected via `MockKernelFacade`:

```rust
let mut mock = MockKernelFacade::new();
mock.filesystem
    .expect_read()
    .with(eq(path), anything())
    .returning(|_, _| Ok(b"test data".to_vec()));
```

**The facade is constructed once during boot** and shared via `Arc<KernelFacade>`. Every service needing OS access receives the `Arc` as a constructor parameter.

## Capability Framework

Every OSAL method checks caller identity and authorization before performing any operation.

### Core Types

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    FileRead(String), FileWrite(String), FileCreate(String),
    FileDelete(String), FileExecute(String), FileMetadata(String),
    ProcessSpawn, ProcessKill, ProcessSignal, ProcessList, ProcessSetPriority,
    TerminalCreate, TerminalRead, TerminalWrite, TerminalResize,
    NetworkConfigure, NetworkQuery, DnsResolve, SocketInspect,
    MonitorCpu, MonitorMemory, MonitorDisk, MonitorNetwork, MonitorSystem,
    DeviceList, DeviceQuery,
    ClipboardRead, ClipboardWrite,
    DisplayAccess, ScreenshotCapture,
    UserQuery, UserModify, GroupQuery, GroupModify, Authenticate,
    SystemInfo, SystemControl, ServiceManage, ClockQuery, ClockModify,
    Admin,
}
```

### CapabilitySet

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySet { caps: Vec<Capability> }

impl CapabilitySet {
    pub fn new(caps: Vec<Capability>) -> Self;
    pub fn check(&self, capability: &Capability) -> bool;
    pub fn intersection(&self, other: &CapabilitySet) -> CapabilitySet;
    pub fn union(&self, other: &CapabilitySet) -> CapabilitySet;
    pub fn is_subset(&self, other: &CapabilitySet) -> bool;
    pub fn is_empty(&self) -> bool;
    pub fn matching_paths(&self, path: &Path) -> Vec<&Capability>;
}
```

Supports wildcards: `/home/*`, `/home/**`, `/var/log/*.log`. Paths are canonicalized before matching to prevent symlink attacks.

### CapabilityContext

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityContext {
    pub subject: Subject,
    pub capabilities: CapabilitySet,
    pub request_id: Uuid,
}

impl CapabilityContext {
    pub fn check(&self, capability: &Capability) -> OsalResult<()> {
        if self.capabilities.contains(Admin) || self.capabilities.check(capability) {
            Ok(())
        } else {
            Err(OsalError::CapabilityDenied {
                subject: self.subject.clone(),
                required: capability.clone(),
                request_id: self.request_id,
            })
        }
    }
}
```

### Enforcement Pattern

```rust
#[async_trait]
impl FileSystem for LinuxFileSystem {
    async fn read(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<Vec<u8>> {
        ctx.check(&Capability::FileRead(path.to_string_lossy().into()))?;
        let canonical = canonicalize(path)?;
        Ok(tokio::fs::read(&canonical).await?)
    }
}
```

### Wildcard Path Matching

| Pattern | Matches | Does not match |
|---|---|---|
| `/home/*` | `/home/alice`, `/home/bob` | `/home/alice/secret.key` |
| `/home/**` | `/home/alice`, `/home/alice/secret.key` | `/etc/passwd` |
| `/var/log/*.log` | `/var/log/syslog.log` | `/var/log/syslog.log.1` |

### Admin Capability

`Capability::Admin` grants all capabilities. Granted only to: the bootstrap process (PID 1), services with `admin = true` in `platform.toml`, interactive users with admin group membership.

### Default Deny

If no capability matches, the operation fails. No implicit grants.

### Audit Integration

Every capability check produces an `AuditRecord` (timestamp, request_id, subject, capability, result, resource, error) emitted to the platform event bus.

## Event Model

Events are typed, structured, and emitted through `tokio::sync::mpsc::Receiver<T>` returned from `watch()` methods.

### Filesystem Events

```rust
pub enum FsEvent {
    FileCreated { path: PathBuf, metadata: FileMetadata, timestamp: SystemTime },
    FileModified { path: PathBuf, old_metadata: Option<FileMetadata>, new_metadata: FileMetadata, timestamp: SystemTime },
    FileDeleted { path: PathBuf, old_metadata: Option<FileMetadata>, timestamp: SystemTime },
    FileRenamed { from: PathBuf, to: PathBuf, timestamp: SystemTime },
    MetadataChanged { path: PathBuf, changed_fields: Vec<MetadataField>, timestamp: SystemTime },
}
```

Emitted on `IN_CREATE`, `IN_MODIFY` (coalesced at 100ms), `IN_DELETE`, `IN_MOVED_FROM`+`IN_MOVED_TO`, permission/ownership/xattr changes.

### Process Events

```rust
pub enum ProcessEvent {
    ProcessStarted { pid: u32, ppid: u32, uid: u32, command: String, timestamp: SystemTime },
    ProcessExited { pid: u32, exit_code: Option<i32>, signal: Option<Signal>, runtime: Duration, timestamp: SystemTime },
    ProcessSuspended { pid: u32, signal: Signal, timestamp: SystemTime },
    ProcessResumed { pid: u32, timestamp: SystemTime },
}
```

Emitted on proc connector/pidfd exec, termination, SIGSTOP/SIGTSTP/SIGTTIN/SIGTTOU, SIGCONT.

### Network Events

```rust
pub enum NetworkEvent {
    InterfaceUp { name: String, index: u32, addresses: Vec<IpAddr>, timestamp: SystemTime },
    InterfaceDown { name: String, index: u32, timestamp: SystemTime },
    ConnectivityGained { interface: String, gateway: Option<IpAddr>, dns_servers: Vec<IpAddr>, timestamp: SystemTime },
    ConnectivityLost { interface: String, last_gateway: Option<IpAddr>, timestamp: SystemTime },
    DnsResolved { hostname: String, addresses: Vec<IpAddr>, duration: Duration, timestamp: SystemTime },
}
```

Emitted on netlink `RTM_NEWLINK`/`RTM_DELLINK`, derived connectivity changes, every DNS resolution.

### Device Events

```rust
pub enum DeviceEvent {
    DeviceAttached { id: DeviceId, info: DeviceInfo, timestamp: SystemTime },
    DeviceRemoved { id: DeviceId, info: Option<DeviceInfo>, timestamp: SystemTime },
    DeviceStateChanged { id: DeviceId, property: String, old_value: Option<String>, new_value: String, timestamp: SystemTime },
    DeviceError { id: DeviceId, error: String, timestamp: SystemTime },
}
```

Emitted on udev `ADD`/`BIND`, `REMOVE`/`UNBIND`, `CHANGE` with property diff, udev error events.

### Monitoring Events

```rust
pub enum MonitorEvent {
    ThresholdCrossed { metric: MetricType, value: f64, threshold: f64, direction: ThresholdDirection, timestamp: SystemTime },
    Warning { metric: MetricType, value: f64, message: String, timestamp: SystemTime },
    Normalized { metric: MetricType, value: f64, previous_value: f64, timestamp: SystemTime },
}
```

### User Events

```rust
pub enum UserEvent {
    UserLoggedIn { uid: u32, username: String, session_id: String, login_type: LoginType, timestamp: SystemTime },
    UserLoggedOut { uid: u32, username: String, session_id: String, timestamp: SystemTime },
    UserLocked { uid: u32, username: String, reason: Option<String>, timestamp: SystemTime },
    UserUnlocked { uid: u32, username: String, timestamp: SystemTime },
    GroupModified { gid: u32, groupname: String, change: GroupChange, timestamp: SystemTime },
}
```

Emitted on PAM session open/close, shadow lock changes, `/etc/group` modification.

### System Events

```rust
pub enum SystemEvent {
    ServiceStateChanged { service_name: String, old_state: ServiceState, new_state: ServiceState, timestamp: SystemTime },
}
```

Emitted when systemd or platform services transition state.

## Error Model

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OsalError {
    CapabilityDenied { subject: Subject, required: Capability, request_id: Uuid },
    FileNotFound(PathBuf), FileAlreadyExists(PathBuf), FileNotDirectory(PathBuf),
    FileNotRegular(PathBuf), FilePermissionDenied(PathBuf), FileReadOnly(PathBuf),
    FileQuotaExceeded(PathBuf), FileSymlinkLoop(PathBuf), FileIo { path: PathBuf, detail: String },
    ProcessNotFound(u32), ProcessAlreadyExited(u32),
    ProcessSignalFailed { pid: u32, signal: Signal, detail: String },
    ProcessAccessDenied(u32), ProcessInvalidPid(u32), ProcessZombie(u32),
    TerminalPtyCreationFailed(String), TerminalInvalidSize { rows: u16, cols: u16 },
    TerminalIoctlFailed(String), TerminalNotAvailable,
    NetworkInterfaceNotFound(String), NetworkDnsFailure(String),
    NetworkNoRoute(String), NetworkPermissionDenied, NetworkLinkError(String),
    MonitorStatReadFailed(String), MonitorInvalidThreshold(String),
    DeviceNotFound(String), DevicePropertyNotFound { device: String, property: String }, DeviceUdevError(String),
    UserNotFound(u32), UserNotFoundByName(String), GroupNotFound(u32),
    GroupNotFoundByName(String), AuthenticationFailed,
    AuthenticationAccountLocked(String), AuthenticationPasswordExpired(String),
    PlatformQueryFailed(String), PlatformUnsupportedArchitecture(String),
    Internal(String),
}

pub type OsalResult<T> = Result<T, OsalError>;
```

### Design Rules

1. **Single enum, all errors.** Every OSAL operation returns `OsalResult<T>`.
2. **`thiserror` for derive macros.** `Display` and `source()` for every variant.
3. **Structured variants, no bare strings.** Programmatic handling.
4. **`CapabilityDenied` checked first.** Before any OS resource touch (prevents TOCTOU).
5. **`Internal` reserved for bugs.** Indicates invariant violation.
6. **Helper methods:** `is_not_found()`, `is_permission_denied()`, `is_capability_denied()`, `subject()`, `required_capability()`, `request_id()`.

## Thread Model

All OSAL traits and types are `Send + Sync`. All methods are `async fn`.

### spawn_blocking for CPU-bound Work

Operations parsing `/proc/*/stat`, walking directories, reading `/etc/passwd` with libc, or enumerating devices use `tokio::task::spawn_blocking`:

```rust
async fn list_processes(&self, ctx: &CapabilityContext) -> OsalResult<Vec<ProcessInfo>> {
    ctx.check(&Capability::ProcessList)?;
    let result = tokio::task::spawn_blocking(move || Self::parse_procfs())
        .await.map_err(|join| OsalError::Internal(join.to_string()))??;
    Ok(result)
}
```

### Event Channels

Watchers return `tokio::sync::mpsc::Receiver<T>` (bounded, capacity 256). Sender uses `try_send`; oldest events dropped if receiver falls behind.

### Subsystem State

| Pattern | Primitive | Example |
|---|---|---|
| Read-heavy | `tokio::sync::RwLock` | Device cache, interface list, user cache |
| Write-heavy | `std::sync::Mutex` | PTY allocation, watch registration |
| Atomic | `AtomicUsize`/`AtomicBool` | Sequence numbers, flags |

### Guarantees

No `thread_local!`, no `lazy_static`, no `OnceCell` for mutable state. No unsafe `Send`/`Sync` impls — all derived automatically.

## Async Model

### #[async_trait]

All OSAL traits use `#[async_trait]` (from `async-trait`), enabling async methods in `Arc<dyn Trait>`.

### Return Types

- Single-shot: `OsalResult<T>`
- Streaming: `OsalResult<Receiver<T>>`
- Subscription: `OsalResult<WatchHandle>` with `unsubscribe()`/`next_event()`

### Timeouts

| Operation | Default Timeout |
|---|---|
| DNS resolution | 10 s |
| File read/write | 30 s |
| Process wait | 60 s |
| Network config change | 15 s |
| Device enumeration | 5 s |

### Signal Handling

Uses `tokio::signal::unix`. `osal-linux` additionally uses `nix::sys::signalfd` wrapped in `tokio::io::unix::AsyncFd`.

### Cancellation

`.await` expressions are cancellation points. `mpsc::Receiver` drops on cancellation; sender observes `try_send` failure. File descriptors close on `Drop`.

## Lifecycle

### Construction

`LinuxKernelFacade::new()` is called once during boot, after the Tokio runtime is initialized:

```rust
#[tokio::main]
async fn main() {
    let kernel = Arc::new(LinuxKernelFacade::new().await);
    let core = CoreService::new(kernel.clone());
    core.run().await;
}
```

Subsystem initialization happens in `new()`. Errors propagate through `OsalResult<Self>`.

### No Explicit Start/Stop

Subsystems have no `start()`/`stop()` methods. They are ready from construction. Resources released on `Drop`.

### Lazy Event Activation

Event streams activate on first `watch()` call, avoiding unnecessary inotify/netlink subscriptions when no consumer exists.

### Shutdown

All `KernelFacade` references dropped → each subsystem's `Drop` closes file descriptors, drops netlink sockets, removes inotify watches, cancels watcher tasks.

## Testing Strategy

### Unit Tests (Subsystem Crates)

Each crate provides a `Mock*` type (e.g., `MockFileSystem`) storing `Box<dyn Fn(...)>` per method, returning default error if unset.

### Integration Tests (KernelFacade with Mocks)

Tests construct a `KernelFacade` with mock implementations to verify capability enforcement, error propagation, and event handling.

```rust
#[tokio::test]
async fn test_capability_denied_before_os_access() {
    let kernel = KernelFacade {
        filesystem: Arc::new(MockFileSystem::default()),
        // ... other mocks
    };
    let ctx = CapabilityContext::new(Subject::Service("test".into()), CapabilitySet::empty());
    let result = kernel.filesystem.read(Path::new("/etc/passwd"), &ctx).await;
    assert!(matches!(result, Err(OsalError::CapabilityDenied { .. })));
}
```

### osal-linux Testing

No unit tests. Integration tests run on actual Arch Linux requiring root privileges and access to `/proc`/`/sys`/`/dev`/`/etc/passwd`.

### Capability Framework Tests

Tested independently with known capability sets:

```rust
#[test]
fn test_wildcard_path_matching() {
    let set = CapabilitySet::new(vec![
        Capability::FileRead("/home/*".into()),
    ]);
    assert!(set.check(&Capability::FileRead("/home/alice".into())));
    assert!(!set.check(&Capability::FileRead("/etc/shadow".into())));
}

#[test]
fn test_admin_grants_all() {
    let set = CapabilitySet::new(vec![Capability::Admin]);
    assert!(set.check(&Capability::ProcessKill));
}
```

### Test Fixtures

`osal-test-utils` provides: `TempDir`, `DummyProcess`, `MockKernelFacade` builder, `TestCapabilityContext`, `EventCollector`.

## Security Model

### Capability Checking

Every public OSAL method takes `&CapabilityContext` as last parameter. Capability check is the first operation — before any OS resource access.

### Path Canonicalization

Path-based capabilities canonicalize paths before matching, preventing symlink traversal:

```rust
fn check_path(cap: &Capability, context: &CapabilityContext) -> OsalResult<()> {
    match cap {
        Capability::FileRead(path) => {
            let canonical = std::fs::canonicalize(path)
                .map_err(|_| OsalError::FileNotFound(path.clone()))?;
            // match against context capabilities with wildcards
        }
    }
}
```

### Default Deny

If no capability matches, the operation fails. Empty `CapabilitySet` performs zero OS operations.

### Audit Events

Every authorization failure emits an `AuditRecord` to the platform event bus.

### Admin Escalation

`Capability::Admin` is the only bypass. Granted to bootstrap process, services with `admin = true`, admin group members.

### Future Integration

Runtime's `PermissionChecker` will resolve capability grants from policy files and service manifests into `CapabilityContext`.

## Portability Guarantee

### The Platform Boundary

1. **Core, Runtime, Memory, Brain, Perception, Execution, and Intelligence are OS-independent.** They import only `osal_core` traits and `KernelFacade`. Never `osal_linux`.
2. **Porting to FreeBSD:** Implement all OSAL traits in `osal-freebsd`. No Core changes.
3. **Porting to macOS:** `osal-macos` implements traits against Darwin syscalls, GCD, IOKit.
4. **Porting to Windows:** Implement against Win32 API. Async model and capability framework unchanged.

### Enforcement

Only the platform binary root depends on `osal-linux`. CI runs `cargo check --deny unused-dependencies` and `cargo deny`.

### Virtual Platform

`osal-virtual` provides in-memory implementations (simulated filesystem, synthetic process table, fake network interfaces) enabling full-stack development without root, on any host OS.

### Porting Checklist

| Subsystem | FreeBSD | macOS | Windows |
|---|---|---|---|
| FileSystem | fts, kqueue | fts, FSEvents/kqueue | CreateFile, ReadDirectoryChangesW |
| ProcessManager | fork+exec, procfs | fork+exec, sysctl | CreateProcess, Toolhelp |
| Terminal | openpty, termios | openpty, termios | CreatePseudoConsole |
| NetworkManager | PF_ROUTE | SystemConfiguration | Win32_NetworkAdapter |
| SystemMonitor | sysctl, devstat | sysctl, IOKit | PDH, NtQuerySystemInformation |
| DeviceManager | devd, sysctl | IOKit | CM_Get_Device_ID_List |
| UserManager | getpwnam_r, PAM | getpwnam_r, OpenDirectory | NetUserGetInfo, SAM |
| PlatformInfo | uname, sysctl | uname, sysctl | GetVersionEx, GetSystemInfo |

## Cross-References

| Document | Section | Description |
|---|---|---|
| System Specification | §4 | Architectural principles — layered architecture and dependency inversion |
| System Specification | §12.3 | Module dependency graph — concrete OSAL crate slots |
| ADR-0002 | Clean Architecture | OSAL as infrastructure layer |
| RFC-0001 | OSAL Design | Original RFC proposing crate structure and KernelFacade pattern |
| Platform Manifest | `platform.toml` | Service capability grants, admin service list |
| `osal-core/README.md` | — | Detailed trait definitions and method signatures |
| `osal-capabilities/README.md` | — | Capability grammar, wildcard syntax, composition rules |
| `osal-linux/README.md` | — | Linux-specific notes, required kernel features |
| ADR-0007 | Capability Model | Capability-based security with default deny |
| ADR-0012 | Async OSAL | All OSAL operations async with Tokio |
