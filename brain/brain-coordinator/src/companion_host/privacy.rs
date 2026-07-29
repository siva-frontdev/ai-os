use ring::{rand::SecureRandom, rand::SystemRandom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::CoordinatorError;

/// Encryption status reported to the user.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EncryptionStatus {
    Enabled,
    Disabled,
    Broken,
}

/// Privacy configuration, stored alongside settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    /// Whether to encrypt the World Model and settings at rest.
    pub encrypt_storage: bool,

    /// Whether to operate in local-only mode (no network requests).
    pub local_only: bool,

    /// Number of backup copies to keep.
    pub backup_count: u32,

    /// Path to the backup directory.
    pub backup_path: PathBuf,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        Self {
            encrypt_storage: false,
            local_only: false,
            backup_count: 3,
            backup_path: PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("ai-os-companion")
                .join("backups"),
        }
    }
}

/// High-level privacy operations.
///
/// Integrates with existing storage paths and PersistenceManager.
/// No new storage systems — only adds encryption to existing files.
#[derive(Debug, Clone)]
pub struct PrivacyManager {
    key_path: PathBuf,
    config: PrivacyConfig,
}

impl PrivacyManager {
    /// Create a new PrivacyManager.
    pub fn new(config: PrivacyConfig) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let key_path = PathBuf::from(home)
            .join(".config")
            .join("ai-os-companion")
            .join(".encryption_key");
        Self { key_path, config }
    }

    /// Create a new PrivacyManager with a custom key path (for testing).
    pub fn new_in(config: PrivacyConfig, key_path: PathBuf) -> Self {
        Self { key_path, config }
    }

    /// Check whether encryption is currently enabled.
    pub fn is_encryption_enabled(&self) -> bool {
        self.key_path.exists()
    }

    /// Enable encryption by generating a key file.
    pub fn enable_encryption(&self) -> Result<(), CoordinatorError> {
        if self.key_path.exists() {
            return Ok(()); // already enabled
        }
        if let Some(parent) = self.key_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CoordinatorError::Internal(format!("key dir: {e}")))?;
        }
        // Generate a random 256-bit key
        let mut key_bytes = [0u8; 32];
        SystemRandom::new()
            .fill(&mut key_bytes)
            .map_err(|e| CoordinatorError::Internal(format!("key generation: {e}")))?;
        let key = key_bytes;
        std::fs::write(&self.key_path, &key)
            .map_err(|e| CoordinatorError::Internal(format!("key write: {e}")))?;
        // Restrict permissions to owner-only
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(perms) = std::fs::metadata(&self.key_path).map(|m| m.permissions()) {
                let mut p = perms;
                p.set_mode(0o400);
                let _ = std::fs::set_permissions(&self.key_path, p);
            }
        }
        Ok(())
    }

    /// Disable encryption by removing the key file.
    pub fn disable_encryption(&self) -> Result<(), CoordinatorError> {
        if self.key_path.exists() {
            std::fs::remove_file(&self.key_path)
                .map_err(|e| CoordinatorError::Internal(format!("key remove: {e}")))?;
        }
        Ok(())
    }

    /// Load the encryption key from disk.
    fn load_key(&self) -> Result<[u8; 32], CoordinatorError> {
        let data = std::fs::read(&self.key_path)
            .map_err(|e| CoordinatorError::Internal(format!("key read: {e}")))?;
        if data.len() != 32 {
            return Err(CoordinatorError::Internal("invalid key length".into()));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&data);
        Ok(key)
    }

    /// Encrypt data using AES-256-GCM via ring.
    /// Returns the nonce prepended to ciphertext.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CoordinatorError> {
        if !self.is_encryption_enabled() {
            return Ok(plaintext.to_vec());
        }
        let key = self.load_key()?;
        let key = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, &key)
            .map_err(|e| CoordinatorError::Internal(format!("aead key: {e}")))?;
        let mut nonce_bytes = [0u8; 12];
        SystemRandom::new()
            .fill(&mut nonce_bytes)
            .map_err(|e| CoordinatorError::Internal(format!("nonce: {e}")))?;
        let nonce_bytes_for_result = nonce_bytes;
        let nonce = ring::aead::Nonce::assume_unique_for_key(nonce_bytes);
        let mut in_out = plaintext.to_vec();
        let tag_len = ring::aead::AES_256_GCM.tag_len();
        in_out.reserve(tag_len);
        let sealing_key = ring::aead::LessSafeKey::new(key);
        sealing_key
            .seal_in_place_append_tag(nonce, ring::aead::Aad::empty(), &mut in_out)
            .map_err(|e| CoordinatorError::Internal(format!("encrypt: {e}")))?;
        let mut result = nonce_bytes_for_result.to_vec();
        result.extend_from_slice(&in_out);
        Ok(result)
    }

    /// Decrypt data encrypted with `encrypt()`.
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, CoordinatorError> {
        if !self.is_encryption_enabled() {
            return Ok(data.to_vec());
        }
        if data.len() < 12 {
            return Err(CoordinatorError::Internal(
                "truncated encrypted data".into(),
            ));
        }
        let key = self.load_key()?;
        let key = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, &key)
            .map_err(|e| CoordinatorError::Internal(format!("aead key: {e}")))?;
        let nonce = ring::aead::Nonce::assume_unique_for_key({
            let mut n = [0u8; 12];
            n.copy_from_slice(&data[..12]);
            n
        });
        let mut in_out = data[12..].to_vec();
        let opening_key = ring::aead::LessSafeKey::new(key);
        let plaintext = opening_key
            .open_in_place(nonce, ring::aead::Aad::empty(), &mut in_out)
            .map_err(|e| CoordinatorError::Internal(format!("decrypt: {e}")))?;
        Ok(plaintext.to_vec())
    }

    /// Return the current encryption status.
    pub fn encryption_status(&self) -> EncryptionStatus {
        if !self.key_path.exists() {
            return EncryptionStatus::Disabled;
        }
        let key = std::fs::read(&self.key_path);
        match key {
            Ok(k) if k.len() == 32 => EncryptionStatus::Enabled,
            _ => EncryptionStatus::Broken,
        }
    }

    /// Get the encryption key path.
    pub fn key_path(&self) -> &Path {
        &self.key_path
    }

    /// Get the privacy config.
    pub fn config(&self) -> &PrivacyConfig {
        &self.config
    }

    /// Create a backup of a file at the given path.
    pub fn create_backup(&self, file_path: &Path) -> Result<PathBuf, CoordinatorError> {
        if !file_path.exists() {
            return Err(CoordinatorError::Internal("file does not exist".into()));
        }
        let backup_dir = &self.config.backup_path;
        std::fs::create_dir_all(backup_dir)
            .map_err(|e| CoordinatorError::Internal(format!("backup dir: {e}")))?;

        let stem = file_path.file_stem().unwrap_or_default().to_string_lossy();
        let ext = file_path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let backup_name = if ext.is_empty() {
            format!("{stem}-{timestamp}.bak")
        } else {
            format!("{stem}-{timestamp}.{ext}.bak")
        };
        let backup_path = backup_dir.join(&backup_name);

        std::fs::copy(file_path, &backup_path)
            .map_err(|e| CoordinatorError::Internal(format!("backup copy: {e}")))?;

        // Rotate old backups
        Self::rotate_backups(backup_dir, self.config.backup_count);

        Ok(backup_path)
    }

    /// Rotate backups, keeping only the N most recent.
    fn rotate_backups(dir: &Path, keep: u32) {
        let mut entries: Vec<_> = match std::fs::read_dir(dir) {
            Ok(e) => e.filter_map(|e| e.ok()).collect(),
            Err(_) => return,
        };
        entries.sort_by_key(|e| e.path());
        let remove_count = entries.len().saturating_sub(keep as usize);
        for entry in entries.iter().take(remove_count) {
            let _ = std::fs::remove_file(entry.path());
        }
    }

    /// Restore the most recent backup of a file.
    pub fn restore_latest_backup(&self, file_path: &Path) -> Result<(), CoordinatorError> {
        let backup_dir = &self.config.backup_path;
        if !backup_dir.exists() {
            return Err(CoordinatorError::Internal("no backups found".into()));
        }
        let stem = file_path.file_stem().unwrap_or_default().to_string_lossy();

        let mut backups: Vec<_> = match std::fs::read_dir(backup_dir) {
            Ok(e) => e
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .file_name()
                        .map(|n| n.to_string_lossy().starts_with(stem.as_ref()))
                        .unwrap_or(false)
                })
                .collect(),
            Err(_) => return Err(CoordinatorError::Internal("no backups found".into())),
        };
        backups.sort_by_key(|e| e.path());
        if let Some(latest) = backups.last() {
            std::fs::copy(latest.path(), file_path)
                .map_err(|e| CoordinatorError::Internal(format!("restore copy: {e}")))?;
            Ok(())
        } else {
            Err(CoordinatorError::Internal("no backups found".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, PrivacyManager) {
        let dir = TempDir::new().unwrap();
        let mut config = PrivacyConfig::default();
        config.backup_path = dir.path().join("backups");
        config.encrypt_storage = true;
        let key_path = dir.path().join(".encryption_key");
        let mgr = PrivacyManager::new_in(config, key_path);
        (dir, mgr)
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let (_dir, mgr) = setup();
        mgr.enable_encryption().unwrap();

        let plaintext = b"This is sensitive companion data.";
        let encrypted = mgr.encrypt(plaintext).unwrap();
        assert_ne!(encrypted, plaintext);

        let decrypted = mgr.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_without_key_passthrough() {
        let (_dir, mgr) = setup();
        // No key → encryption is disabled
        let data = b"hello";
        let result = mgr.encrypt(data).unwrap();
        assert_eq!(result, data); // passthrough
    }

    #[test]
    fn test_encryption_status() {
        let (_dir, mgr) = setup();
        assert_eq!(mgr.encryption_status(), EncryptionStatus::Disabled);
        mgr.enable_encryption().unwrap();
        assert_eq!(mgr.encryption_status(), EncryptionStatus::Enabled);
        mgr.disable_encryption().unwrap();
        assert_eq!(mgr.encryption_status(), EncryptionStatus::Disabled);
    }

    #[test]
    fn test_key_file_permissions() {
        let (_dir, mgr) = setup();
        mgr.enable_encryption().unwrap();
        assert!(mgr.key_path().exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(mgr.key_path()).unwrap();
            let mode = meta.permissions().mode();
            assert_eq!(
                mode & 0o077,
                0,
                "key file should have no group/world access"
            );
        }
    }

    #[test]
    fn test_backup_create_and_restore() {
        let (dir, mgr) = setup();
        let test_file = dir.path().join("test.txt");
        std::fs::write(&test_file, b"important data").unwrap();

        let backup_path = mgr.create_backup(&test_file).unwrap();
        assert!(backup_path.exists());

        // Modify original
        std::fs::write(&test_file, b"corrupted").unwrap();

        // Restore
        mgr.restore_latest_backup(&test_file).unwrap();
        let restored = std::fs::read_to_string(&test_file).unwrap();
        assert_eq!(restored, "important data");
    }

    #[test]
    fn test_backup_rotation() {
        let (dir, _mgr) = setup();
        let test_file = dir.path().join("rotate.txt");
        std::fs::write(&test_file, "data").unwrap();

        let mut config = PrivacyConfig::default();
        config.backup_count = 2;
        config.backup_path = dir.path().join("backups");
        let backup_path = config.backup_path.clone();
        let mgr = PrivacyManager::new(config);
        mgr.create_backup(&test_file).unwrap();
        mgr.create_backup(&test_file).unwrap();
        mgr.create_backup(&test_file).unwrap();
        let count = std::fs::read_dir(&backup_path).unwrap().count();
        assert!(count <= 2, "backup count should not exceed backup_count");
    }
}
