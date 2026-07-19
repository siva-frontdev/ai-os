# OS Abstraction Layer Interface — `osal`

## Purpose

OSAL is the sole OS-specific layer in the stack. It wraps raw Linux syscalls (and optionally BSD/Win32 stubs) behind eight portable subsystem traits. Every layer above OSAL — including Core — goes through `KernelFacade` to access OS services. No Linux type (`iovec`, `epoll`, `stat`, `termios`, etc.) appears in any trait signature. This is the abstraction firewall that keeps the entire stack portable.

## Public APIs

### KernelFacade — single entry point, capability-gated

```rust
pub struct KernelFacade {
    fs: Arc<dyn FileSystem>,
    proc: Arc<dyn ProcessManager>,
    term: Arc<dyn Terminal>,
    net: Arc<dyn NetworkManager>,
    mon: Arc<dyn SystemMonitor>,
    dev: Arc<dyn DeviceManager>,
    users: Arc<dyn UserManager>,
    info: Arc<dyn PlatformInfo>,
}

impl KernelFacade {
    pub async fn open(ctx: CapabilityContext) -> Result<Self, OsalError>;
    pub fn filesystem(&self) -> &dyn FileSystem;
    pub fn processes(&self) -> &dyn ProcessManager;
    pub fn terminal(&self) -> &dyn Terminal;
    pub fn network(&self) -> &dyn NetworkManager;
    pub fn monitor(&self) -> &dyn SystemMonitor;
    pub fn devices(&self) -> &dyn DeviceManager;
    pub fn users(&self) -> &dyn UserManager;
    pub fn platform(&self) -> &dyn PlatformInfo;
}
```

### FileSystem — portable file I/O

```rust
#[async_trait]
pub trait FileSystem: Debug + Send + Sync {
    async fn open(&self, path: &Path, opts: OpenOpts) -> Result<FileHandle, FsError>;
    async fn read(&self, handle: &FileHandle, buf: &mut [u8], offset: u64) -> Result<usize, FsError>;
    async fn write(&self, handle: &FileHandle, buf: &[u8], offset: u64) -> Result<usize, FsError>;
    async fn close(&self, handle: FileHandle) -> Result<(), FsError>;
    async fn stat(&self, path: &Path) -> Result<FileStat, FsError>;
    async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>;
    async fn create_dir(&self, path: &Path, mode: FileMode) -> Result<(), FsError>;
    async fn remove(&self, path: &Path) -> Result<(), FsError>;
    async fn rename(&self, from: &Path, to: &Path) -> Result<(), FsError>;
    async fn watch(&self, path: &Path) -> Result<Box<dyn Stream<Item = FsEvent> + Unpin + Send>, FsError>;
}
```

### ProcessManager — process lifecycle

```rust
#[async_trait]
pub trait ProcessManager: Debug + Send + Sync {
    async fn spawn(&self, cmd: CommandSpec) -> Result<ProcessHandle, ProcError>;
    async fn wait(&self, handle: &ProcessHandle) -> Result<ExitStatus, ProcError>;
    async fn signal(&self, handle: &ProcessHandle, sig: Signal) -> Result<(), ProcError>;
    async fn kill(&self, handle: &ProcessHandle) -> Result<(), ProcError>;
    async fn enumerate(&self) -> Result<Vec<ProcessInfo>, ProcError>;
    async fn environ(&self, handle: &ProcessHandle) -> Result<HashMap<String, String>, ProcError>;
}
```

### Terminal — PTY abstraction

```rust
#[async_trait]
pub trait Terminal: Debug + Send + Sync {
    async fn open(&self, size: TermSize) -> Result<TermHandle, TermError>;
    async fn write(&self, handle: &TermHandle, data: &[u8]) -> Result<usize, TermError>;
    async fn read(&self, handle: &TermHandle, buf: &mut [u8]) -> Result<usize, TermError>;
    async fn resize(&self, handle: &TermHandle, size: TermSize) -> Result<(), TermError>;
    async fn close(&self, handle: TermHandle) -> Result<(), TermError>;
    fn events(&self, handle: &TermHandle) -> Box<dyn Stream<Item = TermEvent> + Unpin + Send>;
}
```

### NetworkManager — socket and interface abstraction

```rust
#[async_trait]
pub trait NetworkManager: Debug + Send + Sync {
    async fn tcp_connect(&self, addr: SocketAddr) -> Result<NetHandle, NetError>;
    async fn tcp_listen(&self, addr: SocketAddr) -> Result<NetListener, NetError>;
    async fn accept(&self, listener: &NetListener) -> Result<(NetHandle, SocketAddr), NetError>;
    async fn send(&self, handle: &NetHandle, buf: &[u8]) -> Result<usize, NetError>;
    async fn recv(&self, handle: &NetHandle, buf: &mut [u8]) -> Result<usize, NetError>;
    async fn close(&self, handle: NetHandle) -> Result<(), NetError>;
    async fn interfaces(&self) -> Result<Vec<NetInterface>, NetError>;
}
```

### SystemMonitor — resource usage snapshots

```rust
#[async_trait]
pub trait SystemMonitor: Debug + Send + Sync {
    async fn cpu(&self) -> Result<CpuSnapshot, MonError>;
    async fn memory(&self) -> Result<MemSnapshot, MonError>;
    async fn disk(&self, path: &Path) -> Result<DiskSnapshot, MonError>;
    async fn load(&self) -> Result<LoadAvg, MonError>;
    async fn uptime(&self) -> Result<Duration, MonError>;
    fn stream(&self, interval: Duration) -> Box<dyn Stream<Item = MonSnapshot> + Unpin + Send>;
}
```

### DeviceManager — hardware enumeration

```rust
#[async_trait]
pub trait DeviceManager: Debug + Send + Sync {
    async fn enumerate(&self) -> Result<Vec<DeviceInfo>, DevError>;
    async fn by_class(&self, class: DeviceClass) -> Result<Vec<DeviceInfo>, DevError>;
    async fn events(&self) -> Box<dyn Stream<Item = DeviceEvent> + Unpin + Send>;
    async fn ioctl(&self, handle: &DeviceHandle, req: IoctlRequest) -> Result<IoctlResponse, DevError>;
}
```

### UserManager — authentication and identity

```rust
#[async_trait]
pub trait UserManager: Debug + Send + Sync {
    async fn authenticate(&self, creds: Credentials) -> Result<UserSession, UserError>;
    async fn lookup(&self, uid: Uid) -> Result<UserInfo, UserError>;
    async fn groups(&self, uid: Uid) -> Result<Vec<GroupInfo>, UserError>;
    async fn switch_user(&self, ctx: CapabilityContext, uid: Uid) -> Result<CapabilityContext, UserError>;
}
```

### PlatformInfo — OS and hardware metadata

```rust
pub trait PlatformInfo: Debug + Send + Sync {
    fn os_name(&self) -> &str;
    fn os_version(&self) -> &str;
    fn arch(&self) -> &str;
    fn hostname(&self) -> Result<String, InfoError>;
    fn num_cpus(&self) -> usize;
    fn page_size(&self) -> usize;
}
```

## Dependencies

- [Core](core.md) — `EventBus`, `Logger`, `Config` (for mount table, network config)
- `nix` (Linux syscall wrappers)
- `tokio` (async I/O via `tokio::fs`, `tokio::net`)
- `inotify` / `fanotify` (file watching, optional)

## Lifecycle

1. **Init** — `KernelFacade::open` checks `CapabilityContext` for OS-level capabilities. Subsystems are lazily initialized on first access.
2. **Start** — `SystemMonitor` begins periodic sampling. `DeviceManager` opens a udev monitor socket. NOOP for other subsystems.
3. **Stop** — All handles are drained. Event streams are terminated. `DeviceManager` closes the udev socket.

## Events Published

| Event | Payload | Description |
|-------|---------|-------------|
| `osal.fs_modified` | `{ path, kind: Create|Modify|Delete }` | File system change (watched paths) |
| `osal.device_added` | `{ devpath, class, vendor }` | Hotplug device attached |
| `osal.device_removed` | `{ devpath, class }` | Hotplug device detached |
| `osal.process_exit` | `{ pid, status }` | Child process exited |
| `osal.monitor_threshold` | `{ resource, value, threshold }` | System resource above threshold |

## Events Consumed

None. OSAL is a leaf layer — it does not subscribe to events from other layers.

## Error Model

| Error | Variant | Recovery |
|-------|---------|----------|
| `FsError::NotFound` | ENOENT | Caller checks path validity |
| `FsError::PermissionDenied` | EACCES | Capability check failed |
| `FsError::Io` | Raw OS I/O error (wrapped) | Retry with backoff |
| `ProcError::NotFound` | ESRCH | Process already exited |
| `NetError::ConnectionRefused` | ECONNREFUSED | Retry with backoff |
| `DevError::NoSuchDevice` | Hotplug race | Re-enumerate |
| `TermError::BrokenPipe` | PTY peer closed | Close handle |

## Performance Expectations

| Operation | Target Latency | Notes |
|-----------|---------------|-------|
| File read (4K cached) | < 5 µs | Direct syscall, no allocation |
| File read (4K uncached) | < 100 µs | Page fault + disk I/O |
| TCP connect (loopback) | < 50 µs | Includes syscall overhead |
| Process spawn | < 1 ms | fork+exec, no shell |
| CPU snapshot | < 10 µs | Read /proc/stat |
| Device enumeration | < 5 ms | udev database query |

## Thread Model

- `FileSystem`, `NetworkManager`, `Terminal` delegate to tokio's async I/O primitives — no blocking the reactor.
- `ProcessManager::wait` uses `tokio::task::spawn_blocking` for `waitpid`.
- `SystemMonitor` runs on a tokio interval — each sample is a `spawn_blocking` call.
- `DeviceManager` runs a dedicated tokio task for udev socket events.
- All subsystem implementations are `Send + Sync`. The `KernelFacade` struct is `Send + Sync` by construction.

## Portability

- **No Linux types in trait signatures.** `OpenOpts`, `FileStat`, `ExitStatus`, `Signal`, `TermSize`, `SocketAddr` are all OSAL-defined types that map to OS-native values internally.
- Each subsystem has a `#[cfg(target_os = "linux")]` and `#[cfg(not(target_os = "linux"))]` module. The non-Linux module returns `Unsupported` errors for genuinely OS-specific operations.
- `Path` and `PathBuf` are used for all filesystem paths — no C strings cross the trait boundary.
- `SocketAddr` is `std::net::SocketAddr`, which is portable.
- `Signal` is an OSAL enum (`SigTerm`, `SigKill`, `SigInt`, etc.) — raw signum values never leak.
