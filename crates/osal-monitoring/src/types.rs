use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CPU
// ---------------------------------------------------------------------------

/// Per-core CPU usage data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCore {
    /// Core index (0-based).
    pub index: u32,
    /// Aggregate CPU usage percentage for this core (0.0 – 100.0).
    pub usage_percent: f64,
    /// Current core frequency in MHz.
    pub frequency_mhz: u64,
}

/// A point-in-time CPU usage snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuSnapshot {
    /// Total CPU usage percentage (0.0 – 100.0).
    pub usage_percent: f64,
    /// Percentage spent in user space.
    pub user_percent: f64,
    /// Percentage spent in kernel space.
    pub system_percent: f64,
    /// Percentage spent waiting on I/O.
    pub iowait_percent: f64,
    /// Percentage stolen by the hypervisor.
    pub steal_percent: f64,
    /// Per-core breakdown.
    pub cores: Vec<CpuCore>,
    /// Number of context switches since boot.
    pub context_switches: u64,
    /// Number of processes currently running.
    pub processes: u32,
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

/// A point-in-time memory usage snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    /// Total physical RAM in bytes.
    pub total_bytes: u64,
    /// Available (free + reclaimable) RAM in bytes.
    pub available_bytes: u64,
    /// Currently used RAM in bytes.
    pub used_bytes: u64,
    /// Cached (page cache + slab) RAM in bytes.
    pub cached_bytes: u64,
    /// Buffer cache RAM in bytes.
    pub buffers_bytes: u64,
    /// Total swap space in bytes.
    pub swap_total: u64,
    /// Used swap space in bytes.
    pub swap_used: u64,
    /// Swap usage as a percentage (0.0 – 100.0).
    pub swap_percent: f64,
}

// ---------------------------------------------------------------------------
// Disk
// ---------------------------------------------------------------------------

/// A point-in-time disk usage snapshot for a single mount point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSnapshot {
    /// Mount point path (e.g. `"/"`, `"/home"`).
    pub mount_point: String,
    /// Block device name (e.g. `"/dev/sda1"`).
    pub device: String,
    /// Total partition size in bytes.
    pub total_bytes: u64,
    /// Used space in bytes.
    pub used_bytes: u64,
    /// Available space in bytes.
    pub available_bytes: u64,
    /// Usage as a percentage (0.0 – 100.0).
    pub usage_percent: f64,
    /// Read throughput in bytes per second.
    pub read_bytes_per_sec: u64,
    /// Write throughput in bytes per second.
    pub write_bytes_per_sec: u64,
    /// Read I/O operations per second.
    pub iops_read: u64,
    /// Write I/O operations per second.
    pub iops_write: u64,
}

// ---------------------------------------------------------------------------
// Network I/O
// ---------------------------------------------------------------------------

/// A point-in-time network I/O snapshot for a single interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIoSnapshot {
    /// Interface name (e.g. `"eth0"`).
    pub interface: String,
    /// Total bytes received.
    pub rx_bytes: u64,
    /// Total bytes transmitted.
    pub tx_bytes: u64,
    /// Total packets received.
    pub rx_packets: u64,
    /// Total packets transmitted.
    pub tx_packets: u64,
    /// Total receive errors.
    pub rx_errors: u64,
    /// Total transmit errors.
    pub tx_errors: u64,
    /// Total receive drops.
    pub rx_dropped: u64,
    /// Total transmit drops.
    pub tx_dropped: u64,
}

// ---------------------------------------------------------------------------
// Temperature
// ---------------------------------------------------------------------------

/// A temperature reading from a hardware sensor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureSnapshot {
    /// Sensor identifier (e.g. `"coretemp-isa-0000"`).
    pub sensor: String,
    /// Human-readable label (e.g. `"Package id 0"`).
    pub label: String,
    /// Current temperature in degrees Celsius.
    pub current_celsius: f64,
    /// High warning threshold, if available.
    pub high_celsius: Option<f64>,
    /// Critical (shutdown) threshold, if available.
    pub critical_celsius: Option<f64>,
}

// ---------------------------------------------------------------------------
// System load
// ---------------------------------------------------------------------------

/// System load averages and process counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLoad {
    /// 1-minute load average.
    pub load_1m: f64,
    /// 5-minute load average.
    pub load_5m: f64,
    /// 15-minute load average.
    pub load_15m: f64,
    /// Number of currently running (R) processes.
    pub running_processes: u32,
    /// Total number of existing processes.
    pub total_processes: u32,
}

// ---------------------------------------------------------------------------
// OS info
// ---------------------------------------------------------------------------

/// Operating-system identification information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    /// System hostname.
    pub hostname: String,
    /// OS distribution name (e.g. `"Arch Linux"`).
    pub os_name: String,
    /// OS version string (e.g. `"23.04"` or release name).
    pub os_version: String,
    /// Kernel version string (e.g. `"6.5.7-arch1-1"`).
    pub kernel_version: String,
    /// Hardware architecture (e.g. `"x86_64"`, `"aarch64"`).
    pub architecture: String,
}

// ---------------------------------------------------------------------------
// Thresholds
// ---------------------------------------------------------------------------

/// Categories of system resources that can be threshold-watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// Central processing unit.
    Cpu,
    /// Random-access memory.
    Memory,
    /// Block storage device.
    Disk,
    /// Network interface.
    Network,
    /// Thermal sensor.
    Temperature,
}

/// Comparison operator for threshold evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThresholdOperator {
    /// Trigger when the metric value exceeds the threshold.
    GreaterThan,
    /// Trigger when the metric value falls below the threshold.
    LessThan,
}

/// A resource threshold that, when crossed, generates a [`MonitoringEvent`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceThreshold {
    /// Which resource category to monitor.
    pub resource: ResourceType,
    /// Metric name (e.g. `"usage_percent"`, `"temperature_celsius"`).
    pub metric: String,
    /// Comparison operator.
    pub operator: ThresholdOperator,
    /// Threshold value to compare against.
    pub value: f64,
    /// Minimum seconds between consecutive events for this threshold.
    pub cooldown_seconds: u64,
}
