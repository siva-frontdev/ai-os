use std::fmt;
use std::os::unix::io::IntoRawFd;
use async_trait::async_trait;
use tokio::sync::mpsc::{self, Receiver};
use osal_capabilities::CapabilityContext;
use osal_core::{
    DeviceManager, DeviceInfo, DeviceHandle, DeviceError, OsalEvent,
};

pub struct LinuxDeviceManager;

impl LinuxDeviceManager {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for LinuxDeviceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxDeviceManager").finish()
    }
}

fn closed_rx() -> Receiver<OsalEvent> {
    let (tx, rx) = mpsc::channel(1);
    drop(tx);
    rx
}

fn read_uevent(path: &std::path::Path) -> Option<DeviceInfo> {
    let uevent_path = path.join("uevent");
    let content = std::fs::read_to_string(&uevent_path).ok()?;
    let mut modalias = String::new();
    let mut driver = None;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("MODALIAS=") {
            modalias = val.to_string();
        } else if let Some(val) = line.strip_prefix("DRIVER=") {
            let v = val.trim();
            if !v.is_empty() {
                driver = Some(v.to_string());
            }
        }
    }
    let device_type = if modalias.contains("pci") {
        "pci"
    } else if modalias.contains("usb") {
        "usb"
    } else if modalias.contains("platform") {
        "platform"
    } else {
        "unknown"
    };
    let id = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Some(DeviceInfo {
        id,
        device_type: device_type.to_string(),
        driver,
        description: modalias,
    })
}

fn enumerate_class_devices() -> Vec<DeviceInfo> {
    let mut devices = Vec::new();
    let class_dir = std::path::Path::new("/sys/class");
    let entries = match std::fs::read_dir(class_dir) {
        Ok(e) => e,
        Err(_) => return devices,
    };
    for entry in entries.flatten() {
        let _class_name = entry.file_name();
        let class_path = entry.path();
        let sub_entries = match std::fs::read_dir(&class_path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for sub in sub_entries.flatten() {
            let dev_path = sub.path();
            if let Some(info) = read_uevent(&dev_path) {
                devices.push(info);
            }
        }
    }
    devices
}

#[async_trait]
impl DeviceManager for LinuxDeviceManager {
    async fn enumerate(&self, _ctx: &CapabilityContext) -> Result<Vec<DeviceInfo>, DeviceError> {
        Ok(enumerate_class_devices())
    }

    async fn access(&self, _ctx: &CapabilityContext, path: &str) -> Result<DeviceHandle, DeviceError> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| DeviceError::Io(e.to_string()))?;
        let fd = file.into_std().await.into_raw_fd();
        Ok(DeviceHandle {
            path: path.to_string(),
            fd: osal_core::Fd(fd as u64),
        })
    }

    fn events(&self) -> Receiver<OsalEvent> {
        closed_rx()
    }
}
