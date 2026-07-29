use crate::types::{Pid, Signal};
use osal_capabilities::Capability;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FilesystemError {
    #[error("path not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("already exists: {0}")]
    AlreadyExists(String),
    #[error("not a directory: {0}")]
    NotADirectory(String),
    #[error("is a directory: {0}")]
    IsADirectory(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("storage full")]
    StorageFull,
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProcessError {
    #[error("process not found: {0}")]
    NotFound(Pid),
    #[error("process already exists: {0}")]
    AlreadyExists(Pid),
    #[error("operation not allowed: {0}")]
    NotAllowed(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("invalid signal: {0}")]
    InvalidSignal(Signal),
    #[error("I/O error: {0}")]
    Io(String),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TerminalError {
    #[error("terminal not found: {0}")]
    NotFound(String),
    #[error("terminal not available: {0}")]
    NotAvailable(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("invalid signal: {0}")]
    InvalidSignal(Signal),
    #[error("disconnected")]
    Disconnected,
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum NetworkError {
    #[error("interface not found: {0}")]
    InterfaceNotFound(String),
    #[error("configuration failed: {0}")]
    ConfigurationFailed(String),
    #[error("DNS lookup failed: {0}")]
    DnsFailed(String),
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MonitorError {
    #[error("monitoring not available: {0}")]
    NotAvailable(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DeviceError {
    #[error("device not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("device busy: {0}")]
    Busy(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UserError {
    #[error("user not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("user switch failed: {0}")]
    SwitchFailed(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WindowError {
    #[error("window not found: {0}")]
    NotFound(String),
    #[error("window manager not available: {0}")]
    NotAvailable(String),
    #[error("screen capture failed: {0}")]
    CaptureFailed(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum InputError {
    #[error("input device not available: {0}")]
    NotAvailable(String),
    #[error("no display server available")]
    NoDisplayServer,
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DesktopError {
    #[error("operation not available: {0}")]
    NotAvailable(String),
    #[error("no display server available")]
    NoDisplayServer,
    #[error("launch failed: {0}")]
    LaunchFailed(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OsalError {
    #[error("filesystem error: {0}")]
    Filesystem(#[from] FilesystemError),
    #[error("process error: {0}")]
    Process(#[from] ProcessError),
    #[error("terminal error: {0}")]
    Terminal(#[from] TerminalError),
    #[error("network error: {0}")]
    Network(#[from] NetworkError),
    #[error("device error: {0}")]
    Device(#[from] DeviceError),
    #[error("monitor error: {0}")]
    Monitor(#[from] MonitorError),
    #[error("user error: {0}")]
    User(#[from] UserError),
    #[error("window error: {0}")]
    Window(#[from] WindowError),
    #[error("input error: {0}")]
    Input(#[from] InputError),
    #[error("desktop error: {0}")]
    Desktop(#[from] DesktopError),
    #[error("capability denied: {0}")]
    CapabilityDenied(Capability),
    #[error("operation timed out: {operation} after {duration:?}")]
    Timeout {
        operation: String,
        duration: Duration,
    },
    #[error("operation interrupted: {operation} by signal {signal:?}")]
    Interrupted { operation: String, signal: Signal },
    #[error("resource exhausted: {resource} (limit {limit}, requested {requested})")]
    ResourceExhausted {
        resource: String,
        limit: u64,
        requested: u64,
    },
    #[error("not supported: {operation} on {platform}")]
    NotSupported { operation: String, platform: String },
    #[error("permission denied: {context}")]
    PermissionDenied { context: String },
    #[error("I/O error: {0}")]
    IoError(String),
}

pub type OsalResult<T> = Result<T, OsalError>;
