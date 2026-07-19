//! The `Capability` enum and supporting types.
use serde::{Deserialize, Serialize};

/// Process ID — lightweight newtype used for capability targeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pid(pub u64);

impl std::fmt::Display for Pid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A unit of authorization checked before every privileged OSAL operation.
///
/// The `Admin` variant grants **all** capabilities without exception.
/// Path-based variants (e.g. `FileRead(String)`) accept a concrete path
/// or `"*"` to match any path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    // -----------------------------------------------------------------------
    // Filesystem
    // -----------------------------------------------------------------------

    /// Read a file at the given path (`"*"` = any path).
    FileRead(String),
    /// Write to a file at the given path (`"*"` = any path).
    FileWrite(String),
    /// Execute a file at the given path (`"*"` = any path).
    FileExecute(String),
    /// Delete a file at the given path (`"*"` = any path).
    FileDelete(String),
    /// Watch a file or directory for changes (`"*"` = any path).
    FileWatch(String),
    /// Read metadata of a file at the given path (`"*"` = any path).
    FileMetadata(String),

    // -----------------------------------------------------------------------
    // Process
    // -----------------------------------------------------------------------

    /// Spawn new processes.
    ProcessSpawn,
    /// Kill (send a signal to) a specific process.
    ProcessKill(Pid),
    /// Suspend a specific process.
    ProcessSuspend(Pid),
    /// Resume a suspended process.
    ProcessResume(Pid),
    /// List / enumerate running processes.
    ProcessEnumerate,

    // -----------------------------------------------------------------------
    // Terminal
    // -----------------------------------------------------------------------

    /// Execute commands in a terminal.
    TerminalExecute,
    /// Allocate and use a pseudo-terminal.
    TerminalPTY,
    /// Send a signal to a process via the terminal.
    TerminalSignal(Pid),

    // -----------------------------------------------------------------------
    // Network
    // -----------------------------------------------------------------------

    /// Configure network interfaces.
    NetworkConfigure,
    /// Open network sockets.
    NetworkSocket,
    /// Perform DNS lookups.
    NetworkDns,
    /// Modify firewall rules.
    NetworkFirewall,

    // -----------------------------------------------------------------------
    // Monitoring
    // -----------------------------------------------------------------------

    /// Read CPU usage statistics.
    MonitorCpu,
    /// Read memory usage statistics.
    MonitorMemory,
    /// Read disk usage statistics.
    MonitorDisk,
    /// Read network I/O statistics.
    MonitorNetwork,
    /// Read temperature sensors.
    MonitorTemperature,
    /// Enumerate and inspect running processes.
    MonitorProcesses,

    // -----------------------------------------------------------------------
    // Devices
    // -----------------------------------------------------------------------

    /// Enumerate available hardware devices.
    DeviceEnumerate,
    /// Access a hardware device at the given path (`"*"` = any device).
    DeviceAccess(String),

    // -----------------------------------------------------------------------
    // Clipboard
    // -----------------------------------------------------------------------

    /// Read from the system clipboard.
    ClipboardRead,
    /// Write to the system clipboard.
    ClipboardWrite,

    // -----------------------------------------------------------------------
    // Display / Window
    // -----------------------------------------------------------------------

    /// List open windows.
    WindowList,
    /// Focus or manage window focus.
    WindowFocus,
    /// Capture screen content.
    ScreenCapture,

    // -----------------------------------------------------------------------
    // Users
    // -----------------------------------------------------------------------

    /// Enumerate system users.
    UserEnumerate,
    /// Switch the current user context.
    UserSwitch,
    /// Enumerate system groups.
    GroupEnumerate,

    // -----------------------------------------------------------------------
    // System
    // -----------------------------------------------------------------------

    /// Shut down the system.
    SystemShutdown,
    /// Reboot the system.
    SystemReboot,
    /// Put the system to sleep (suspend-to-RAM).
    SystemSleep,
    /// Hibernate the system (suspend-to-disk).
    SystemHibernate,

    // -----------------------------------------------------------------------
    // Admin — universal grant
    // -----------------------------------------------------------------------

    /// Grants every capability. Bypasses all individual checks.
    Admin,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capability::FileRead(p) => write!(f, "FileRead({})", p),
            Capability::FileWrite(p) => write!(f, "FileWrite({})", p),
            Capability::FileExecute(p) => write!(f, "FileExecute({})", p),
            Capability::FileDelete(p) => write!(f, "FileDelete({})", p),
            Capability::FileWatch(p) => write!(f, "FileWatch({})", p),
            Capability::FileMetadata(p) => write!(f, "FileMetadata({})", p),
            Capability::ProcessSpawn => write!(f, "ProcessSpawn"),
            Capability::ProcessKill(p) => write!(f, "ProcessKill({})", p),
            Capability::ProcessSuspend(p) => write!(f, "ProcessSuspend({})", p),
            Capability::ProcessResume(p) => write!(f, "ProcessResume({})", p),
            Capability::ProcessEnumerate => write!(f, "ProcessEnumerate"),
            Capability::TerminalExecute => write!(f, "TerminalExecute"),
            Capability::TerminalPTY => write!(f, "TerminalPTY"),
            Capability::TerminalSignal(p) => write!(f, "TerminalSignal({})", p),
            Capability::NetworkConfigure => write!(f, "NetworkConfigure"),
            Capability::NetworkSocket => write!(f, "NetworkSocket"),
            Capability::NetworkDns => write!(f, "NetworkDns"),
            Capability::NetworkFirewall => write!(f, "NetworkFirewall"),
            Capability::MonitorCpu => write!(f, "MonitorCpu"),
            Capability::MonitorMemory => write!(f, "MonitorMemory"),
            Capability::MonitorDisk => write!(f, "MonitorDisk"),
            Capability::MonitorNetwork => write!(f, "MonitorNetwork"),
            Capability::MonitorTemperature => write!(f, "MonitorTemperature"),
            Capability::MonitorProcesses => write!(f, "MonitorProcesses"),
            Capability::DeviceEnumerate => write!(f, "DeviceEnumerate"),
            Capability::DeviceAccess(p) => write!(f, "DeviceAccess({})", p),
            Capability::ClipboardRead => write!(f, "ClipboardRead"),
            Capability::ClipboardWrite => write!(f, "ClipboardWrite"),
            Capability::WindowList => write!(f, "WindowList"),
            Capability::WindowFocus => write!(f, "WindowFocus"),
            Capability::ScreenCapture => write!(f, "ScreenCapture"),
            Capability::UserEnumerate => write!(f, "UserEnumerate"),
            Capability::UserSwitch => write!(f, "UserSwitch"),
            Capability::GroupEnumerate => write!(f, "GroupEnumerate"),
            Capability::SystemShutdown => write!(f, "SystemShutdown"),
            Capability::SystemReboot => write!(f, "SystemReboot"),
            Capability::SystemSleep => write!(f, "SystemSleep"),
            Capability::SystemHibernate => write!(f, "SystemHibernate"),
            Capability::Admin => write!(f, "Admin"),
        }
    }
}
