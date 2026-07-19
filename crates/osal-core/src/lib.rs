#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
pub mod events;
mod types;
mod facade;

pub use error::{
    FilesystemError, ProcessError, TerminalError, NetworkError,
    MonitorError, DeviceError, UserError, OsalError, OsalResult,
};
pub use events::OsalEvent;
pub use types::*;
pub use facade::{
    KernelFacade, FileSystem, ProcessManager, Terminal, NetworkManager,
    SystemMonitor, DeviceManager, UserManager, PlatformInfo,
    FileMetadata, FileKind, DirEntry, ChildHandle, ProcessInfo,
    PtyHandle, InterfaceInfo, NetworkConfig, MemoryInfo, DiskInfo,
    NetworkIO, DeviceInfo, DeviceHandle, UserInfo, GroupInfo,
};

#[cfg(test)]
pub use facade::mock;
