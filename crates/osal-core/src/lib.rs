#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
pub mod events;
mod facade;
mod types;

pub use error::{
    DesktopError, DeviceError, FilesystemError, InputError, MonitorError, NetworkError, OsalError,
    OsalResult, ProcessError, TerminalError, UserError, WindowError,
};
pub use events::OsalEvent;
pub use facade::{
    ChildHandle, ClipboardProvider, DesktopProvider, DeviceHandle, DeviceInfo, DeviceManager,
    DirEntry, DiskInfo, FileKind, FileMetadata, FileSystem, GroupInfo, InputDevice, InterfaceInfo,
    KernelFacade, MemoryInfo, NetworkConfig, NetworkIO, NetworkManager, PlatformInfo, ProcessInfo,
    ProcessManager, PtyHandle, Rect, SystemMonitor, Terminal, UserInfo, UserManager, WindowInfo,
    WindowManager,
};
pub use types::*;

pub use facade::mock;
