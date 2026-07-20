#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
pub mod events;
mod facade;
mod types;

pub use error::{
    DeviceError, FilesystemError, MonitorError, NetworkError, OsalError, OsalResult, ProcessError,
    TerminalError, UserError,
};
pub use events::OsalEvent;
pub use facade::{
    ChildHandle, DeviceHandle, DeviceInfo, DeviceManager, DirEntry, DiskInfo, FileKind,
    FileMetadata, FileSystem, GroupInfo, InterfaceInfo, KernelFacade, MemoryInfo, NetworkConfig,
    NetworkIO, NetworkManager, PlatformInfo, ProcessInfo, ProcessManager, PtyHandle, SystemMonitor,
    Terminal, UserInfo, UserManager,
};
pub use types::*;

pub use facade::mock;
