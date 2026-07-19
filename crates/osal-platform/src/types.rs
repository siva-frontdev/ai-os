use serde::{Deserialize, Serialize};

/// Detailed metadata about the operating system.
///
/// Fields map to fields in `/etc/os-release` on Linux systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInformation {
    /// Machine-readable OS name (e.g. `"arch"`)
    pub name: String,
    /// Version string (e.g. `"rolling"`)
    pub version: String,
    /// Version identifier (e.g. `"2024.01"`)
    pub version_id: String,
    /// Human-readable OS name (e.g. `"Arch Linux"`)
    pub pretty_name: String,
    /// Identifier of a related OS family (e.g. `Some("debian")`)
    pub id_like: Option<String>,
    /// Home page URL for the OS
    pub home_url: Option<String>,
    /// Support / bug-report URL
    pub support_url: Option<String>,
}

/// Information about the running kernel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelInformation {
    /// Kernel release string (e.g. `"6.6.1-arch1"`)
    pub release: String,
    /// Full kernel version string (e.g. `"#1 SMP PREEMPT_DYNAMIC"`)
    pub version: String,
    /// Hardware architecture (e.g. `"x86_64"`, `"aarch64"`)
    pub architecture: String,
    /// Optional kernel build date
    pub build_date: Option<String>,
}

/// Information about the host hardware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInformation {
    /// Product model name (e.g. `"ThinkPad X1 Carbon"`)
    pub model: Option<String>,
    /// Product serial number
    pub serial: Option<String>,
    /// Manufacturer name (e.g. `"Lenovo"`)
    pub manufacturer: Option<String>,
    /// Total physical memory in bytes
    pub total_memory_bytes: u64,
    /// Number of logical processors
    pub processor_count: u32,
    /// CPU model name
    pub processor_model: Option<String>,
    /// CPU base frequency in MHz
    pub processor_frequency_mhz: Option<u64>,
    /// Virtualization technology (e.g. `"KVM"`, `"VMware"`)
    pub virtualization: Option<String>,
}
