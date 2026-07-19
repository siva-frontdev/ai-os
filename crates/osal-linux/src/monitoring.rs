use std::ffi::CString;
use std::fmt;
use std::fs;
use std::path::Path;

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use osal_core::{
    DiskInfo, MemoryInfo, MonitorError, NetworkIO, OsalEvent, Pid, ProcessInfo, SystemMonitor, Uid,
};
use tokio::sync::mpsc::Receiver;

pub struct LinuxSystemMonitor;

impl LinuxSystemMonitor {
    pub fn new() -> Self {
        Self
    }
}

impl fmt::Debug for LinuxSystemMonitor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxSystemMonitor").finish()
    }
}

#[async_trait]
impl SystemMonitor for LinuxSystemMonitor {
    async fn cpu_usage(&self, _ctx: &CapabilityContext) -> Result<f64, MonitorError> {
        let content = tokio::task::spawn_blocking(|| {
            fs::read_to_string("/proc/stat").map_err(|e| MonitorError::Io(e.to_string()))
        })
        .await
        .map_err(|e| MonitorError::Io(e.to_string()))??;

        let line = content
            .lines()
            .next()
            .ok_or_else(|| MonitorError::NotAvailable("empty /proc/stat".into()))?;

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 || parts[0] != "cpu" {
            return Err(MonitorError::NotAvailable(
                "unexpected /proc/stat format".into(),
            ));
        }

        let parse = |i: usize| -> Result<u64, MonitorError> {
            parts.get(i).and_then(|s| s.parse().ok()).ok_or_else(|| {
                MonitorError::Io(format!("failed to parse field {} of /proc/stat", i))
            })
        };

        let user = parse(1)?;
        let nice = parse(2)?;
        let system = parse(3)?;
        let idle = parse(4)?;
        let iowait = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
        let irq = parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
        let softirq = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);
        let steal = parts.get(8).and_then(|s| s.parse().ok()).unwrap_or(0);

        let total = user + nice + system + idle + iowait + irq + softirq + steal;
        if total == 0 {
            return Ok(0.0);
        }
        Ok((total - idle) as f64 / total as f64)
    }

    async fn memory_info(&self, _ctx: &CapabilityContext) -> Result<MemoryInfo, MonitorError> {
        let content = tokio::task::spawn_blocking(|| {
            fs::read_to_string("/proc/meminfo").map_err(|e| MonitorError::Io(e.to_string()))
        })
        .await
        .map_err(|e| MonitorError::Io(e.to_string()))??;

        let parse_kb = |prefix: &str| -> Result<u64, MonitorError> {
            for line in content.lines() {
                if line.starts_with(prefix) {
                    let val: u64 = line
                        .split_whitespace()
                        .nth(1)
                        .ok_or_else(|| MonitorError::Io(format!("missing value for {}", prefix)))?
                        .parse()
                        .map_err(|e| {
                            MonitorError::Io(format!("failed to parse {}: {}", prefix, e))
                        })?;
                    return Ok(val * 1024);
                }
            }
            Err(MonitorError::NotAvailable(format!(
                "{} not found in /proc/meminfo",
                prefix
            )))
        };

        let total = parse_kb("MemTotal:")?;
        let available = parse_kb("MemAvailable:")?;
        let free = parse_kb("MemFree:")?;
        let cached = parse_kb("Cached:").unwrap_or(0);
        let buffers = parse_kb("Buffers:").unwrap_or(0);

        Ok(MemoryInfo {
            total,
            used: total.saturating_sub(available),
            free,
            cached: cached + buffers,
        })
    }

    #[allow(unused_variables)]
    async fn disk_info(
        &self,
        _ctx: &CapabilityContext,
        path: &str,
    ) -> Result<DiskInfo, MonitorError> {
        let path_owned = path.to_string();
        let stat_result = tokio::task::spawn_blocking(move || {
            let cpath = CString::new(path_owned.as_str())
                .map_err(|e| MonitorError::Io(format!("invalid path: {}", e)))?;
            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
            let ret = unsafe { libc::statvfs(cpath.as_ptr(), &mut stat) };
            if ret != 0 {
                return Err(MonitorError::Io(
                    std::io::Error::last_os_error().to_string(),
                ));
            }
            let block_size = stat.f_frsize as u64;
            let total_blocks = stat.f_blocks as u64;
            let free_blocks = stat.f_bavail as u64;
            Ok((total_blocks * block_size, free_blocks * block_size))
        })
        .await
        .map_err(|e| MonitorError::Io(e.to_string()))??;

        let (total, free) = stat_result;

        let path_clone = path.to_string();
        let filesystem = tokio::task::spawn_blocking(move || {
            get_fs_type(&path_clone).unwrap_or_else(|| "unknown".into())
        })
        .await
        .map_err(|e| MonitorError::Io(e.to_string()))?;

        Ok(DiskInfo {
            total,
            used: total.saturating_sub(free),
            free,
            mount_point: path.to_string(),
            filesystem,
        })
    }

    async fn network_io(&self, _ctx: &CapabilityContext) -> Result<NetworkIO, MonitorError> {
        let content = tokio::task::spawn_blocking(|| {
            fs::read_to_string("/proc/net/dev").map_err(|e| MonitorError::Io(e.to_string()))
        })
        .await
        .map_err(|e| MonitorError::Io(e.to_string()))??;

        let mut bytes_sent: u64 = 0;
        let mut bytes_received: u64 = 0;
        let mut packets_sent: u64 = 0;
        let mut packets_received: u64 = 0;

        for line in content.lines().skip(2) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 10 {
                continue;
            }
            if let Ok(val) = parts[1].parse::<u64>() {
                bytes_received += val;
            }
            if let Ok(val) = parts[2].parse::<u64>() {
                packets_received += val;
            }
            if parts.len() > 9 {
                if let Ok(val) = parts[9].parse::<u64>() {
                    bytes_sent += val;
                }
            }
            if parts.len() > 10 {
                if let Ok(val) = parts[10].parse::<u64>() {
                    packets_sent += val;
                }
            }
        }

        Ok(NetworkIO {
            bytes_sent,
            bytes_received,
            packets_sent,
            packets_received,
        })
    }

    async fn temperature(&self, _ctx: &CapabilityContext) -> Result<f64, MonitorError> {
        let temp_str = tokio::task::spawn_blocking(|| -> Result<String, MonitorError> {
            let thermal_dir = Path::new("/sys/class/thermal");
            let dir = fs::read_dir(thermal_dir)
                .map_err(|e| MonitorError::Io(format!("cannot list thermal zones: {}", e)))?;
            for entry in dir {
                let entry = entry.map_err(|e| MonitorError::Io(e.to_string()))?;
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("thermal_zone") {
                    let temp_path = entry.path().join("temp");
                    if let Ok(s) = fs::read_to_string(&temp_path) {
                        return Ok(s.trim().to_string());
                    }
                }
            }
            Err(MonitorError::NotAvailable("no thermal zone found".into()))
        })
        .await
        .map_err(|e| MonitorError::Io(e.to_string()))??;

        let millidegrees: f64 = temp_str
            .parse()
            .map_err(|e| MonitorError::Io(format!("failed to parse temperature: {}", e)))?;
        Ok(millidegrees / 1000.0)
    }

    async fn process_list(
        &self,
        _ctx: &CapabilityContext,
    ) -> Result<Vec<ProcessInfo>, MonitorError> {
        tokio::task::spawn_blocking(read_process_list)
            .await
            .map_err(|e| MonitorError::Io(e.to_string()))?
    }

    fn events(&self) -> Receiver<OsalEvent> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        drop(tx);
        rx
    }
}

fn read_process_list() -> Result<Vec<ProcessInfo>, MonitorError> {
    let mut processes = Vec::new();
    let proc_dir = Path::new("/proc");

    let dir = fs::read_dir(proc_dir)
        .map_err(|e| MonitorError::Io(format!("cannot read /proc: {}", e)))?;

    for entry in dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let pid: u64 = match name_str.parse() {
            Ok(p) if p > 0 => p,
            _ => continue,
        };

        let stat_path = entry.path().join("stat");
        let stat_content = match fs::read_to_string(&stat_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let (comm, state, ppid) = parse_proc_stat(&stat_content);

        let status_path = entry.path().join("status");
        let status_content = match fs::read_to_string(&status_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let uid = parse_status_u64(&status_content, "Uid:");
        let vm_rss_kb = parse_status_u64(&status_content, "VmRSS:");

        let cmdline_path = entry.path().join("cmdline");
        let cmdline = match fs::read(&cmdline_path) {
            Ok(bytes) => {
                let s = String::from_utf8_lossy(&bytes);
                s.replace('\0', " ").trim().to_string()
            }
            Err(_) => String::new(),
        };

        let command = if cmdline.is_empty() { comm } else { cmdline };

        processes.push(ProcessInfo {
            pid: Pid(pid),
            parent_pid: Some(Pid(ppid as u64)),
            command,
            user: Uid(uid as u32),
            state,
            cpu_usage: 0.0,
            memory_usage: vm_rss_kb * 1024,
        });
    }

    Ok(processes)
}

fn parse_proc_stat(content: &str) -> (String, String, i64) {
    let open = match content.find('(') {
        Some(o) => o,
        None => return (String::new(), String::new(), 0),
    };
    let close = match content.rfind(')') {
        Some(c) => c,
        None => return (String::new(), String::new(), 0),
    };

    let comm = content[open + 1..close].to_string();
    let rest = &content[close + 2..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let state = fields.first().unwrap_or(&"").to_string();
    let ppid: i64 = fields.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    (comm, state, ppid)
}

fn parse_status_u64(content: &str, prefix: &str) -> u64 {
    for line in content.lines() {
        if line.starts_with(prefix) {
            if let Some(val_str) = line.split_whitespace().nth(1) {
                if let Ok(val) = val_str.parse::<u64>() {
                    return val;
                }
            }
        }
    }
    0
}

fn get_fs_type(path: &str) -> Option<String> {
    let resolved = fs::canonicalize(path).ok()?;
    let resolved_str = resolved.to_string_lossy();
    let content = fs::read_to_string("/proc/mounts").ok()?;
    let mut best_match: Option<(usize, String)> = None;
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let mount_point = parts[1];
        let fs_type = parts[2];
        if resolved_str.starts_with(mount_point) {
            let len = mount_point.len();
            match &best_match {
                Some((best_len, _)) if len > *best_len => {
                    best_match = Some((len, fs_type.to_string()));
                }
                None => {
                    best_match = Some((len, fs_type.to_string()));
                }
                _ => {}
            }
        }
    }
    best_match.map(|(_, t)| t)
}
