//! System-wide event types emitted by OSAL subsystems.
use crate::types::{ExitStatus, Pid, UserId};
use chrono::{DateTime, Utc};

/// Every observable OS event is represented by a variant of `OsalEvent`.
///
/// Subsystems emit these through `tokio::sync::mpsc::Receiver<OsalEvent>`
/// channels obtained from their respective trait methods.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum OsalEvent {
    /// A new process has been spawned.
    ProcessStarted {
        /// Process ID of the spawned process.
        pid: Pid,
        /// Command line / executable name.
        command: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A process has exited.
    ProcessExited {
        /// Process ID of the exited process.
        pid: Pid,
        /// Exit status (code or signal).
        exit: ExitStatus,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A file or directory was created.
    FileCreated {
        /// Absolute path of the created entry.
        path: String,
        /// Type string (e.g. "file", "directory", "symlink").
        file_type: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A file was modified (size may be 0 if unknown).
    FileModified {
        /// Absolute path of the modified file.
        path: String,
        /// New size in bytes.
        size: u64,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A file or directory was deleted.
    FileDeleted {
        /// Absolute path of the deleted entry.
        path: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A network interface connected.
    NetworkConnected {
        /// Interface name (e.g. "eth0").
        interface: String,
        /// IP address that was assigned.
        address: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A network interface disconnected.
    NetworkDisconnected {
        /// Interface name (e.g. "eth0").
        interface: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A hardware device was attached.
    DeviceAttached {
        /// Unique device identifier.
        device_id: String,
        /// Device type string (e.g. "block", "usb").
        device_type: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A hardware device was removed.
    DeviceRemoved {
        /// Unique device identifier.
        device_id: String,
        /// Device type string.
        device_type: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A window was opened / created.
    WindowOpened {
        /// Window title.
        title: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A window was closed.
    WindowClosed {
        /// Window title.
        title: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A window gained focus.
    WindowFocused {
        /// Window title.
        title: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// Clipboard content changed.
    ClipboardChanged {
        /// MIME type or format description of the clipboard content.
        content_type: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// Screen resolution or arrangement changed.
    ScreenChanged {
        /// Resolution string (e.g. "1920x1080").
        resolution: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A system-wide resource usage threshold was crossed.
    SystemResourceWarning {
        /// The resource that triggered the warning (e.g. "cpu", "memory").
        resource: String,
        /// The current value of the resource metric.
        value: f64,
        /// The threshold that was crossed.
        threshold: f64,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A user logged in.
    UserLoggedIn {
        /// Identifier of the user who logged in.
        user_id: UserId,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A user logged out.
    UserLoggedOut {
        /// Identifier of the user who logged out.
        user_id: UserId,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// Output data from a process terminal.
    TerminalOutput {
        /// Process ID that produced the output.
        pid: Pid,
        /// Raw byte data from the terminal.
        data: Vec<u8>,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
    /// A system service changed state.
    ServiceStateChanged {
        /// Name of the service.
        service: String,
        /// New state (e.g. "running", "stopped", "failed").
        state: String,
        /// When the event occurred.
        timestamp: DateTime<Utc>,
    },
}
