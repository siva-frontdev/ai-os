//! OSAL — Platform Information Types
//!
//! This crate provides rich platform information types:
//! - [`OsInformation`] — detailed OS metadata (name, version, home URL, etc.)
//! - [`KernelInformation`] — kernel release, version, architecture, build date
//! - [`HardwareInformation`] — CPU, memory, model, manufacturer details
//!
//! It also re-exports the [`PlatformInfo`] trait from [`osal_core`], a
//! synchronous read-only interface for basic platform properties such as
//! hostname, CPU count, memory, uptime, kernel version, and OS name/version.
//!
//! ## Re-exports
//!
//! | Symbol | Source | Description |
//! |---|---|---|
//! | [`PlatformInfo`] | `osal_core` | Basic platform information trait |
//!
//! ## Provided types
//!
//! | Type | Description |
//! |---|---|
//! | [`OsInformation`] | OS name, version, pretty name, support URLs |
//! | [`KernelInformation`] | Kernel release, version string, architecture |
//! | [`HardwareInformation`] | CPU count, memory, model, manufacturer |

pub use osal_core::PlatformInfo;

mod types;
pub use types::{HardwareInformation, KernelInformation, OsInformation};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_information_serde_roundtrip() {
        let info = OsInformation {
            name: "Arch Linux".into(),
            version: "rolling".into(),
            version_id: "2024.01".into(),
            pretty_name: "Arch Linux".into(),
            id_like: None,
            home_url: Some("https://archlinux.org".into()),
            support_url: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: OsInformation = serde_json::from_str(&json).unwrap();
        assert_eq!(info.name, decoded.name);
        assert_eq!(info.version_id, decoded.version_id);
    }

    #[test]
    fn test_kernel_information_serde_roundtrip() {
        let info = KernelInformation {
            release: "6.6.1-arch1".into(),
            version: "#1 SMP PREEMPT_DYNAMIC".into(),
            architecture: "x86_64".into(),
            build_date: Some("2024-01-15".into()),
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: KernelInformation = serde_json::from_str(&json).unwrap();
        assert_eq!(info.release, decoded.release);
        assert_eq!(info.architecture, decoded.architecture);
    }

    #[test]
    fn test_hardware_information_serde_roundtrip() {
        let info = HardwareInformation {
            model: Some("ThinkPad X1 Carbon".into()),
            serial: Some("ABC123".into()),
            manufacturer: Some("Lenovo".into()),
            total_memory_bytes: 17179869184,
            processor_count: 8,
            processor_model: Some("Intel Core i7-1365U".into()),
            processor_frequency_mhz: Some(1800),
            virtualization: Some("KVM".into()),
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: HardwareInformation = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.model, info.model);
        assert_eq!(decoded.total_memory_bytes, info.total_memory_bytes);
        assert_eq!(decoded.processor_count, info.processor_count);
    }

    #[test]
    fn test_os_information_minimal() {
        let info = OsInformation {
            name: String::new(),
            version: String::new(),
            version_id: String::new(),
            pretty_name: String::new(),
            id_like: None,
            home_url: None,
            support_url: None,
        };
        assert!(info.name.is_empty());
        assert!(info.home_url.is_none());
    }

    #[test]
    fn test_hardware_information_minimal() {
        let info = HardwareInformation {
            model: None,
            serial: None,
            manufacturer: None,
            total_memory_bytes: 0,
            processor_count: 0,
            processor_model: None,
            processor_frequency_mhz: None,
            virtualization: None,
        };
        assert!(info.model.is_none());
        assert_eq!(info.total_memory_bytes, 0);
    }

    #[test]
    fn test_platform_info_trait_re_exported() {
        fn _assert_trait_object(_: &dyn PlatformInfo) {}
    }

    #[test]
    fn test_os_information_defaults() {
        let info = OsInformation {
            name: "TestOS".into(),
            version: "1.0".into(),
            version_id: "1".into(),
            pretty_name: "Test OS 1.0".into(),
            id_like: Some("debian".into()),
            home_url: Some("https://example.com".into()),
            support_url: Some("https://support.example.com".into()),
        };
        assert_eq!(info.id_like.as_deref(), Some("debian"));
        assert_eq!(info.home_url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn test_kernel_information_minimal() {
        let info = KernelInformation {
            release: "5.10.0".into(),
            version: String::new(),
            architecture: "arm64".into(),
            build_date: None,
        };
        assert_eq!(info.release, "5.10.0");
        assert!(info.build_date.is_none());
    }
}
