use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::companion_host::permissions::PermissionRegistry;
use crate::companion_host::privacy::PrivacyConfig;
use crate::errors::CoordinatorError;

/// All user-configurable settings for the companion.
///
/// Settings configure existing systems only — they do not introduce
/// new cognitive capabilities or modify the architecture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompanionSettings {
    /// LLM provider hint (e.g. "auto", "openai", "anthropic", "nvapi").
    pub llm_provider: String,

    /// Nvidia API token for NVAPI provider. Stored locally; not sent to cloud.
    pub nvapi_token: String,

    /// Observation source configuration.
    pub observation: ObservationSettings,

    /// Native notification preferences.
    pub notifications: NotificationSettings,

    /// Attention system sensitivity (0.0–1.0).
    pub attention_sensitivity: f64,

    /// How often the companion reflects on its own (seconds).
    pub reflection_frequency_secs: u64,

    /// Start the companion at user login.
    pub autostart: bool,

    /// Enable developer diagnostics mode.
    pub developer_mode: bool,

    /// Enable debug-level logging.
    pub debug_logging: bool,

    /// Path to the World Model storage file.
    pub wm_storage_path: PathBuf,

    /// TCP port for the web UI.
    pub ui_port: u16,

    /// Observation source permissions.
    pub permissions: PermissionRegistry,

    /// Privacy configuration (encryption, local-only, backup).
    pub privacy: PrivacyConfig,

    /// Data retention configuration.
    pub retention: RetentionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSettings {
    pub time_source: bool,
    pub system_source: bool,
    pub desktop_source: bool,
    pub interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub min_priority: String,
}

/// How long different types of data are retained.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// How long raw observations are kept before being summarized (seconds).
    pub observation_ttl_secs: u64,
    /// How long reflections are kept (seconds).
    pub reflection_ttl_secs: u64,
    /// Maximum number of audit log entries.
    pub audit_log_max_entries: usize,
    /// Whether to automatically archive low-importance entities.
    pub auto_archive: bool,
    /// Importance threshold below which entities may be archived.
    pub archive_threshold: f64,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            observation_ttl_secs: 7 * 86400, // 7 days
            reflection_ttl_secs: 30 * 86400, // 30 days
            audit_log_max_entries: 1000,
            auto_archive: false,
            archive_threshold: 0.2,
        }
    }
}

impl Default for CompanionSettings {
    fn default() -> Self {
        Self {
            llm_provider: "auto".into(),
            nvapi_token: String::new(),
            observation: ObservationSettings::default(),
            notifications: NotificationSettings::default(),
            attention_sensitivity: 0.5,
            reflection_frequency_secs: 60,
            autostart: false,
            developer_mode: false,
            debug_logging: false,
            wm_storage_path: PathBuf::from("~/.local/share/ai-os-companion/world_model.json"),
            ui_port: 9876,
            permissions: PermissionRegistry::new(),
            privacy: PrivacyConfig::default(),
            retention: RetentionConfig::default(),
        }
    }
}

impl Default for ObservationSettings {
    fn default() -> Self {
        Self {
            time_source: true,
            system_source: true,
            desktop_source: false,
            interval_secs: 30,
        }
    }
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            min_priority: "info".into(),
        }
    }
}

/// Manager for loading, saving, and applying companion settings.
#[derive(Debug)]
pub struct SettingsManager {
    pub settings: CompanionSettings,
    path: PathBuf,
}

impl SettingsManager {
    /// Load settings from the given path, or create defaults if the file
    /// does not exist.
    pub async fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let settings = match tokio::fs::read_to_string(&path).await {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            Err(_) => CompanionSettings::default(),
        };
        Self { path, settings }
    }

    /// Save the current settings to disk.
    pub fn save(&self) -> Result<(), CoordinatorError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CoordinatorError::Internal(format!("settings dir: {e}")))?;
        }
        let json = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| CoordinatorError::Internal(format!("settings serialize: {e}")))?;
        std::fs::write(&self.path, &json)
            .map_err(|e| CoordinatorError::Internal(format!("settings save: {e}")))?;
        Ok(())
    }

    /// Get the current settings.
    pub fn get(&self) -> &CompanionSettings {
        &self.settings
    }

    /// Update settings and save to disk.
    pub fn set(&mut self, new: CompanionSettings) -> Result<(), CoordinatorError> {
        self.settings = new;
        self.save()?;
        Ok(())
    }

    /// Path to the settings file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// A shared handle to SettingsManager for passing across async boundaries.
pub type DynSettingsManager = Arc<Mutex<SettingsManager>>;

/// Create a shared settings manager loaded from the default path.
pub async fn load_settings_manager() -> DynSettingsManager {
    let path = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
        .join(".config")
        .join("ai-os-companion")
        .join("settings.json");
    Arc::new(Mutex::new(SettingsManager::load(path).await))
}
