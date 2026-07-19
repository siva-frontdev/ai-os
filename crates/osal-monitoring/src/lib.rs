#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OSAL monitoring subsystem — data types and default implementation.
//!
//! This crate re-exports the [`SystemMonitor`] trait and associated types
//! from [`osal_core`], and provides additional detailed snapshot types
//! for CPU, memory, disk, network I/O, temperature, system load, and
//! OS information. It also provides a no-op [`DefaultSystemMonitor`]
//! implementation that can be used as a placeholder.
//!
//! # Re-exports from osal-core
//!
//! | Item | Description |
//! |---|---|
//! | [`SystemMonitor`] | Trait for OS resource monitoring |
//! | [`MonitorError`] | Monitoring error variants |
//! | [`MemoryInfo`] | Simple memory usage snapshot |
//! | [`DiskInfo`] | Simple disk usage for a mount point |
//! | [`NetworkIO`] | Simple network I/O counters |
//! | [`ProcessInfo`] | Snapshot of a running process |
//!
//! # Local types
//!
//! | Type | Description |
//! |---|---|
//! | [`CpuSnapshot`] | Detailed CPU usage breakdown |
//! | [`CpuCore`] | Per-core CPU data |
//! | [`MemorySnapshot`] | Detailed memory with swap info |
//! | [`DiskSnapshot`] | Detailed disk usage with IOPS |
//! | [`NetworkIoSnapshot`] | Per-interface network statistics |
//! | [`TemperatureSnapshot`] | Temperature sensor reading |
//! | [`SystemLoad`] | Load averages and process counts |
//! | [`OsInfo`] | OS identification information |
//! | [`ResourceType`] | Resource category enum |
//! | [`ThresholdOperator`] | Comparison operator for thresholds |
//! | [`ResourceThreshold`] | Threshold configuration |

mod types;

pub use types::{
    CpuCore, CpuSnapshot, DiskSnapshot, MemorySnapshot, NetworkIoSnapshot, OsInfo,
    ResourceThreshold, ResourceType, SystemLoad, TemperatureSnapshot, ThresholdOperator,
};

pub use osal_core::{
    DiskInfo, MemoryInfo, MonitorError, NetworkIO, OsalEvent, ProcessInfo, SystemMonitor,
};

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use tokio::sync::mpsc;

/// A no-op implementation of [`SystemMonitor`] that returns errors for all
/// operations.
///
/// Every fallible method returns [`MonitorError::NotAvailable`] with the
/// method name as context. The `events()` method returns a closed receiver.
///
/// This is useful as a placeholder or default when no real monitoring
/// backend is configured.
#[derive(Debug, Default)]
pub struct DefaultSystemMonitor;

#[async_trait]
impl osal_core::SystemMonitor for DefaultSystemMonitor {
    async fn cpu_usage(&self, _ctx: &CapabilityContext) -> Result<f64, MonitorError> {
        Err(MonitorError::NotAvailable("cpu_usage".into()))
    }

    async fn memory_info(&self, _ctx: &CapabilityContext) -> Result<MemoryInfo, MonitorError> {
        Err(MonitorError::NotAvailable("memory_info".into()))
    }

    async fn disk_info(
        &self,
        _ctx: &CapabilityContext,
        _path: &str,
    ) -> Result<DiskInfo, MonitorError> {
        Err(MonitorError::NotAvailable("disk_info".into()))
    }

    async fn network_io(&self, _ctx: &CapabilityContext) -> Result<NetworkIO, MonitorError> {
        Err(MonitorError::NotAvailable("network_io".into()))
    }

    async fn temperature(&self, _ctx: &CapabilityContext) -> Result<f64, MonitorError> {
        Err(MonitorError::NotAvailable("temperature".into()))
    }

    async fn process_list(
        &self,
        _ctx: &CapabilityContext,
    ) -> Result<Vec<ProcessInfo>, MonitorError> {
        Err(MonitorError::NotAvailable("process_list".into()))
    }

    fn events(&self) -> mpsc::Receiver<OsalEvent> {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        rx
    }
}

impl DefaultSystemMonitor {
    /// Create a new `DefaultSystemMonitor`.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osal_core::SystemMonitor;

    fn dummy_ctx() -> CapabilityContext {
        CapabilityContext::new("test")
    }

    #[tokio::test]
    async fn test_default_cpu_usage_returns_not_available() {
        let mon = DefaultSystemMonitor::new();
        let ctx = dummy_ctx();
        let result = mon.cpu_usage(&ctx).await;
        assert!(matches!(result, Err(MonitorError::NotAvailable(_))));
    }

    #[tokio::test]
    async fn test_default_memory_info_returns_not_available() {
        let mon = DefaultSystemMonitor::new();
        let ctx = dummy_ctx();
        let result = mon.memory_info(&ctx).await;
        assert!(matches!(result, Err(MonitorError::NotAvailable(_))));
    }

    #[tokio::test]
    async fn test_default_disk_info_returns_not_available() {
        let mon = DefaultSystemMonitor::new();
        let ctx = dummy_ctx();
        let result = mon.disk_info(&ctx, "/").await;
        assert!(matches!(result, Err(MonitorError::NotAvailable(_))));
    }

    #[tokio::test]
    async fn test_default_network_io_returns_not_available() {
        let mon = DefaultSystemMonitor::new();
        let ctx = dummy_ctx();
        let result = mon.network_io(&ctx).await;
        assert!(matches!(result, Err(MonitorError::NotAvailable(_))));
    }

    #[tokio::test]
    async fn test_default_temperature_returns_not_available() {
        let mon = DefaultSystemMonitor::new();
        let ctx = dummy_ctx();
        let result = mon.temperature(&ctx).await;
        assert!(matches!(result, Err(MonitorError::NotAvailable(_))));
    }

    #[tokio::test]
    async fn test_default_process_list_returns_not_available() {
        let mon = DefaultSystemMonitor::new();
        let ctx = dummy_ctx();
        let result = mon.process_list(&ctx).await;
        assert!(matches!(result, Err(MonitorError::NotAvailable(_))));
    }

    #[tokio::test]
    async fn test_default_events_returns_closed_receiver() {
        let mon = DefaultSystemMonitor::new();
        let mut rx = mon.events();
        let result = rx.recv().await;
        assert!(result.is_none());
    }

    #[test]
    fn test_cpu_snapshot_construction() {
        let core = CpuCore {
            index: 0,
            usage_percent: 45.5,
            frequency_mhz: 2400,
        };
        let snap = CpuSnapshot {
            usage_percent: 35.0,
            user_percent: 20.0,
            system_percent: 10.0,
            iowait_percent: 3.0,
            steal_percent: 2.0,
            cores: vec![core],
            context_switches: 1_000_000,
            processes: 200,
        };
        assert_eq!(snap.usage_percent, 35.0);
        assert_eq!(snap.cores.len(), 1);
    }

    #[test]
    fn test_memory_snapshot_construction() {
        let snap = MemorySnapshot {
            total_bytes: 16_000_000_000,
            available_bytes: 8_000_000_000,
            used_bytes: 7_000_000_000,
            cached_bytes: 2_000_000_000,
            buffers_bytes: 500_000_000,
            swap_total: 2_000_000_000,
            swap_used: 100_000_000,
            swap_percent: 5.0,
        };
        assert_eq!(snap.total_bytes, 16_000_000_000);
        assert_eq!(snap.swap_percent, 5.0);
    }

    #[test]
    fn test_disk_snapshot_construction() {
        let snap = DiskSnapshot {
            mount_point: "/".into(),
            device: "/dev/sda1".into(),
            total_bytes: 100_000_000_000,
            used_bytes: 50_000_000_000,
            available_bytes: 50_000_000_000,
            usage_percent: 50.0,
            read_bytes_per_sec: 100_000,
            write_bytes_per_sec: 50_000,
            iops_read: 100,
            iops_write: 50,
        };
        assert_eq!(snap.mount_point, "/");
        assert_eq!(snap.usage_percent, 50.0);
    }

    #[test]
    fn test_network_io_snapshot_construction() {
        let snap = NetworkIoSnapshot {
            interface: "eth0".into(),
            rx_bytes: 1_000_000,
            tx_bytes: 500_000,
            rx_packets: 10_000,
            tx_packets: 5_000,
            rx_errors: 0,
            tx_errors: 1,
            rx_dropped: 2,
            tx_dropped: 0,
        };
        assert_eq!(snap.interface, "eth0");
        assert_eq!(snap.tx_errors, 1);
    }

    #[test]
    fn test_temperature_snapshot_construction() {
        let snap = TemperatureSnapshot {
            sensor: "coretemp-isa-0000".into(),
            label: "Package id 0".into(),
            current_celsius: 65.0,
            high_celsius: Some(85.0),
            critical_celsius: Some(100.0),
        };
        assert_eq!(snap.current_celsius, 65.0);
        assert!(snap.high_celsius.is_some());
    }

    #[test]
    fn test_system_load_construction() {
        let load = SystemLoad {
            load_1m: 1.5,
            load_5m: 2.0,
            load_15m: 1.8,
            running_processes: 3,
            total_processes: 250,
        };
        assert_eq!(load.load_1m, 1.5);
        assert_eq!(load.total_processes, 250);
    }

    #[test]
    fn test_os_info_construction() {
        let info = OsInfo {
            hostname: "archbox".into(),
            os_name: "Arch Linux".into(),
            os_version: "rolling".into(),
            kernel_version: "6.5.7-arch1-1".into(),
            architecture: "x86_64".into(),
        };
        assert_eq!(info.hostname, "archbox");
        assert_eq!(info.architecture, "x86_64");
    }

    #[test]
    fn test_resource_threshold_construction() {
        let threshold = ResourceThreshold {
            resource: ResourceType::Cpu,
            metric: "usage_percent".into(),
            operator: ThresholdOperator::GreaterThan,
            value: 90.0,
            cooldown_seconds: 60,
        };
        assert_eq!(threshold.resource, ResourceType::Cpu);
        assert_eq!(threshold.value, 90.0);
    }

    #[test]
    fn test_cpu_snapshot_serde_roundtrip() {
        let snap = CpuSnapshot {
            usage_percent: 50.0,
            user_percent: 30.0,
            system_percent: 15.0,
            iowait_percent: 3.0,
            steal_percent: 2.0,
            cores: vec![CpuCore {
                index: 0,
                usage_percent: 50.0,
                frequency_mhz: 2400,
            }],
            context_switches: 500_000,
            processes: 150,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let deserialized: CpuSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap.usage_percent, deserialized.usage_percent);
        assert_eq!(snap.cores.len(), deserialized.cores.len());
    }

    #[test]
    fn test_memory_snapshot_serde_roundtrip() {
        let snap = MemorySnapshot {
            total_bytes: 8_000_000_000,
            available_bytes: 4_000_000_000,
            used_bytes: 3_500_000_000,
            cached_bytes: 1_000_000_000,
            buffers_bytes: 200_000_000,
            swap_total: 1_000_000_000,
            swap_used: 50_000_000,
            swap_percent: 5.0,
        };
        let json = serde_json::to_string(&snap).unwrap();
        let deserialized: MemorySnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snap.total_bytes, deserialized.total_bytes);
    }

    #[test]
    fn test_resource_type_debug_and_eq() {
        assert_eq!(ResourceType::Cpu, ResourceType::Cpu);
        assert_ne!(ResourceType::Cpu, ResourceType::Memory);
        assert_eq!(format!("{:?}", ResourceType::Disk), "Disk");
    }

    #[test]
    fn test_threshold_operator_debug_and_eq() {
        assert_eq!(
            ThresholdOperator::GreaterThan,
            ThresholdOperator::GreaterThan
        );
        assert_ne!(ThresholdOperator::GreaterThan, ThresholdOperator::LessThan);
    }

    #[test]
    fn test_default_system_monitor_debug() {
        let mon = DefaultSystemMonitor::new();
        let debug_str = format!("{:?}", mon);
        assert_eq!(debug_str, "DefaultSystemMonitor");
    }
}
