use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use osal_core::FileKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub size: u64,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub accessed: DateTime<Utc>,
    pub permissions: u32,
    pub file_type: FileKind,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub file_type: FileKind,
    pub size: u64,
    pub modified: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TempFile {
    pub path: PathBuf,
}

impl TempFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_file_new() {
        let tf = TempFile::new(PathBuf::from("/tmp/test.txt"));
        assert_eq!(tf.path(), &PathBuf::from("/tmp/test.txt"));
    }

    #[test]
    fn test_temp_file_path_returns_ref() {
        let tf = TempFile::new(PathBuf::from("/tmp/foo"));
        assert_eq!(tf.path(), &PathBuf::from("/tmp/foo"));
    }

    #[test]
    fn test_file_metadata_construction() {
        let ts = Utc::now();
        let meta = FileMetadata {
            path: PathBuf::from("/home/user/doc.txt"),
            size: 1024,
            created: ts,
            modified: ts,
            accessed: ts,
            permissions: 0o644,
            file_type: FileKind::File,
            is_hidden: false,
        };
        assert_eq!(meta.path, PathBuf::from("/home/user/doc.txt"));
        assert_eq!(meta.size, 1024);
        assert_eq!(meta.permissions, 0o644);
        assert_eq!(meta.file_type, FileKind::File);
        assert!(!meta.is_hidden);
    }

    #[test]
    fn test_file_metadata_hidden_file() {
        let meta = FileMetadata {
            path: PathBuf::from("/home/user/.hidden"),
            is_hidden: true,
            ..Default::default()
        };
        assert!(meta.is_hidden);
    }

    #[test]
    fn test_dir_entry_construction() {
        let ts = Utc::now();
        let entry = DirEntry {
            name: "doc.txt".into(),
            path: PathBuf::from("/home/user/doc.txt"),
            file_type: FileKind::File,
            size: 4096,
            modified: ts,
        };
        assert_eq!(entry.name, "doc.txt");
        assert_eq!(entry.size, 4096);
        assert_eq!(entry.file_type, FileKind::File);
    }

    #[test]
    fn test_dir_entry_directory_type() {
        let ts = Utc::now();
        let entry = DirEntry {
            name: "subdir".into(),
            path: PathBuf::from("/home/user/subdir"),
            file_type: FileKind::Directory,
            size: 0,
            modified: ts,
        };
        assert_eq!(entry.file_type, FileKind::Directory);
    }

    #[test]
    fn test_file_kind_variants_from_osal_core() {
        let _file = FileKind::File;
        let _dir = FileKind::Directory;
        let _symlink = FileKind::Symlink;
        let _socket = FileKind::Socket;
        let _pipe = FileKind::Pipe;
        let _block = FileKind::BlockDevice;
        let _char = FileKind::CharDevice;
        let _other = FileKind::Other;
    }

    #[test]
    fn test_temp_file_equality() {
        let a = TempFile::new(PathBuf::from("/tmp/x"));
        let b = TempFile::new(PathBuf::from("/tmp/x"));
        assert_eq!(a.path, b.path);
    }
}

impl Default for FileMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            path: PathBuf::new(),
            size: 0,
            created: now,
            modified: now,
            accessed: now,
            permissions: 0o644,
            file_type: FileKind::File,
            is_hidden: false,
        }
    }
}
