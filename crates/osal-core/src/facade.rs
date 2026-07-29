use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use osal_capabilities::CapabilityContext;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

use crate::error::*;
use crate::events::OsalEvent;
use crate::types::*;

// ---------------------------------------------------------------------------
// Data types returned by subsystem traits
// ---------------------------------------------------------------------------

/// Metadata about a filesystem entry.
#[derive(Debug, Clone)]
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

/// Classification of a filesystem entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    Socket,
    Pipe,
    BlockDevice,
    CharDevice,
    Other,
}

/// A single entry in a directory listing.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub kind: FileKind,
}

/// Handle returned after spawning a child process.
#[derive(Debug, Clone)]
pub struct ChildHandle {
    pub pid: Pid,
}

/// Snapshot of a running process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: Pid,
    pub parent_pid: Option<Pid>,
    pub command: String,
    pub user: Uid,
    pub state: String,
    pub cpu_usage: f64,
    pub memory_usage: u64,
}

/// Handle to a pseudo-terminal (PTY).
#[derive(Debug, Clone)]
pub struct PtyHandle {
    pub id: String,
    pub pid: Option<Pid>,
}

/// Describes a network interface.
#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub name: String,
    pub addresses: Vec<IpAddr>,
    pub mac_address: Option<String>,
    pub is_up: bool,
}

/// Configuration applied to a network interface.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub dhcp: bool,
    pub address: Option<IpAddr>,
    pub netmask: Option<IpAddr>,
    pub gateway: Option<IpAddr>,
    pub dns_servers: Vec<IpAddr>,
}

/// Memory usage snapshot.
#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub cached: u64,
}

/// Disk usage for a mount point.
#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub mount_point: String,
    pub filesystem: String,
}

/// Network I/O statistics.
#[derive(Debug, Clone)]
pub struct NetworkIO {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
}

/// Describes a hardware device.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub device_type: String,
    pub driver: Option<String>,
    pub description: String,
}

/// An opened device file.
#[derive(Debug, Clone)]
pub struct DeviceHandle {
    pub path: String,
    pub fd: Fd,
}

/// Information about a system user.
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub uid: Uid,
    pub gid: Gid,
    pub username: String,
    pub home_dir: String,
    pub shell: String,
}

/// Information about a system group.
#[derive(Debug, Clone)]
pub struct GroupInfo {
    pub gid: Gid,
    pub name: String,
    pub members: Vec<String>,
}

// ---------------------------------------------------------------------------
// Subsystem traits
// ---------------------------------------------------------------------------

/// Filesystem abstraction — read, write, delete, list, watch files and directories.
#[async_trait]
pub trait FileSystem: Send + Sync {
    async fn read(&self, ctx: &CapabilityContext, path: &str) -> Result<Vec<u8>, FilesystemError>;

    async fn write(
        &self,
        ctx: &CapabilityContext,
        path: &str,
        data: &[u8],
    ) -> Result<(), FilesystemError>;

    async fn delete(&self, ctx: &CapabilityContext, path: &str) -> Result<(), FilesystemError>;

    async fn create_dir(&self, ctx: &CapabilityContext, path: &str) -> Result<(), FilesystemError>;

    async fn metadata(
        &self,
        ctx: &CapabilityContext,
        path: &str,
    ) -> Result<FileMetadata, FilesystemError>;

    async fn list(
        &self,
        ctx: &CapabilityContext,
        path: &str,
    ) -> Result<Vec<DirEntry>, FilesystemError>;

    async fn watch(
        &self,
        ctx: &CapabilityContext,
        path: &str,
    ) -> Result<Receiver<OsalEvent>, FilesystemError>;
}

/// Process manager — spawn, kill, suspend, resume, enumerate processes.
#[async_trait]
pub trait ProcessManager: Send + Sync {
    async fn spawn(
        &self,
        ctx: &CapabilityContext,
        command: &str,
        args: &[&str],
    ) -> Result<ChildHandle, ProcessError>;

    async fn kill(
        &self,
        ctx: &CapabilityContext,
        pid: Pid,
        signal: Signal,
    ) -> Result<(), ProcessError>;

    async fn suspend(&self, ctx: &CapabilityContext, pid: Pid) -> Result<(), ProcessError>;

    async fn resume(&self, ctx: &CapabilityContext, pid: Pid) -> Result<(), ProcessError>;

    async fn list(&self, ctx: &CapabilityContext) -> Result<Vec<ProcessInfo>, ProcessError>;

    async fn wait(&self, ctx: &CapabilityContext, pid: Pid) -> Result<ExitStatus, ProcessError>;

    fn events(&self) -> Receiver<OsalEvent>;
}

/// Terminal / PTY abstraction.
#[async_trait]
pub trait Terminal: Send + Sync {
    async fn open_pty(&self, ctx: &CapabilityContext) -> Result<PtyHandle, TerminalError>;

    async fn write_pty(
        &self,
        ctx: &CapabilityContext,
        id: &str,
        data: &[u8],
    ) -> Result<(), TerminalError>;

    async fn read_pty(&self, ctx: &CapabilityContext, id: &str) -> Result<Vec<u8>, TerminalError>;

    async fn resize_pty(
        &self,
        ctx: &CapabilityContext,
        id: &str,
        rows: u16,
        cols: u16,
    ) -> Result<(), TerminalError>;

    async fn signal_pty(
        &self,
        ctx: &CapabilityContext,
        id: &str,
        signal: Signal,
    ) -> Result<(), TerminalError>;

    fn events(&self) -> Receiver<OsalEvent>;
}

/// Network management — interfaces, configuration, DNS.
#[async_trait]
pub trait NetworkManager: Send + Sync {
    async fn interfaces(&self, ctx: &CapabilityContext)
        -> Result<Vec<InterfaceInfo>, NetworkError>;

    async fn configure(
        &self,
        ctx: &CapabilityContext,
        interface: &str,
        config: NetworkConfig,
    ) -> Result<(), NetworkError>;

    async fn dns_lookup(&self, ctx: &CapabilityContext, host: &str)
        -> Result<IpAddr, NetworkError>;

    async fn set_hostname(
        &self,
        ctx: &CapabilityContext,
        hostname: &str,
    ) -> Result<(), NetworkError>;

    fn events(&self) -> Receiver<OsalEvent>;
}

/// System resource monitoring.
#[async_trait]
pub trait SystemMonitor: Send + Sync {
    async fn cpu_usage(&self, ctx: &CapabilityContext) -> Result<f64, MonitorError>;

    async fn memory_info(&self, ctx: &CapabilityContext) -> Result<MemoryInfo, MonitorError>;

    async fn disk_info(
        &self,
        ctx: &CapabilityContext,
        path: &str,
    ) -> Result<DiskInfo, MonitorError>;

    async fn network_io(&self, ctx: &CapabilityContext) -> Result<NetworkIO, MonitorError>;

    async fn temperature(&self, ctx: &CapabilityContext) -> Result<f64, MonitorError>;

    async fn process_list(&self, ctx: &CapabilityContext)
        -> Result<Vec<ProcessInfo>, MonitorError>;

    fn events(&self) -> Receiver<OsalEvent>;
}

/// Hardware device enumeration and access.
#[async_trait]
pub trait DeviceManager: Send + Sync {
    async fn enumerate(&self, ctx: &CapabilityContext) -> Result<Vec<DeviceInfo>, DeviceError>;

    async fn access(
        &self,
        ctx: &CapabilityContext,
        path: &str,
    ) -> Result<DeviceHandle, DeviceError>;

    fn events(&self) -> Receiver<OsalEvent>;
}

/// User and group management.
#[async_trait]
pub trait UserManager: Send + Sync {
    async fn current_user(&self, ctx: &CapabilityContext) -> Result<UserInfo, UserError>;

    async fn enumerate_users(&self, ctx: &CapabilityContext) -> Result<Vec<UserInfo>, UserError>;

    async fn enumerate_groups(&self, ctx: &CapabilityContext) -> Result<Vec<GroupInfo>, UserError>;

    async fn switch_user(&self, ctx: &CapabilityContext, user_id: &UserId)
        -> Result<(), UserError>;

    async fn get_user_by_uid(
        &self,
        ctx: &CapabilityContext,
        uid: Uid,
    ) -> Result<UserInfo, UserError>;

    fn events(&self) -> Receiver<OsalEvent>;
}

/// Read-only platform and kernel information (sync — no capability check needed).
pub trait PlatformInfo: Send + Sync {
    fn os_name(&self) -> &str;
    fn os_version(&self) -> &str;
    fn hostname(&self) -> String;
    fn num_cpus(&self) -> usize;
    fn total_memory(&self) -> u64;
    fn uptime(&self) -> Duration;
    fn kernel_version(&self) -> &str;
}

/// Rectangle dimensions (screen region or window geometry).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Information about a desktop window.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub window_id: String,
    pub title: String,
    pub process: Option<String>,
    pub geometry: Option<Rect>,
    pub is_visible: bool,
}

// ---------------------------------------------------------------------------
// Desktop subsystem traits
// ---------------------------------------------------------------------------

/// Window management — list, focus, close windows; screen dimensions.
#[async_trait]
pub trait WindowManager: Send + Sync {
    async fn list_windows(&self, ctx: &CapabilityContext) -> Result<Vec<WindowInfo>, WindowError>;

    async fn focused_window(
        &self,
        ctx: &CapabilityContext,
    ) -> Result<Option<WindowInfo>, WindowError>;

    async fn focus_window(
        &self,
        ctx: &CapabilityContext,
        title: &str,
        partial: bool,
    ) -> Result<(), WindowError>;

    async fn close_window(&self, ctx: &CapabilityContext, title: &str) -> Result<(), WindowError>;

    async fn screen_dimensions(&self, ctx: &CapabilityContext) -> Result<(u32, u32), WindowError>;
}

/// Input device abstraction — mouse and keyboard.
#[async_trait]
pub trait InputDevice: Send + Sync {
    async fn mouse_move(&self, ctx: &CapabilityContext, x: i32, y: i32) -> Result<(), InputError>;

    async fn mouse_click(&self, ctx: &CapabilityContext, button: &str) -> Result<(), InputError>;

    async fn keyboard_type(&self, ctx: &CapabilityContext, text: &str) -> Result<(), InputError>;

    async fn keyboard_combo(
        &self,
        ctx: &CapabilityContext,
        keys: &[&str],
    ) -> Result<(), InputError>;

    async fn cursor_position(&self, ctx: &CapabilityContext) -> Result<(i32, i32), InputError>;
}

/// Clipboard access — read and write text content.
#[async_trait]
pub trait ClipboardProvider: Send + Sync {
    async fn get_text(&self, ctx: &CapabilityContext) -> Result<Option<String>, DesktopError>;

    async fn set_text(&self, ctx: &CapabilityContext, text: &str) -> Result<(), DesktopError>;
}

/// Higher-level desktop operations — URL opening, app launching, screenshots, display info.
#[async_trait]
pub trait DesktopProvider: Send + Sync {
    async fn open_url(&self, ctx: &CapabilityContext, url: &str) -> Result<(), DesktopError>;

    async fn launch_app(&self, ctx: &CapabilityContext, app: &str) -> Result<(), DesktopError>;

    async fn take_screenshot(
        &self,
        ctx: &CapabilityContext,
        path: &str,
    ) -> Result<(), DesktopError>;

    /// Returns "x11", "wayland", or an error if no display server is available.
    fn display_server(&self) -> Result<String, DesktopError>;
}

// ---------------------------------------------------------------------------
// KernelFacade — the central access point
// ---------------------------------------------------------------------------

/// Single entry point for all OS-level operations.
pub struct KernelFacade {
    pub filesystem: Arc<dyn FileSystem>,
    pub process: Arc<dyn ProcessManager>,
    pub terminal: Arc<dyn Terminal>,
    pub network: Arc<dyn NetworkManager>,
    pub monitoring: Arc<dyn SystemMonitor>,
    pub devices: Arc<dyn DeviceManager>,
    pub users: Arc<dyn UserManager>,
    pub platform: Arc<dyn PlatformInfo>,
    pub windows: Arc<dyn WindowManager>,
    pub input: Arc<dyn InputDevice>,
    pub clipboard: Arc<dyn ClipboardProvider>,
    pub desktop: Arc<dyn DesktopProvider>,
}

// ---------------------------------------------------------------------------
// Mock implementations for testing
// ---------------------------------------------------------------------------

pub mod mock {
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::sync::mpsc;

    use super::*;

    static NEXT_PID: AtomicU64 = AtomicU64::new(1000);

    fn dummy_ctx() -> CapabilityContext {
        CapabilityContext::new("test")
    }

    /// Creates a channel that immediately closes (empty event stream).
    fn closed_rx() -> Receiver<OsalEvent> {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        rx
    }

    pub struct MockFileSystem;
    impl MockFileSystem {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockFileSystem {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockFileSystem").finish()
        }
    }
    #[async_trait]
    impl FileSystem for MockFileSystem {
        async fn read(
            &self,
            _ctx: &CapabilityContext,
            path: &str,
        ) -> Result<Vec<u8>, FilesystemError> {
            if path.is_empty() {
                Err(FilesystemError::NotFound(path.to_string()))
            } else {
                Ok(vec![])
            }
        }
        async fn write(
            &self,
            _ctx: &CapabilityContext,
            _path: &str,
            _data: &[u8],
        ) -> Result<(), FilesystemError> {
            Ok(())
        }
        async fn delete(
            &self,
            _ctx: &CapabilityContext,
            path: &str,
        ) -> Result<(), FilesystemError> {
            if path.is_empty() {
                Err(FilesystemError::NotFound(path.to_string()))
            } else {
                Ok(())
            }
        }
        async fn create_dir(
            &self,
            _ctx: &CapabilityContext,
            path: &str,
        ) -> Result<(), FilesystemError> {
            if path.is_empty() {
                Err(FilesystemError::InvalidPath(path.to_string()))
            } else {
                Ok(())
            }
        }
        async fn metadata(
            &self,
            _ctx: &CapabilityContext,
            path: &str,
        ) -> Result<FileMetadata, FilesystemError> {
            if path.is_empty() {
                Err(FilesystemError::NotFound(path.to_string()))
            } else {
                Ok(FileMetadata {
                    path: path.to_string(),
                    size: 0,
                    kind: FileKind::File,
                    permissions: Permissions(0o644),
                    modified: Utc::now(),
                    created: Utc::now(),
                    owner: Uid(0),
                    group: Gid(0),
                })
            }
        }
        async fn list(
            &self,
            _ctx: &CapabilityContext,
            path: &str,
        ) -> Result<Vec<DirEntry>, FilesystemError> {
            if path.is_empty() {
                Err(FilesystemError::NotFound(path.to_string()))
            } else {
                Ok(vec![])
            }
        }
        async fn watch(
            &self,
            _ctx: &CapabilityContext,
            _path: &str,
        ) -> Result<Receiver<OsalEvent>, FilesystemError> {
            Ok(closed_rx())
        }
    }

    pub struct MockProcessManager;
    impl MockProcessManager {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockProcessManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockProcessManager").finish()
        }
    }
    #[async_trait]
    impl ProcessManager for MockProcessManager {
        async fn spawn(
            &self,
            _ctx: &CapabilityContext,
            command: &str,
            _args: &[&str],
        ) -> Result<ChildHandle, ProcessError> {
            if command.is_empty() {
                Err(ProcessError::ExecutionFailed("empty command".to_string()))
            } else {
                let pid = Pid(NEXT_PID.fetch_add(1, Ordering::SeqCst));
                Ok(ChildHandle { pid })
            }
        }
        async fn kill(
            &self,
            _ctx: &CapabilityContext,
            pid: Pid,
            _signal: Signal,
        ) -> Result<(), ProcessError> {
            if pid.0 == 0 {
                Err(ProcessError::NotFound(pid))
            } else {
                Ok(())
            }
        }
        async fn suspend(&self, _ctx: &CapabilityContext, pid: Pid) -> Result<(), ProcessError> {
            if pid.0 == 0 {
                Err(ProcessError::NotFound(pid))
            } else {
                Ok(())
            }
        }
        async fn resume(&self, _ctx: &CapabilityContext, pid: Pid) -> Result<(), ProcessError> {
            if pid.0 == 0 {
                Err(ProcessError::NotFound(pid))
            } else {
                Ok(())
            }
        }
        async fn list(&self, _ctx: &CapabilityContext) -> Result<Vec<ProcessInfo>, ProcessError> {
            Ok(vec![])
        }
        async fn wait(
            &self,
            _ctx: &CapabilityContext,
            pid: Pid,
        ) -> Result<ExitStatus, ProcessError> {
            if pid.0 == 0 {
                Err(ProcessError::NotFound(pid))
            } else {
                Ok(ExitStatus(Some(0), None))
            }
        }
        fn events(&self) -> Receiver<OsalEvent> {
            closed_rx()
        }
    }

    pub struct MockTerminal;
    impl MockTerminal {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockTerminal {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockTerminal").finish()
        }
    }
    #[async_trait]
    impl Terminal for MockTerminal {
        async fn open_pty(&self, _ctx: &CapabilityContext) -> Result<PtyHandle, TerminalError> {
            Ok(PtyHandle {
                id: "mock-pty".into(),
                pid: None,
            })
        }
        async fn write_pty(
            &self,
            _ctx: &CapabilityContext,
            _id: &str,
            _data: &[u8],
        ) -> Result<(), TerminalError> {
            Ok(())
        }
        async fn read_pty(
            &self,
            _ctx: &CapabilityContext,
            _id: &str,
        ) -> Result<Vec<u8>, TerminalError> {
            Ok(vec![])
        }
        async fn resize_pty(
            &self,
            _ctx: &CapabilityContext,
            _id: &str,
            _rows: u16,
            _cols: u16,
        ) -> Result<(), TerminalError> {
            Ok(())
        }
        async fn signal_pty(
            &self,
            _ctx: &CapabilityContext,
            _id: &str,
            _signal: Signal,
        ) -> Result<(), TerminalError> {
            Ok(())
        }
        fn events(&self) -> Receiver<OsalEvent> {
            closed_rx()
        }
    }

    pub struct MockNetworkManager;
    impl MockNetworkManager {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockNetworkManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockNetworkManager").finish()
        }
    }
    #[async_trait]
    impl NetworkManager for MockNetworkManager {
        async fn interfaces(
            &self,
            _ctx: &CapabilityContext,
        ) -> Result<Vec<InterfaceInfo>, NetworkError> {
            Ok(vec![])
        }
        async fn configure(
            &self,
            _ctx: &CapabilityContext,
            _interface: &str,
            _config: NetworkConfig,
        ) -> Result<(), NetworkError> {
            Ok(())
        }
        async fn dns_lookup(
            &self,
            _ctx: &CapabilityContext,
            host: &str,
        ) -> Result<IpAddr, NetworkError> {
            if host.is_empty() {
                Err(NetworkError::DnsFailed("empty hostname".to_string()))
            } else {
                Ok("127.0.0.1".parse().unwrap())
            }
        }
        async fn set_hostname(
            &self,
            _ctx: &CapabilityContext,
            _hostname: &str,
        ) -> Result<(), NetworkError> {
            Ok(())
        }
        fn events(&self) -> Receiver<OsalEvent> {
            closed_rx()
        }
    }

    pub struct MockSystemMonitor;
    impl MockSystemMonitor {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockSystemMonitor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockSystemMonitor").finish()
        }
    }
    #[async_trait]
    impl SystemMonitor for MockSystemMonitor {
        async fn cpu_usage(&self, _ctx: &CapabilityContext) -> Result<f64, MonitorError> {
            Ok(0.0)
        }
        async fn memory_info(&self, _ctx: &CapabilityContext) -> Result<MemoryInfo, MonitorError> {
            Ok(MemoryInfo {
                total: 1_000,
                used: 500,
                free: 400,
                cached: 100,
            })
        }
        async fn disk_info(
            &self,
            _ctx: &CapabilityContext,
            _path: &str,
        ) -> Result<DiskInfo, MonitorError> {
            Ok(DiskInfo {
                total: 10_000,
                used: 3_000,
                free: 7_000,
                mount_point: "/".into(),
                filesystem: "ext4".into(),
            })
        }
        async fn network_io(&self, _ctx: &CapabilityContext) -> Result<NetworkIO, MonitorError> {
            Ok(NetworkIO {
                bytes_sent: 0,
                bytes_received: 0,
                packets_sent: 0,
                packets_received: 0,
            })
        }
        async fn temperature(&self, _ctx: &CapabilityContext) -> Result<f64, MonitorError> {
            Ok(45.0)
        }
        async fn process_list(
            &self,
            _ctx: &CapabilityContext,
        ) -> Result<Vec<ProcessInfo>, MonitorError> {
            Ok(vec![])
        }
        fn events(&self) -> Receiver<OsalEvent> {
            closed_rx()
        }
    }

    pub struct MockDeviceManager;
    impl MockDeviceManager {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockDeviceManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockDeviceManager").finish()
        }
    }
    #[async_trait]
    impl DeviceManager for MockDeviceManager {
        async fn enumerate(
            &self,
            _ctx: &CapabilityContext,
        ) -> Result<Vec<DeviceInfo>, DeviceError> {
            Ok(vec![])
        }
        async fn access(
            &self,
            _ctx: &CapabilityContext,
            path: &str,
        ) -> Result<DeviceHandle, DeviceError> {
            if path.is_empty() {
                Err(DeviceError::NotFound(path.to_string()))
            } else {
                Ok(DeviceHandle {
                    path: path.to_string(),
                    fd: Fd(3),
                })
            }
        }
        fn events(&self) -> Receiver<OsalEvent> {
            closed_rx()
        }
    }

    pub struct MockUserManager;
    impl MockUserManager {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockUserManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockUserManager").finish()
        }
    }
    #[async_trait]
    impl UserManager for MockUserManager {
        async fn current_user(&self, _ctx: &CapabilityContext) -> Result<UserInfo, UserError> {
            Ok(UserInfo {
                uid: Uid(1000),
                gid: Gid(1000),
                username: "test".into(),
                home_dir: "/home/test".into(),
                shell: "/bin/bash".into(),
            })
        }
        async fn enumerate_users(
            &self,
            _ctx: &CapabilityContext,
        ) -> Result<Vec<UserInfo>, UserError> {
            Ok(vec![])
        }
        async fn enumerate_groups(
            &self,
            _ctx: &CapabilityContext,
        ) -> Result<Vec<GroupInfo>, UserError> {
            Ok(vec![])
        }
        async fn switch_user(
            &self,
            _ctx: &CapabilityContext,
            user_id: &UserId,
        ) -> Result<(), UserError> {
            if user_id.0.is_empty() {
                Err(UserError::NotFound("empty user".to_string()))
            } else {
                Ok(())
            }
        }
        async fn get_user_by_uid(
            &self,
            _ctx: &CapabilityContext,
            _uid: Uid,
        ) -> Result<UserInfo, UserError> {
            Ok(UserInfo {
                uid: Uid(1000),
                gid: Gid(1000),
                username: "test".into(),
                home_dir: "/home/test".into(),
                shell: "/bin/bash".into(),
            })
        }
        fn events(&self) -> Receiver<OsalEvent> {
            closed_rx()
        }
    }

    pub struct MockPlatformInfo;
    impl MockPlatformInfo {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockPlatformInfo {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockPlatformInfo").finish()
        }
    }
    impl PlatformInfo for MockPlatformInfo {
        fn os_name(&self) -> &str {
            "MockOS"
        }
        fn os_version(&self) -> &str {
            "0.1.0"
        }
        fn hostname(&self) -> String {
            "mockhost".into()
        }
        fn num_cpus(&self) -> usize {
            4
        }
        fn total_memory(&self) -> u64 {
            8_000_000_000
        }
        fn uptime(&self) -> Duration {
            Duration::from_secs(3600)
        }
        fn kernel_version(&self) -> &str {
            "mock-kernel-1.0"
        }
    }

    pub struct MockWindowManager;
    impl MockWindowManager {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockWindowManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockWindowManager").finish()
        }
    }
    #[async_trait]
    impl WindowManager for MockWindowManager {
        async fn list_windows(
            &self,
            _ctx: &CapabilityContext,
        ) -> Result<Vec<WindowInfo>, WindowError> {
            Ok(vec![WindowInfo {
                window_id: "1".into(),
                title: "Mock Window".into(),
                process: Some("mock_app".into()),
                geometry: Some(Rect {
                    x: 0,
                    y: 0,
                    width: 1024,
                    height: 768,
                }),
                is_visible: true,
            }])
        }
        async fn focused_window(
            &self,
            _ctx: &CapabilityContext,
        ) -> Result<Option<WindowInfo>, WindowError> {
            Ok(Some(WindowInfo {
                window_id: "1".into(),
                title: "Mock Window".into(),
                process: Some("mock_app".into()),
                geometry: Some(Rect {
                    x: 0,
                    y: 0,
                    width: 1024,
                    height: 768,
                }),
                is_visible: true,
            }))
        }
        async fn focus_window(
            &self,
            _ctx: &CapabilityContext,
            title: &str,
            _partial: bool,
        ) -> Result<(), WindowError> {
            if title.is_empty() {
                Err(WindowError::NotFound("empty title".into()))
            } else {
                Ok(())
            }
        }
        async fn close_window(
            &self,
            _ctx: &CapabilityContext,
            title: &str,
        ) -> Result<(), WindowError> {
            if title.is_empty() {
                Err(WindowError::NotFound("empty title".into()))
            } else {
                Ok(())
            }
        }
        async fn screen_dimensions(
            &self,
            _ctx: &CapabilityContext,
        ) -> Result<(u32, u32), WindowError> {
            Ok((1920, 1080))
        }
    }

    pub struct MockInputDevice;
    impl MockInputDevice {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockInputDevice {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockInputDevice").finish()
        }
    }
    #[async_trait]
    impl InputDevice for MockInputDevice {
        async fn mouse_move(
            &self,
            _ctx: &CapabilityContext,
            _x: i32,
            _y: i32,
        ) -> Result<(), InputError> {
            Ok(())
        }
        async fn mouse_click(
            &self,
            _ctx: &CapabilityContext,
            _button: &str,
        ) -> Result<(), InputError> {
            Ok(())
        }
        async fn keyboard_type(
            &self,
            _ctx: &CapabilityContext,
            _text: &str,
        ) -> Result<(), InputError> {
            Ok(())
        }
        async fn keyboard_combo(
            &self,
            _ctx: &CapabilityContext,
            _keys: &[&str],
        ) -> Result<(), InputError> {
            Ok(())
        }
        async fn cursor_position(
            &self,
            _ctx: &CapabilityContext,
        ) -> Result<(i32, i32), InputError> {
            Ok((0, 0))
        }
    }

    pub struct MockClipboardProvider;
    impl MockClipboardProvider {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockClipboardProvider {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockClipboardProvider").finish()
        }
    }
    #[async_trait]
    impl ClipboardProvider for MockClipboardProvider {
        async fn get_text(&self, _ctx: &CapabilityContext) -> Result<Option<String>, DesktopError> {
            Ok(Some("mock clipboard content".into()))
        }
        async fn set_text(
            &self,
            _ctx: &CapabilityContext,
            _text: &str,
        ) -> Result<(), DesktopError> {
            Ok(())
        }
    }

    pub struct MockDesktopProvider;
    impl MockDesktopProvider {
        pub fn new() -> Self {
            Self
        }
    }
    impl std::fmt::Debug for MockDesktopProvider {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockDesktopProvider").finish()
        }
    }
    #[async_trait]
    impl DesktopProvider for MockDesktopProvider {
        async fn open_url(&self, _ctx: &CapabilityContext, _url: &str) -> Result<(), DesktopError> {
            Ok(())
        }
        async fn launch_app(
            &self,
            _ctx: &CapabilityContext,
            _app: &str,
        ) -> Result<(), DesktopError> {
            Ok(())
        }
        async fn take_screenshot(
            &self,
            _ctx: &CapabilityContext,
            _path: &str,
        ) -> Result<(), DesktopError> {
            Ok(())
        }
        fn display_server(&self) -> Result<String, DesktopError> {
            Ok("mock".into())
        }
    }

    /// Build a fully-wired mock `KernelFacade` for testing.
    pub fn mock_kernel() -> KernelFacade {
        KernelFacade {
            filesystem: Arc::new(MockFileSystem::new()),
            process: Arc::new(MockProcessManager::new()),
            terminal: Arc::new(MockTerminal::new()),
            network: Arc::new(MockNetworkManager::new()),
            monitoring: Arc::new(MockSystemMonitor::new()),
            devices: Arc::new(MockDeviceManager::new()),
            users: Arc::new(MockUserManager::new()),
            platform: Arc::new(MockPlatformInfo::new()),
            windows: Arc::new(MockWindowManager::new()),
            input: Arc::new(MockInputDevice::new()),
            clipboard: Arc::new(MockClipboardProvider::new()),
            desktop: Arc::new(MockDesktopProvider::new()),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_mock_filesystem_read_empty_path_returns_error() {
            let fs = MockFileSystem::new();
            let ctx = dummy_ctx();
            let result = fs.read(&ctx, "").await;
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), FilesystemError::NotFound(_)));
        }

        #[tokio::test]
        async fn test_mock_filesystem_read_valid_path_returns_ok() {
            let fs = MockFileSystem::new();
            let ctx = dummy_ctx();
            let result = fs.read(&ctx, "/tmp/test.txt").await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_mock_filesystem_delete_empty_path_returns_error() {
            let fs = MockFileSystem::new();
            let ctx = dummy_ctx();
            let result = fs.delete(&ctx, "").await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_mock_filesystem_metadata_empty_path_returns_error() {
            let fs = MockFileSystem::new();
            let ctx = dummy_ctx();
            let result = fs.metadata(&ctx, "").await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_mock_filesystem_metadata_valid_path_returns_valid() {
            let fs = MockFileSystem::new();
            let ctx = dummy_ctx();
            let meta = fs.metadata(&ctx, "/tmp/foo.txt").await.unwrap();
            assert_eq!(meta.path, "/tmp/foo.txt");
        }

        #[tokio::test]
        async fn test_mock_process_spawn_empty_command_returns_error() {
            let pm = MockProcessManager::new();
            let ctx = dummy_ctx();
            let result = pm.spawn(&ctx, "", &[]).await;
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                ProcessError::ExecutionFailed(_)
            ));
        }

        #[tokio::test]
        async fn test_mock_process_spawn_valid_returns_handle() {
            let pm = MockProcessManager::new();
            let ctx = dummy_ctx();
            let handle = pm.spawn(&ctx, "ls", &["-la"]).await.unwrap();
            assert!(handle.pid.0 > 0);
        }

        #[tokio::test]
        async fn test_mock_process_kill_pid_zero_returns_error() {
            let pm = MockProcessManager::new();
            let ctx = dummy_ctx();
            let result = pm.kill(&ctx, Pid(0), Signal(9)).await;
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), ProcessError::NotFound(_)));
        }

        #[tokio::test]
        async fn test_mock_process_wait_pid_zero_returns_error() {
            let pm = MockProcessManager::new();
            let ctx = dummy_ctx();
            let result = pm.wait(&ctx, Pid(0)).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_mock_process_events_returns_closed_receiver() {
            let pm = MockProcessManager::new();
            let mut rx = pm.events();
            let result = rx.recv().await;
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn test_mock_terminal_open_pty_returns_handle() {
            let term = MockTerminal::new();
            let ctx = dummy_ctx();
            let handle = term.open_pty(&ctx).await.unwrap();
            assert_eq!(handle.id, "mock-pty");
        }

        #[tokio::test]
        async fn test_mock_network_dns_lookup_empty_returns_error() {
            let net = MockNetworkManager::new();
            let ctx = dummy_ctx();
            let result = net.dns_lookup(&ctx, "").await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_mock_network_dns_lookup_valid_returns_ip() {
            let net = MockNetworkManager::new();
            let ctx = dummy_ctx();
            let ip = net.dns_lookup(&ctx, "localhost").await.unwrap();
            assert_eq!(ip, "127.0.0.1".parse::<IpAddr>().unwrap());
        }

        #[tokio::test]
        async fn test_mock_monitor_cpu_usage_returns_zero() {
            let mon = MockSystemMonitor::new();
            let ctx = dummy_ctx();
            let cpu = mon.cpu_usage(&ctx).await.unwrap();
            assert_eq!(cpu, 0.0);
        }

        #[tokio::test]
        async fn test_mock_monitor_memory_info_returns_valid() {
            let mon = MockSystemMonitor::new();
            let ctx = dummy_ctx();
            let mem = mon.memory_info(&ctx).await.unwrap();
            assert_eq!(mem.total, 1_000);
            assert_eq!(mem.used, 500);
            assert_eq!(mem.free, 400);
        }

        #[tokio::test]
        async fn test_mock_device_access_empty_path_returns_error() {
            let dev = MockDeviceManager::new();
            let ctx = dummy_ctx();
            let result = dev.access(&ctx, "").await;
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), DeviceError::NotFound(_)));
        }

        #[tokio::test]
        async fn test_mock_device_access_valid_path_returns_handle() {
            let dev = MockDeviceManager::new();
            let ctx = dummy_ctx();
            let handle = dev.access(&ctx, "/dev/null").await.unwrap();
            assert_eq!(handle.path, "/dev/null");
            assert_eq!(handle.fd.0, 3);
        }

        #[tokio::test]
        async fn test_mock_user_current_user_returns_default() {
            let users = MockUserManager::new();
            let ctx = dummy_ctx();
            let info = users.current_user(&ctx).await.unwrap();
            assert_eq!(info.username, "test");
            assert_eq!(info.uid.0, 1000);
        }

        #[tokio::test]
        async fn test_mock_user_switch_user_empty_returns_error() {
            let users = MockUserManager::new();
            let ctx = dummy_ctx();
            let result = users.switch_user(&ctx, &UserId("".into())).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_mock_user_get_user_by_uid_returns_default() {
            let users = MockUserManager::new();
            let ctx = dummy_ctx();
            let info = users.get_user_by_uid(&ctx, Uid(1000)).await.unwrap();
            assert_eq!(info.username, "test");
        }

        #[test]
        fn test_mock_platform_info_returns_values() {
            let pi = MockPlatformInfo::new();
            assert_eq!(pi.os_name(), "MockOS");
            assert_eq!(pi.os_version(), "0.1.0");
            assert_eq!(pi.hostname(), "mockhost");
            assert_eq!(pi.num_cpus(), 4);
            assert_eq!(pi.total_memory(), 8_000_000_000);
            assert_eq!(pi.uptime(), Duration::from_secs(3600));
            assert_eq!(pi.kernel_version(), "mock-kernel-1.0");
        }

        #[test]
        fn test_mock_kernel_facade_constructs() {
            let k = mock_kernel();
            assert_eq!(k.platform.os_name(), "MockOS");
        }

        #[tokio::test]
        async fn test_mock_kernel_facade_end_to_end() {
            let k = mock_kernel();
            let ctx = dummy_ctx();

            let meta = k.filesystem.metadata(&ctx, "/etc/hostname").await.unwrap();
            assert_eq!(meta.path, "/etc/hostname");

            let handle = k
                .process
                .spawn(&ctx, "cat", &["/etc/hostname"])
                .await
                .unwrap();
            assert!(handle.pid.0 > 0);

            let wait = k.process.wait(&ctx, handle.pid).await.unwrap();
            assert_eq!(wait.exit_code(), Some(0));

            let pty = k.terminal.open_pty(&ctx).await.unwrap();
            assert_eq!(pty.id, "mock-pty");

            let ip = k.network.dns_lookup(&ctx, "localhost").await.unwrap();
            assert_eq!(ip.to_string(), "127.0.0.1");

            let cpu = k.monitoring.cpu_usage(&ctx).await.unwrap();
            assert_eq!(cpu, 0.0);

            let user = k.users.current_user(&ctx).await.unwrap();
            assert_eq!(user.username, "test");

            let dev = k.devices.access(&ctx, "/dev/null").await.unwrap();
            assert_eq!(dev.path, "/dev/null");
        }

        #[tokio::test]
        async fn test_mock_window_manager_list_windows() {
            let wm = MockWindowManager::new();
            let ctx = dummy_ctx();
            let windows = wm.list_windows(&ctx).await.unwrap();
            assert_eq!(windows.len(), 1);
            assert_eq!(windows[0].title, "Mock Window");
        }

        #[tokio::test]
        async fn test_mock_window_manager_focus_empty_title_returns_error() {
            let wm = MockWindowManager::new();
            let ctx = dummy_ctx();
            let result = wm.focus_window(&ctx, "", false).await;
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), WindowError::NotFound(_)));
        }

        #[tokio::test]
        async fn test_mock_window_manager_focus_valid_title_returns_ok() {
            let wm = MockWindowManager::new();
            let ctx = dummy_ctx();
            let result = wm.focus_window(&ctx, "Firefox", false).await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_mock_window_manager_screen_dimensions() {
            let wm = MockWindowManager::new();
            let ctx = dummy_ctx();
            let (w, h) = wm.screen_dimensions(&ctx).await.unwrap();
            assert_eq!(w, 1920);
            assert_eq!(h, 1080);
        }

        #[tokio::test]
        async fn test_mock_input_device_mouse_move() {
            let input = MockInputDevice::new();
            let ctx = dummy_ctx();
            assert!(input.mouse_move(&ctx, 100, 200).await.is_ok());
        }

        #[tokio::test]
        async fn test_mock_input_device_keyboard_type() {
            let input = MockInputDevice::new();
            let ctx = dummy_ctx();
            assert!(input.keyboard_type(&ctx, "hello").await.is_ok());
        }

        #[tokio::test]
        async fn test_mock_clipboard_get_set() {
            let cb = MockClipboardProvider::new();
            let ctx = dummy_ctx();
            let text = cb.get_text(&ctx).await.unwrap();
            assert_eq!(text, Some("mock clipboard content".into()));
            assert!(cb.set_text(&ctx, "new content").await.is_ok());
        }

        #[tokio::test]
        async fn test_mock_desktop_provider_open_url() {
            let dp = MockDesktopProvider::new();
            let ctx = dummy_ctx();
            assert!(dp.open_url(&ctx, "https://example.com").await.is_ok());
        }

        #[test]
        fn test_mock_desktop_provider_display_server() {
            let dp = MockDesktopProvider::new();
            assert_eq!(dp.display_server().unwrap(), "mock");
        }

        #[tokio::test]
        async fn test_mock_kernel_facade_desktop_end_to_end() {
            let k = mock_kernel();
            let ctx = dummy_ctx();

            let windows = k.windows.list_windows(&ctx).await.unwrap();
            assert!(!windows.is_empty());

            assert!(k.input.mouse_move(&ctx, 100, 200).await.is_ok());
            assert!(k.clipboard.set_text(&ctx, "test").await.is_ok());

            let cb_text = k.clipboard.get_text(&ctx).await.unwrap();
            assert!(cb_text.is_some());

            assert!(k.desktop.open_url(&ctx, "https://aios.dev").await.is_ok());
            assert_eq!(k.desktop.display_server().unwrap(), "mock");
        }
    }
}
