use std::sync::Mutex;

use super::ObservationSource;

/// Reports time-of-day changes (hour granularity).
///
/// Prevents constant time-based noise by only producing an
/// observation when the hour bucket changes.
#[derive(Debug)]
pub struct TimeSource {
    last_bucket: Mutex<Option<i32>>,
}

impl TimeSource {
    pub fn new() -> Self {
        Self {
            last_bucket: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl ObservationSource for TimeSource {
    fn name(&self) -> &'static str {
        "time"
    }

    async fn poll(&self) -> Option<String> {
        let (day, hour) = current_time_bucket();
        let bucket = (day * 100 + hour) as i32;

        let mut last = self.last_bucket.lock().ok()?;
        if *last == Some(bucket) {
            return None;
        }
        *last = Some(bucket);

        let period = match hour {
            5..=11 => "morning",
            12..=16 => "afternoon",
            17..=21 => "evening",
            _ => "night",
        };
        Some(format!("day {day}, {period} ({hour}:00)"))
    }
}

fn current_time_bucket() -> (u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let day = (secs / 86400) as u32;
    let hour = ((secs / 3600) % 24) as u32;
    (day, hour)
}
