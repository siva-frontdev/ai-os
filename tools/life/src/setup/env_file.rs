//! Minimal, secure `.env` file persistence.
//!
//! The wizard stores provider credentials in a gitignored `.env` file (see
//! `runtime/config/.env`). This module reads existing `KEY=VALUE` lines,
//! preserves unrelated keys, and writes with restrictive permissions so
//! secrets are not world-readable.

use std::fs;
use std::path::Path;

use crate::error::SetupError;

/// An ordered map of environment entries backed by a `.env` file.
#[derive(Debug, Clone, Default)]
pub struct EnvFile {
    /// Entries in file order; later writes replace earlier keys.
    entries: Vec<(String, String)>,
}

impl EnvFile {
    /// Load a `.env` file. A missing file loads as empty.
    pub fn load(path: &Path) -> Result<Self, SetupError> {
        let mut entries = Vec::new();
        if path.exists() {
            let content = fs::read_to_string(path)
                .map_err(|e| SetupError::persist(format!("read {}: {e}", path.display())))?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    entries.push((key.trim().to_string(), value.trim().to_string()));
                }
            }
        }
        Ok(Self { entries })
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Set or replace a value, preserving existing file position.
    pub fn set(&mut self, key: &str, value: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value.to_string();
        } else {
            self.entries.push((key.to_string(), value.to_string()));
        }
    }

    /// Persist to `path`, creating the parent directory if needed.
    ///
    /// Writes the file with owner-only permissions (`0600`) so credential
    /// values are never exposed to other local users.
    pub fn write(&self, path: &Path) -> Result<(), SetupError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                SetupError::persist(format!("create directory {}: {e}", parent.display()))
            })?;
        }
        let mut content =
            String::from("# AI-OS provider credentials — DO NOT COMMIT (gitignored).\n");
        for (key, value) in &self.entries {
            content.push_str(&format!("{key}={value}\n"));
        }
        fs::write(path, content)
            .map_err(|e| SetupError::persist(format!("write {}: {e}", path.display())))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| {
                SetupError::persist(format!("set permissions on {}: {e}", path.display()))
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_parses_key_value_lines() {
        let dir = std::env::temp_dir().join(format!("envfile-load-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        fs::write(&path, "# comment\nALPHA=one\n\nBETA=two\nALPHA=replaced\n").unwrap();
        let env = EnvFile::load(&path).unwrap();
        assert_eq!(env.get("ALPHA"), Some("replaced"));
        assert_eq!(env.get("BETA"), Some("two"));
        assert_eq!(env.get("MISSING"), None);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_missing_file_is_empty() {
        let dir = std::env::temp_dir().join(format!("envfile-missing-{}", uuid::Uuid::new_v4()));
        let path = dir.join(".env");
        let env = EnvFile::load(&path).unwrap();
        assert_eq!(env.get("ALPHA"), None);
    }

    #[test]
    fn set_then_write_roundtrips() {
        let dir = std::env::temp_dir().join(format!("envfile-write-{}", uuid::Uuid::new_v4()));
        let path = dir.join(".env");
        let mut env = EnvFile::default();
        env.set("AIOS_GMAIL_CLIENT_ID", "cid");
        env.set("AIOS_GMAIL_REFRESH_TOKEN", "rtok");
        env.write(&path).unwrap();

        let loaded = EnvFile::load(&path).unwrap();
        assert_eq!(loaded.get("AIOS_GMAIL_CLIENT_ID"), Some("cid"));
        assert_eq!(loaded.get("AIOS_GMAIL_REFRESH_TOKEN"), Some("rtok"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "file must be owner-only");
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_preserves_unrelated_keys() {
        let dir = std::env::temp_dir().join(format!("envfile-keep-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        fs::write(&path, "OTHER=stay\n").unwrap();
        let mut env = EnvFile::load(&path).unwrap();
        env.set("AIOS_GMAIL_REFRESH_TOKEN", "rtok");
        env.write(&path).unwrap();
        let loaded = EnvFile::load(&path).unwrap();
        assert_eq!(loaded.get("OTHER"), Some("stay"));
        assert_eq!(loaded.get("AIOS_GMAIL_REFRESH_TOKEN"), Some("rtok"));
        fs::remove_dir_all(&dir).ok();
    }
}
