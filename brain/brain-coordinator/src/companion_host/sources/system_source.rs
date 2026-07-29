use std::sync::Mutex;

use super::ObservationSource;

/// Monitors system load and memory via /proc.
///
/// Only emits when CPU load changes significantly (>0.5 delta)
/// or memory usage shifts (>5% delta).
#[derive(Debug)]
pub struct SystemSource {
    last_load_key: Mutex<Option<u64>>,
    last_mem_key: Mutex<Option<u64>>,
}

impl SystemSource {
    pub fn new() -> Self {
        Self {
            last_load_key: Mutex::new(None),
            last_mem_key: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl ObservationSource for SystemSource {
    fn name(&self) -> &'static str {
        "system"
    }

    async fn poll(&self) -> Option<String> {
        let (load, mem_pct) = read_load().await?;

        let load_key = (load * 100.0) as u64;
        let mem_key = (mem_pct * 100.0) as u64;

        let mut last_load = self.last_load_key.lock().ok()?;
        let mut last_mem = self.last_mem_key.lock().ok()?;

        let cpu_changed = last_load.map_or(true, |l| load_key.abs_diff(l) > 50);
        let mem_changed = last_mem.map_or(true, |m| mem_key.abs_diff(m) > 500);

        if !cpu_changed && !mem_changed {
            return None;
        }

        *last_load = Some(load_key);
        *last_mem = Some(mem_key);

        Some(format!(
            "system load: {load:.2}, memory: {mem_pct:.0}% used"
        ))
    }
}

async fn read_load() -> Option<(f64, f64)> {
    let load = tokio::fs::read_to_string("/proc/loadavg").await.ok()?;
    let parts: Vec<&str> = load.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let one_min: f64 = parts[0].parse().ok()?;

    let meminfo = tokio::fs::read_to_string("/proc/meminfo").await.ok()?;
    let total = parse_mem_value(&meminfo, "MemTotal:")?;
    let avail = parse_mem_value(&meminfo, "MemAvailable:")?;
    let mem_pct = 100.0 - (avail as f64 / total as f64 * 100.0);

    Some((one_min, mem_pct))
}

fn parse_mem_value(data: &str, key: &str) -> Option<u64> {
    let line = data.lines().find(|l| l.starts_with(key))?;
    let val: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(val)
}
