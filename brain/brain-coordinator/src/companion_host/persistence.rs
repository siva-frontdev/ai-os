use std::path::{Path, PathBuf};

use memory_core::wm::Entity;
use serde::{Deserialize, Serialize};

/// Manages World Model persistence to disk.
///
/// Handles loading a previously saved World Model from a JSON file
/// and saving the current World Model state back to disk.
/// Uses atomic writes (write to temp file, then rename) to prevent
/// data corruption on crash.
pub struct PersistenceManager {
    wm_path: PathBuf,
}

impl PersistenceManager {
    /// Create a new PersistenceManager for the given file path.
    pub fn new(wm_path: impl AsRef<Path>) -> Self {
        Self {
            wm_path: wm_path.as_ref().to_path_buf(),
        }
    }

    /// Load the World Model from disk.
    ///
    /// Returns the deserialized world model data, or an error
    /// if the file exists but cannot be read or parsed.
    /// Returns `Ok(None)` if the file does not exist (cold start).
    pub async fn load(
        &self,
    ) -> Result<Option<PersistedWorldModel>, crate::errors::CoordinatorError> {
        if !self.wm_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read_to_string(&self.wm_path).map_err(|e| {
            crate::errors::CoordinatorError::Internal(format!("Failed to read WM file: {e}"))
        })?;
        let persisted: PersistedWorldModel = serde_json::from_str(&data).map_err(|e| {
            crate::errors::CoordinatorError::Internal(format!("Failed to parse WM file: {e}"))
        })?;

        Ok(Some(persisted))
    }

    /// Save the World Model to disk.
    ///
    /// Performs an atomic write via temp file + rename to prevent
    /// corruption if the process is interrupted mid-write.
    pub async fn save(
        &self,
        entities: &[Entity],
        relationships: &[memory_core::wm::Relationship],
    ) -> Result<(), crate::errors::CoordinatorError> {
        let persisted = PersistedWorldModel {
            entities: entities.to_vec(),
            relationships: relationships.to_vec(),
            saved_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let json = serde_json::to_string_pretty(&persisted).map_err(|e| {
            crate::errors::CoordinatorError::Internal(format!("Failed to serialize WM: {e}"))
        })?;

        let tmp_path = self.wm_path.with_extension("json.tmp");
        if let Some(parent) = self.wm_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::errors::CoordinatorError::Internal(format!(
                    "Failed to create WM directory: {e}"
                ))
            })?;
        }
        std::fs::write(&tmp_path, json).map_err(|e| {
            crate::errors::CoordinatorError::Internal(format!("Failed to write WM temp file: {e}"))
        })?;
        std::fs::rename(&tmp_path, &self.wm_path).map_err(|e| {
            crate::errors::CoordinatorError::Internal(format!("Failed to rename WM file: {e}"))
        })?;

        Ok(())
    }

    /// Return the path this manager writes to.
    pub fn path(&self) -> &Path {
        &self.wm_path
    }
}

/// Serialized World Model for file persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistedWorldModel {
    pub entities: Vec<Entity>,
    pub relationships: Vec<memory_core::wm::Relationship>,
    pub saved_at: u64,
}
