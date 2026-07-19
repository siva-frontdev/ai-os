# osal-monitoring — System Monitoring Subsystem Trait Definitions

## Architecture

The `SystemMonitor` trait provides an OS-agnostic interface for collecting
system resource snapshots (CPU, memory, disk, network I/O, temperature) and
for watching resource thresholds.

```
┌───────────────────────────────────────────────────────┐
│                    SystemMonitor                       │
│  ┌───────┐ ┌──────┐ ┌──────┐ ┌────────┐ ┌─────────┐ │
│  │  CPU  │ │Memory│ │ Disk │ │Network │ │Thermal  │ │
│  │Snapshot│ │Snap. │ │Snap. │ │IOSnap. │ │Snapshot │ │
│  └───────┘ └──────┘ └──────┘ └────────┘ └─────────┘ │
│  ┌──────────┐ ┌───────┐ ┌────────┐ ┌────────────────┐│
│  │SystemLoad│ │Uptime │ │Hostname│ │  OsInfo        ││
│  └──────────┘ └───────┘ └────────┘ └────────────────┘│
│  ┌───────────────────────────────────────────────────┐│
│  │         watch_threshold(ResourceThreshold)         ││
│  │         → mpsc::Receiver<MonitoringEvent>          ││
│  └───────────────────────────────────────────────────┘│
└───────────────────────────────────────────────────────┘
```

All monitoring operations that return resource usage data are capability-gated.
Operations such as `uptime()` and `hostname()` are typically low-privilege and
may not require a context.

---

## Trait: `SystemMonitor`

```rust
use async_trait::async_trait;
use std::fmt::Debug;
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::mpsc;
use osal_core::OsalResult;
use osal_capabilities::CapabilityContext;

#[async_trait]
pub trait SystemMonitor: Debug + Send + Sync {
    /// Capture a point-in-time CPU usage snapshot.
    async fn cpu_snapshot(&self, ctx: &CapabilityContext) -> OsalResult<CpuSnapshot>;

    /// Capture a point-in-time memory usage snapshot.
    async fn memory_snapshot(&self, ctx: &CapabilityContext) -> OsalResult<MemorySnapshot>;

    /// Capture point-in-time disk usage snapshots for all mount points.
    async fn disk_snapshot(&self, ctx: &CapabilityContext) -> OsalResult<Vec<DiskSnapshot>>;

    /// Capture a point-in-time network I/O snapshot.
    async fn network_snapshot(&self, ctx: &CapabilityContext) -> OsalResult<NetworkIoSnapshot>;

    /// Capture temperature readings from all available sensors.
    async fn temperature_snapshot(&self, ctx: &CapabilityContext) -> OsalResult<Vec<TemperatureSnapshot>>;

    /// Get the current system load averages.
    async fn system_load(&self, ctx: &CapabilityContext) -> OsalResult<SystemLoad>;

    /// Get the system uptime duration.
    async fn uptime(&self) -> OsalResult<Duration>;

    /// Get the system hostname.
    async fn hostname(&self) -> OsalResult<String>;

    /// Get operating-system information.
    async fn os_info(&self) -> OsalResult<OsInfo>;

    /// Register a resource threshold watcher.
    ///
    /// Returns a receiver that delivers [`MonitoringEvent`] values whenever
    /// the given threshold is crossed.
    async fn watch_threshold(&self, threshold: &ResourceThreshold)
        -> OsalResult<mpsc::Receiver<MonitoringEvent>>;
}
```

---

## Data Types

### `CpuSnapshot`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuSnapshot {
    pub usage_percent: f64,
    pub user_percent: f64,
    pub system_percent: f64,
    pub iowait_percent: f64,
    pub steal_percent: f64,
    pub cores: Vec<CpuCore>,
    pub context_switches: u64,
    pub processes: u32,
}
```

### `CpuCore`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCore {
    pub index: u32,
    pub usage_percent: f64,
    pub frequency_mhz: u64,
}
```

### `MemorySnapshot`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub cached_bytes: u64,
    pub buffers_bytes: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub swap_percent: f64,
}
```

### `DiskSnapshot`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSnapshot {
    pub mount_point: String,
    pub device: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f64,
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub iops_read: u64,
    pub iops_write: u64,
}
```

### `NetworkIoSnapshot`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIoSnapshot {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}
```

### `TemperatureSnapshot`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureSnapshot {
    pub sensor: String,
    pub label: String,
    pub current_celsius: f64,
    pub high_celsius: Option<f64>,
    pub critical_celsius: Option<f64>,
}
```

### `SystemLoad`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLoad {
    pub load_1m: f64,
    pub load_5m: f64,
    pub load_15m: f64,
    pub running_processes: u32,
    pub total_processes: u32,
}
```

### `OsInfo`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub architecture: String,
}
```

### `ResourceThreshold`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceThreshold {
    pub resource: ResourceType,
    pub metric: String,
    pub operator: ThresholdOperator,
    pub value: f64,
    pub cooldown_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Cpu,
    Memory,
    Disk,
    Network,
    Temperature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThresholdOperator {
    GreaterThan,
    LessThan,
}
```

---

## Event: `MonitoringEvent`

```rust
#[derive(Debug, Clone)]
pub enum MonitoringEvent {
    CrossedThreshold {
        resource: ResourceType,
        metric: String,
        value: f64,
        threshold: f64,
    },
    Warning {
        message: String,
    },
    Normalized {
        resource: ResourceType,
        metric: String,
    },
}
```

---

## Error: `MonitorError`

```rust
#[derive(Debug, thiserror::Error)]
pub enum MonitorError {
    #[error("monitoring data collection failed: {0}")]
    CollectionFailed(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("operation not supported on this platform: {0}")]
    NotSupported(String),

    #[error("no data available: {0}")]
    NoData(String),

    #[error("I/O error: {0}")]
    IoError(String),
}
```
