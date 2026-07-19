use serde::{Deserialize, Serialize};

/// Categories of hardware devices.
///
/// This is a richer categorisation than the flat string used in
/// [`osal_core::DeviceInfo::device_type`].  Use it when you need
/// structured matching rather than string comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceType {
    /// Video/image capture device.
    Camera,
    /// Audio input/output device.
    Audio,
    /// Graphics processing unit.
    Gpu,
    /// USB bus device.
    Usb,
    /// Block storage device (disk, SSD, NVMe).
    Storage,
    /// Network interface controller.
    Network,
    /// Human input device.
    Input(InputSubType),
    /// Display/output device (monitor, framebuffer).
    Display,
    /// Bluetooth adapter or peripheral.
    Bluetooth,
    /// Device type could not be determined.
    Unclassified,
}

/// Sub-category for input devices.
///
/// Used as a payload for [`DeviceType::Input`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputSubType {
    /// Keyboard.
    Keyboard,
    /// Pointing device (mouse, trackball).
    Mouse,
    /// Touchpad / trackpad.
    Touchpad,
}

/// Operational status of a hardware device.
///
/// Mirrors common udev / sysfs states and is broader than the
/// error/no-error dichotomy in [`osal_core::DeviceError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceStatus {
    /// Device is connected and operational.
    Connected,
    /// Device is disconnected or unplugged.
    Disconnected,
    /// Device is in an error state.
    Error,
    /// Device status is not known.
    Unknown,
}
