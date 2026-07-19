# osal-core — Operating System Abstraction Layer (Core)

This crate is the bedrock of the OSAL — Layer 1 of the AI-native OS stack. Every subsystem, driver, and service depends on the types, errors, events, and trait interfaces defined here. It depends on nothing except the standard library, tokio, serde, thiserror, chrono, async-trait, and osal-capabilities.

---

## Base Types

All base types are lightweight newtype wrappers with `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, and `Deserialize` derives.

```rust
pub struct Pid(pub u64);              // Process ID
pub struct Uid(pub u32);             // User ID
pub struct Gid(pub u32);             // Group ID
pub struct Fd(pub u64);              // File descriptor
pub struct Signal(pub i32);          // Signal number
pub struct Permissions(pub u32);     // Unix permission bits
pub struct UserId(pub String);       // String user identifier
pub struct SessionId(pub String);    // String session identifier

pub struct ExitStatus(
    pub Option<i32>,                  // Exit code (None if terminated by signal)
    pub Option<Signal>,               // Terminating signal (None if exited normally)
);
```

- `ExitStatus` represents process termination. If the process exited normally, the first field is `Some(code)` and the second is `None`. If killed by a signal, the first field is `None` and the second is `Some(signal)`.

---

## Error Types

### Subsystem Errors

Each OSAL subsystem defines its own error enum. Only the variant names and their payloads are specified here; implementations may add variants behind `#[non_exhaustive]`.

```rust
#[non_exhaustive]
pub enum FilesystemError {
    NotFound(String),
    PermissionDenied(String),
    AlreadyExists(String),
    NotADirectory(String),
    IsADirectory(String),
    Io(String),
    InvalidPath(String),
    StorageFull,
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[non_exhaustive]
pub enum ProcessError {
    NotFound(Pid),
    AlreadyExists(Pid),
    NotAllowed(String),
    ExecutionFailed(String),
    InvalidSignal(Signal),
    Io(String),
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[non_exhaustive]
pub enum TerminalError {
    NotFound(String),
    NotAvailable(String),
    Io(String),
    InvalidSignal(Signal),
    Disconnected,
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[non_exhaustive]
pub enum NetworkError {
    InterfaceNotFound(String),
    ConfigurationFailed(String),
    DnsFailed(String),
    ConnectionFailed(String),
    PermissionDenied(String),
    Io(String),
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[non_exhaustive]
pub enum MonitorError {
    NotAvailable(String),
    PermissionDenied(String),
    Io(String),
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[non_exhaustive]
pub enum DeviceError {
    NotFound(String),
    PermissionDenied(String),
    Busy(String),
    Io(String),
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[non_exhaustive]
pub enum UserError {
    NotFound(String),
    PermissionDenied(String),
    SwitchFailed(String),
    Io(String),
    Other(Box<dyn std::error::Error + Send + Sync>),
}
```

### Unified Error: `OsalError`

```rust
pub enum OsalError {
    Filesystem(FilesystemError),
    Process(ProcessError),
    Terminal(TerminalError),
    Network(NetworkError),
    Device(DeviceError),
    Monitor(MonitorError),
    User(UserError),
    CapabilityDenied(Capability),
    Timeout { operation: String, duration: Duration },
    Interrupted { operation: String, signal: Signal },
    ResourceExhausted { resource: String, limit: u64, requested: u64 },
    NotSupported { operation: String, platform: String },
    PermissionDenied { context: String },
    IoError(String),
}
```

### Result Alias

```rust
pub type OsalResult<T> = Result<T, OsalError>;
```

---

## Events: `OsalEvent`

Every subsystem can emit events through tokio `mpsc::Receiver<OsalEvent>` channels. All variants carry a `timestamp: DateTime<Utc>`.

```rust
pub enum OsalEvent {
    ProcessStarted { pid: Pid, command: String, timestamp: DateTime<Utc> },
    ProcessExited { pid: Pid, exit: ExitStatus, timestamp: DateTime<Utc> },
    FileCreated { path: String, file_type: String, timestamp: DateTime<Utc> },
    FileModified { path: String, size: u64, timestamp: DateTime<Utc> },
    FileDeleted { path: String, timestamp: DateTime<Utc> },
    NetworkConnected { interface: String, address: String, timestamp: DateTime<Utc> },
    NetworkDisconnected { interface: String, timestamp: DateTime<Utc> },
    DeviceAttached { device_id: String, device_type: String, timestamp: DateTime<Utc> },
    DeviceRemoved { device_id: String, device_type: String, timestamp: DateTime<Utc> },
    ClipboardChanged { content_type: String, timestamp: DateTime<Utc> },
    ScreenChanged { resolution: String, timestamp: DateTime<Utc> },
    SystemResourceWarning { resource: String, value: f64, threshold: f64, timestamp: DateTime<Utc> },
    UserLoggedIn { user_id: UserId, timestamp: DateTime<Utc> },
    UserLoggedOut { user_id: UserId, timestamp: DateTime<Utc> },
    TerminalOutput { pid: Pid, data: Vec<u8>, timestamp: DateTime<Utc> },
    ServiceStateChanged { service: String, state: String, timestamp: DateTime<Utc> },
}
```

---

## KernelFacade

`KernelFacade` is the single entry point through which all platform code accesses the OS. It holds `Arc<dyn Trait>` references to every subsystem. This design enables full dependency injection and testability.

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
```

### Subsystem Traits

All traits are `#[async_trait]`, `Send + Sync`, and use async methods. Implementations live in platform-specific crates (e.g. `osal-linux`).

#### `FileSystem`
- `async fn read(&self, path: &str) -> Result<Vec<u8>, FilesystemError>`
- `async fn write(&self, path: &str, data: &[u8]) -> Result<(), FilesystemError>`
- `async fn delete(&self, path: &str) -> Result<(), FilesystemError>`
- `async fn create_dir(&self, path: &str) -> Result<(), FilesystemError>`
- `async fn metadata(&self, path: &str) -> Result<FileMetadata, FilesystemError>`
- `async fn list(&self, path: &str) -> Result<Vec<DirEntry>, FilesystemError>`
- `async fn watch(&self, path: &str) -> Result<Receiver<OsalEvent>, FilesystemError>`

#### `ProcessManager`
- `async fn spawn(&self, command: &str, args: &[&str]) -> Result<ChildHandle, ProcessError>`
- `async fn kill(&self, pid: Pid, signal: Signal) -> Result<(), ProcessError>`
- `async fn suspend(&self, pid: Pid) -> Result<(), ProcessError>`
- `async fn resume(&self, pid: Pid) -> Result<(), ProcessError>`
- `async fn list(&self) -> Result<Vec<ProcessInfo>, ProcessError>`
- `async fn wait(&self, pid: Pid) -> Result<ExitStatus, ProcessError>`
- `fn events(&self) -> Receiver<OsalEvent>`

#### `Terminal`
- `async fn open_pty(&self) -> Result<PtyHandle, TerminalError>`
- `async fn write_pty(&self, id: &str, data: &[u8]) -> Result<(), TerminalError>`
- `async fn read_pty(&self, id: &str) -> Result<Vec<u8>, TerminalError>`
- `async fn resize_pty(&self, id: &str, rows: u16, cols: u16) -> Result<(), TerminalError>`
- `async fn signal_pty(&self, id: &str, signal: Signal) -> Result<(), TerminalError>`
- `fn events(&self) -> Receiver<OsalEvent>`

#### `NetworkManager`
- `async fn interfaces(&self) -> Result<Vec<InterfaceInfo>, NetworkError>`
- `async fn configure(&self, interface: &str, config: NetworkConfig) -> Result<(), NetworkError>`
- `async fn dns_lookup(&self, host: &str) -> Result<IpAddr, NetworkError>`
- `async fn set_hostname(&self, hostname: &str) -> Result<(), NetworkError>`
- `fn events(&self) -> Receiver<OsalEvent>`

#### `SystemMonitor`
- `async fn cpu_usage(&self) -> Result<f64, MonitorError>`
- `async fn memory_info(&self) -> Result<MemoryInfo, MonitorError>`
- `async fn disk_info(&self, path: &str) -> Result<DiskInfo, MonitorError>`
- `async fn network_io(&self) -> Result<NetworkIO, MonitorError>`
- `async fn temperature(&self) -> Result<f64, MonitorError>`
- `async fn process_list(&self) -> Result<Vec<ProcessInfo>, MonitorError>`
- `fn events(&self) -> Receiver<OsalEvent>`

#### `DeviceManager`
- `async fn enumerate(&self) -> Result<Vec<DeviceInfo>, DeviceError>`
- `async fn access(&self, path: &str) -> Result<DeviceHandle, DeviceError>`
- `fn events(&self) -> Receiver<OsalEvent>`

#### `UserManager`
- `async fn current_user(&self) -> Result<UserInfo, UserError>`
- `async fn enumerate_users(&self) -> Result<Vec<UserInfo>, UserError>`
- `async fn enumerate_groups(&self) -> Result<Vec<GroupInfo>, UserError>`
- `async fn switch_user(&self, user_id: &UserId) -> Result<(), UserError>`
- `async fn get_user_by_uid(&self, uid: Uid) -> Result<UserInfo, UserError>`
- `fn events(&self) -> Receiver<OsalEvent>`

#### `PlatformInfo`
- `fn os_name(&self) -> &str`
- `fn os_version(&self) -> &str`
- `fn hostname(&self) -> String`
- `fn num_cpus(&self) -> usize`
- `fn total_memory(&self) -> u64`
- `fn uptime(&self) -> Duration`
- `fn kernel_version(&self) -> &str`

### Associated Data Types

```rust
pub struct FileMetadata {
    pub path: String,
    pub size: u64,
    pub kind: FileKind,
    pub permissions: Permissions,
    pub modified: DateTime<Utc>,
    pub created: DateTime<Utc>,
    pub owner: Uid,
    pub group: Gid,
}

pub enum FileKind { File, Directory, Symlink, Socket, Pipe, BlockDevice, CharDevice, Other }

pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub kind: FileKind,
}

pub struct ChildHandle {
    pub pid: Pid,
}

pub struct ProcessInfo {
    pub pid: Pid,
    pub parent_pid: Option<Pid>,
    pub command: String,
    pub user: Uid,
    pub state: String,
    pub cpu_usage: f64,
    pub memory_usage: u64,
}

pub struct PtyHandle {
    pub id: String,
    pub pid: Option<Pid>,
}

pub struct InterfaceInfo {
    pub name: String,
    pub addresses: Vec<IpAddr>,
    pub mac_address: Option<String>,
    pub is_up: bool,
}

pub struct NetworkConfig {
    pub dhcp: bool,
    pub address: Option<IpAddr>,
    pub netmask: Option<IpAddr>,
    pub gateway: Option<IpAddr>,
    pub dns_servers: Vec<IpAddr>,
}

pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub cached: u64,
}

pub struct DiskInfo {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub mount_point: String,
    pub filesystem: String,
}

pub struct NetworkIO {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
}

pub struct DeviceInfo {
    pub id: String,
    pub device_type: String,
    pub driver: Option<String>,
    pub description: String,
}

pub struct DeviceHandle {
    pub path: String,
    pub fd: Fd,
}

pub struct UserInfo {
    pub uid: Uid,
    pub gid: Gid,
    pub username: String,
    pub home_dir: String,
    pub shell: String,
}

pub struct GroupInfo {
    pub gid: Gid,
    pub name: String,
    pub members: Vec<String>,
}
```
