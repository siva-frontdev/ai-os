# osal-devices — Hardware Device Subsystem Trait Definitions

## Architecture

The `DeviceManager` trait provides an OS-agnostic interface for enumerating,
querying, accessing, and releasing hardware devices. Device events (attach,
remove, state change) are delivered through a channel obtained from `watch()`.

```
┌─────────────────────────────────────────────────────┐
│                    DeviceManager                      │
│  ┌──────────┐ ┌──────────┐ ┌────────┐ ┌───────────┐│
│  │enumerate │ │enumerate │ │  info  │ │access/    ││
│  │(all)     │ │by type   │ │(by ID) │ │release    ││
│  └──────────┘ └──────────┘ └────────┘ └───────────┘│
│  ┌──────────────────────────────────────────────────┐│
│  │              watch() → mpsc::Receiver             ││
│  │         (DeviceEvent stream)                     ││
│  └──────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────┘
```

Capability checks are performed on every `enumerate()` and `access()` call.
The `info()` and `release()` methods typically require lower privilege and may
not need a context parameter.

---

## Trait: `DeviceManager`

```rust
use async_trait::async_trait;
use std::fmt::Debug;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use osal_core::{OsalResult, Fd};
use osal_capabilities::CapabilityContext;

#[async_trait]
pub trait DeviceManager: Debug + Send + Sync {
    /// Enumerate all hardware devices on the system.
    async fn enumerate(&self, ctx: &CapabilityContext) -> OsalResult<Vec<DeviceInfo>>;

    /// Enumerate devices matching a specific type.
    async fn enumerate_by_type(
        &self,
        device_type: DeviceType,
        ctx: &CapabilityContext,
    ) -> OsalResult<Vec<DeviceInfo>>;

    /// Get detailed information about a specific device by ID.
    async fn info(&self, device_id: &str) -> OsalResult<DeviceInfo>;

    /// Open a handle to a device for I/O operations.
    async fn access(&self, device_id: &str, ctx: &CapabilityContext) -> OsalResult<DeviceHandle>;

    /// Release (close) a previously opened device handle.
    async fn release(&self, device_id: &str) -> OsalResult<()>;

    /// Subscribe to a channel that receives device events.
    async fn watch(&self) -> mpsc::Receiver<DeviceEvent>;
}
```

---

## Data Types

### `DeviceType`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceType {
    Camera,
    Audio,
    Gpu,
    Usb,
    Storage,
    Network,
    Input,
    Display,
    Bluetooth,
    Unclassified,
}
```

### `InputSubType`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputSubType {
    Keyboard,
    Mouse,
    Touchpad,
}
```

### `DeviceStatus`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceStatus {
    Connected,
    Disconnected,
    Error,
    Unknown,
}
```

### `DeviceInfo`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_type: DeviceType,
    pub vendor_id: Option<String>,
    pub product_id: Option<String>,
    pub serial: Option<String>,
    pub driver: Option<String>,
    pub description: String,
    pub status: DeviceStatus,
    pub properties: HashMap<String, String>,
    pub location: Option<String>,
    pub major: u32,
    pub minor: u32,
}
```

### `DeviceHandle`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceHandle {
    pub device_id: String,
    pub fd: Fd,
    pub opened_at: DateTime<Utc>,
}
```

---

## Event: `DeviceEvent`

```rust
#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Attached {
        device_id: String,
        device_type: DeviceType,
        timestamp: DateTime<Utc>,
    },
    Removed {
        device_id: String,
        device_type: DeviceType,
        timestamp: DateTime<Utc>,
    },
    StateChanged {
        device_id: String,
        old_status: DeviceStatus,
        new_status: DeviceStatus,
        timestamp: DateTime<Utc>,
    },
    Error {
        device_id: String,
        error: String,
        timestamp: DateTime<Utc>,
    },
}
```

---

## Error: `DeviceError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("device not found: {0}")]
    NotFound(String),

    #[error("access denied to device: {0}")]
    AccessDenied(String),

    #[error("device is busy: {0}")]
    Busy(String),

    #[error("operation not supported by device: {0}")]
    NotSupported(String),

    #[error("driver error: {0}")]
    DriverError(String),

    #[error("I/O error: {0}")]
    IoError(String),
}
```
