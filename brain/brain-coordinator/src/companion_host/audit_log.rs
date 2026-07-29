use std::collections::VecDeque;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::CoordinatorError;

/// A single entry in the read-only activity audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub timestamp: DateTime<Utc>,
    pub category: ActivityCategory,
    pub summary: String,
    pub details: String,
}

/// Categories of user-visible activity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActivityCategory {
    Observation,
    Conversation,
    MemoryCreated,
    MemoryUpdated,
    RelationshipUpdated,
    Notification,
    UserCorrection,
    MemoryDeletion,
    SettingsChange,
    Backup,
    Restore,
    EncryptionChange,
    PermissionChange,
    DataExpired,
    DataArchived,
}

impl std::fmt::Display for ActivityCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Observation => write!(f, "Observation"),
            Self::Conversation => write!(f, "Conversation"),
            Self::MemoryCreated => write!(f, "Memory created"),
            Self::MemoryUpdated => write!(f, "Memory updated"),
            Self::RelationshipUpdated => write!(f, "Relationship updated"),
            Self::Notification => write!(f, "Notification"),
            Self::UserCorrection => write!(f, "User correction"),
            Self::MemoryDeletion => write!(f, "Memory deletion"),
            Self::SettingsChange => write!(f, "Settings change"),
            Self::Backup => write!(f, "Backup"),
            Self::Restore => write!(f, "Restore"),
            Self::EncryptionChange => write!(f, "Encryption change"),
            Self::PermissionChange => write!(f, "Permission change"),
            Self::DataExpired => write!(f, "Data expired"),
            Self::DataArchived => write!(f, "Data archived"),
        }
    }
}

/// Read-only audit log of all trust-relevant activity.
///
/// Entries are never modified or deleted after creation. The log is
/// persisted to disk as a JSON array and loaded on startup. Only the
/// most recent entries are kept (configurable max).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    entries: VecDeque<ActivityEntry>,
    #[serde(skip)]
    max_entries: usize,
    #[serde(skip)]
    path: Option<std::path::PathBuf>,
}

impl AuditLog {
    /// Create a new in-memory audit log.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries.min(10000)),
            max_entries,
            path: None,
        }
    }

    /// Set the persistence path for the audit log.
    pub fn with_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Record an activity entry.
    pub fn record(
        &mut self,
        category: ActivityCategory,
        summary: impl Into<String>,
        details: impl Into<String>,
    ) {
        let entry = ActivityEntry {
            timestamp: Utc::now(),
            category,
            summary: summary.into(),
            details: details.into(),
        };
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Return all entries (most recent first).
    pub fn entries(&self) -> Vec<ActivityEntry> {
        self.entries.iter().rev().cloned().collect()
    }

    /// Return entries matching a category (most recent first).
    pub fn entries_by_category(&self, category: &ActivityCategory) -> Vec<ActivityEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| e.category == *category)
            .cloned()
            .collect()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Load the audit log from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, CoordinatorError> {
        let data = match std::fs::read_to_string(path.as_ref()) {
            Ok(d) => d,
            Err(_) => return Ok(Self::new(1000)),
        };
        let mut log: AuditLog = serde_json::from_str(&data)
            .map_err(|e| CoordinatorError::Internal(format!("audit log parse: {e}")))?;
        log.path = Some(path.as_ref().to_path_buf());
        log.max_entries = 1000;
        Ok(log)
    }

    /// Save the audit log to its configured path.
    pub fn save(&self) -> Result<(), CoordinatorError> {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => return Ok(()),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CoordinatorError::Internal(format!("audit log dir: {e}")))?;
        }
        let json = serde_json::to_string_pretty(&self)
            .map_err(|e| CoordinatorError::Internal(format!("audit log serialize: {e}")))?;
        std::fs::write(&path, &json)
            .map_err(|e| CoordinatorError::Internal(format!("audit log save: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_retrieve() {
        let mut log = AuditLog::new(100);
        log.record(
            ActivityCategory::Observation,
            "Observed desktop",
            "Active window: Firefox",
        );
        log.record(
            ActivityCategory::MemoryCreated,
            "Created entity",
            "Name: AI-OS",
        );

        assert_eq!(log.len(), 2);
        let entries = log.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].summary, "Created entity");
        assert_eq!(entries[1].summary, "Observed desktop");
    }

    #[test]
    fn test_max_entries() {
        let mut log = AuditLog::new(5);
        for i in 0..10 {
            log.record(ActivityCategory::Observation, format!("entry {i}"), "");
        }
        assert_eq!(log.len(), 5);
        let entries = log.entries();
        assert_eq!(entries[0].summary, "entry 9"); // most recent first
        assert_eq!(entries[4].summary, "entry 5");
    }

    #[test]
    fn test_filter_by_category() {
        let mut log = AuditLog::new(100);
        log.record(ActivityCategory::Observation, "obs1", "");
        log.record(ActivityCategory::Conversation, "chat1", "");
        log.record(ActivityCategory::Observation, "obs2", "");

        let obs_entries = log.entries_by_category(&ActivityCategory::Observation);
        assert_eq!(obs_entries.len(), 2);
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut log = AuditLog::new(100);
        log.record(
            ActivityCategory::SettingsChange,
            "Changed UI port",
            "9876 → 3030",
        );
        let json = serde_json::to_string_pretty(&log).unwrap();
        let restored: AuditLog = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored.entries()[0].summary, "Changed UI port");
    }
}
