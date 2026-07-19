#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OSAL device subsystem — rich device categorisation and no-op defaults.
//!
//! This crate re-exports the core device types ([`DeviceManager`],
//! [`DeviceError`], [`DeviceInfo`], [`DeviceHandle`]) from [`osal_core`]
//! and adds richer categorisation enums ([`DeviceType`], [`InputSubType`],
//! [`DeviceStatus`]) that are not part of the core abstraction.
//!
//! A [`DefaultDeviceManager`] no-op implementation is provided for testing
//! and for scaffolding before a real OSAL backend is wired in.

mod types;

pub use types::{DeviceType, InputSubType, DeviceStatus};
pub use osal_core::{DeviceManager, DeviceError, DeviceInfo, DeviceHandle};

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use osal_core::{DeviceError as DevErr, DeviceHandle as DevHnd, DeviceInfo as DevInfo};
use tokio::sync::mpsc;

/// No-op [`DeviceManager`] that returns empty results and errors.
///
/// Every method returns either an empty vector, a closed receiver, or a
/// "not found" / "not supported" error so that callers can exercise
/// error paths during development without a real OSAL backend.
#[derive(Debug)]
pub struct DefaultDeviceManager;

#[async_trait]
impl DeviceManager for DefaultDeviceManager {
    async fn enumerate(&self, _ctx: &CapabilityContext) -> Result<Vec<DevInfo>, DevErr> {
        Ok(Vec::new())
    }

    async fn access(&self, _ctx: &CapabilityContext, path: &str) -> Result<DevHnd, DevErr> {
        Err(DevErr::NotFound(path.to_string()))
    }

    fn events(&self) -> mpsc::Receiver<osal_core::OsalEvent> {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osal_core::{DeviceError, DeviceManager};
    use tokio::sync::mpsc;

    fn dummy_ctx() -> CapabilityContext {
        CapabilityContext::new("test")
    }

    #[tokio::test]
    async fn test_default_enumerate_returns_empty() {
        let mgr = DefaultDeviceManager;
        let devices = mgr.enumerate(&dummy_ctx()).await.unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn test_default_access_returns_not_found() {
        let mgr = DefaultDeviceManager;
        let err = mgr.access(&dummy_ctx(), "/dev/null").await.unwrap_err();
        assert!(matches!(err, DeviceError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_default_access_with_empty_path() {
        let mgr = DefaultDeviceManager;
        let err = mgr.access(&dummy_ctx(), "").await.unwrap_err();
        assert!(matches!(err, DeviceError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_default_events_channel_is_closed() {
        let mgr = DefaultDeviceManager;
        let mut rx: mpsc::Receiver<osal_core::OsalEvent> = mgr.events();
        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn test_device_manager_trait_is_object_safe() {
        let mgr: &dyn DeviceManager = &DefaultDeviceManager;
        let devices = mgr.enumerate(&dummy_ctx()).await.unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn test_device_type_serde_roundtrip() {
        let dt = DeviceType::Input(InputSubType::Keyboard);
        let json = serde_json::to_string(&dt).unwrap();
        let back: DeviceType = serde_json::from_str(&json).unwrap();
        assert_eq!(dt, back);
    }

    #[test]
    fn test_device_status_serde_roundtrip() {
        let st = DeviceStatus::Connected;
        let json = serde_json::to_string(&st).unwrap();
        let back: DeviceStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(st, back);
    }

    #[test]
    fn test_device_type_display() {
        assert_eq!(format!("{:?}", DeviceType::Camera), "Camera");
        assert_eq!(format!("{:?}", DeviceType::Input(InputSubType::Mouse)), "Input(Mouse)");
    }

    #[test]
    fn test_input_sub_type_variants() {
        assert_eq!(InputSubType::Keyboard as u8, 0);
        assert_eq!(InputSubType::Mouse as u8, 1);
        assert_eq!(InputSubType::Touchpad as u8, 2);
    }

    #[test]
    fn test_default_device_manager_debug() {
        let mgr = DefaultDeviceManager;
        let s = format!("{:?}", mgr);
        assert_eq!(s, "DefaultDeviceManager");
    }
}
