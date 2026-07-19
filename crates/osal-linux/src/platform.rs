use std::fmt;
use std::time::Duration;
use osal_core::PlatformInfo;

pub struct LinuxPlatformInfo {
    os_name: String,
    os_version: String,
    hostname: String,
    num_cpus: usize,
    total_memory: u64,
    kernel_version: String,
}

impl LinuxPlatformInfo {
    pub fn new() -> Self {
        Self {
            os_name: read_os_name(),
            os_version: read_os_version(),
            hostname: read_hostname(),
            num_cpus: read_num_cpus(),
            total_memory: read_total_memory(),
            kernel_version: read_kernel_version(),
        }
    }
}

impl fmt::Debug for LinuxPlatformInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxPlatformInfo").finish()
    }
}

fn read_os_release_value(key: &str) -> String {
    let content = std::fs::read_to_string("/etc/os-release")
        .unwrap_or_default();
    for line in content.lines() {
        if let Some(val) = line.strip_prefix(key) {
            if let Some(rest) = val.strip_prefix('=') {
                return rest.trim_matches('"').to_string();
            }
        }
    }
    "unknown".to_string()
}

fn read_os_name() -> String {
    read_os_release_value("ID")
}

fn read_os_version() -> String {
    read_os_release_value("VERSION_ID")
}

fn read_hostname() -> String {
    let mut buf = vec![0i8; 256];
    let ret = unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len()) };
    if ret != 0 {
        return "localhost".to_string();
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    cstr.to_string_lossy().into_owned()
}

fn read_num_cpus() -> usize {
    let n = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
    if n > 0 { n as usize } else { 1 }
}

fn read_total_memory() -> u64 {
    let content = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let val: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(kb) = val.parse::<u64>() {
                return kb * 1024;
            }
        }
    }
    0
}

fn read_kernel_version() -> String {
    let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::uname(&mut uts) };
    if ret != 0 {
        return "unknown".to_string();
    }
    let release = unsafe { std::ffi::CStr::from_ptr(uts.release.as_ptr()) };
    release.to_string_lossy().into_owned()
}

impl PlatformInfo for LinuxPlatformInfo {
    fn os_name(&self) -> &str {
        &self.os_name
    }

    fn os_version(&self) -> &str {
        &self.os_version
    }

    fn hostname(&self) -> String {
        self.hostname.clone()
    }

    fn num_cpus(&self) -> usize {
        self.num_cpus
    }

    fn total_memory(&self) -> u64 {
        self.total_memory
    }

    fn uptime(&self) -> Duration {
        let content = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
        let secs: f64 = content
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        Duration::from_secs_f64(secs)
    }

    fn kernel_version(&self) -> &str {
        &self.kernel_version
    }
}
