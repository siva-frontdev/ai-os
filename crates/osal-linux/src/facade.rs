use osal_core::KernelFacade;
use std::sync::Arc;

use crate::desktop::{
    LinuxClipboardProvider, LinuxDesktopProvider, LinuxInputDevice, LinuxWindowManager,
};
use crate::devices::LinuxDeviceManager;
use crate::filesystem::LinuxFileSystem;
use crate::monitoring::LinuxSystemMonitor;
use crate::network::LinuxNetworkManager;
use crate::platform::LinuxPlatformInfo;
use crate::process::LinuxProcessManager;
use crate::terminal::LinuxTerminal;
use crate::users::LinuxUserManager;

/// Linux implementation of `KernelFacade`.
///
/// Constructs all subsystem implementations and wires them into the facade.
/// This is the **single entry point** for creating a fully-wired OSAL kernel
/// on Linux. All Linux-specific code is encapsulated behind the OSAL traits.
pub struct LinuxKernelFacade;

impl LinuxKernelFacade {
    /// Build a `KernelFacade` with all subsystems backed by Linux implementations.
    pub fn new() -> KernelFacade {
        KernelFacade {
            filesystem: Arc::new(LinuxFileSystem::new()),
            process: Arc::new(LinuxProcessManager::new()),
            terminal: Arc::new(LinuxTerminal::new()),
            network: Arc::new(LinuxNetworkManager::new()),
            monitoring: Arc::new(LinuxSystemMonitor::new()),
            devices: Arc::new(LinuxDeviceManager::new()),
            users: Arc::new(LinuxUserManager::new()),
            platform: Arc::new(LinuxPlatformInfo::new()),
            windows: Arc::new(LinuxWindowManager::new()),
            input: Arc::new(LinuxInputDevice::new()),
            clipboard: Arc::new(LinuxClipboardProvider::new()),
            desktop: Arc::new(LinuxDesktopProvider::new()),
        }
    }
}
