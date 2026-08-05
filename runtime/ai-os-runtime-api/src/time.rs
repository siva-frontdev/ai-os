//! Shared clock helpers for the Runtime API.

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}
