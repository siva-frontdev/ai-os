# OSAL — Platform Information Types

Rich platform information types describing the operating system, kernel,
and hardware of the host machine.

## Re-exports

- `PlatformInfo` — basic platform information trait from `osal_core`

## Types

```rust
pub struct OsInformation {
    pub name: String,
    pub version: String,
    pub version_id: String,
    pub pretty_name: String,
    pub id_like: Option<String>,
    pub home_url: Option<String>,
    pub support_url: Option<String>,
}

pub struct KernelInformation {
    pub release: String,
    pub version: String,
    pub architecture: String,
    pub build_date: Option<String>,
}

pub struct HardwareInformation {
    pub model: Option<String>,
    pub serial: Option<String>,
    pub manufacturer: Option<String>,
    pub total_memory_bytes: u64,
    pub processor_count: u32,
    pub processor_model: Option<String>,
    pub processor_frequency_mhz: Option<u64>,
    pub virtualization: Option<String>,
}
```
