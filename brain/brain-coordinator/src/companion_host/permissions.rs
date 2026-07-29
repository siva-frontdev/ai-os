use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Whether a source has permission to observe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PermissionState {
    Granted,
    Denied,
    NotRequested,
}

/// A single permission entry for an observation source category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionEntry {
    pub state: PermissionState,
    pub description: String,
}

/// Registry of all observation source permissions.
///
/// Sources are identified by category name ("desktop", "time", "system",
/// "filesystem", "clipboard", "calendar", etc). The companion checks
/// `is_granted()` before polling any source. Denied sources are silently
/// skipped — they produce no observations and do not error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRegistry {
    pub permissions: HashMap<String, PermissionEntry>,
}

impl PermissionRegistry {
    /// Create a registry with safe defaults (all denied).
    pub fn new() -> Self {
        let mut permissions = HashMap::new();
        for (name, desc) in Self::all_known() {
            permissions.insert(
                name.to_string(),
                PermissionEntry {
                    state: PermissionState::Denied,
                    description: desc.to_string(),
                },
            );
        }
        Self { permissions }
    }

    /// Create a registry with all known sources granted (for first-run convenience).
    pub fn all_granted() -> Self {
        let mut permissions = HashMap::new();
        for (name, desc) in Self::all_known() {
            permissions.insert(
                name.to_string(),
                PermissionEntry {
                    state: PermissionState::Granted,
                    description: desc.to_string(),
                },
            );
        }
        Self { permissions }
    }

    /// Check whether a source category is permitted to observe.
    pub fn is_granted(&self, source_name: &str) -> bool {
        self.permissions
            .get(source_name)
            .map_or(false, |p| p.state == PermissionState::Granted)
    }

    /// Set the permission state for a source category.
    pub fn set(&mut self, source: &str, state: PermissionState) {
        if let Some(entry) = self.permissions.get_mut(source) {
            entry.state = state;
        }
    }

    /// Iterate over all known source categories.
    pub fn all_known() -> Vec<(&'static str, &'static str)> {
        vec![
            ("time", "Track time of day for context awareness"),
            ("system", "Monitor system load and memory usage"),
            (
                "desktop",
                "Observe which windows and applications are active",
            ),
            ("filesystem", "Read file system activity"),
            ("clipboard", "Monitor clipboard content"),
            ("calendar", "Read calendar events"),
            ("email", "Read email activity"),
            ("browser", "Monitor browser activity"),
            ("location", "Access location data"),
            ("microphone", "Access microphone"),
            ("camera", "Access camera"),
        ]
    }

    /// Map an observation source's `name()` to its permission category.
    /// Multiple source implementations may map to the same category.
    pub fn category_for_source(source_name: &str) -> &str {
        match source_name {
            "time" => "time",
            "system" => "system",
            "desktop" => "desktop",
            _ => source_name, // pass through for custom sources
        }
    }
}

impl Default for PermissionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_all_denied() {
        let reg = PermissionRegistry::new();
        assert!(!reg.is_granted("time"));
        assert!(!reg.is_granted("system"));
        assert!(!reg.is_granted("desktop"));
        assert!(!reg.is_granted("unknown_source"));
    }

    #[test]
    fn test_all_granted() {
        let reg = PermissionRegistry::all_granted();
        assert!(reg.is_granted("time"));
        assert!(reg.is_granted("system"));
    }

    #[test]
    fn test_set_permission() {
        let mut reg = PermissionRegistry::new();
        assert!(!reg.is_granted("desktop"));
        reg.set("desktop", PermissionState::Granted);
        assert!(reg.is_granted("desktop"));
        reg.set("desktop", PermissionState::Denied);
        assert!(!reg.is_granted("desktop"));
    }

    #[test]
    fn test_category_mapping() {
        assert_eq!(PermissionRegistry::category_for_source("time"), "time");
        assert_eq!(PermissionRegistry::category_for_source("system"), "system");
        assert_eq!(
            PermissionRegistry::category_for_source("desktop"),
            "desktop"
        );
        assert_eq!(
            PermissionRegistry::category_for_source("custom_thing"),
            "custom_thing"
        );
    }

    #[test]
    fn test_unknown_source_not_granted() {
        let reg = PermissionRegistry::new();
        assert!(!reg.is_granted("does_not_exist"));
    }
}
