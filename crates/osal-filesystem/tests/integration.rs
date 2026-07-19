use chrono::Utc;
use osal_core::{FileKind, FileSystem, FilesystemError};
use osal_filesystem::{DirEntry, FileEvent, FileMetadata, TempFile};
use std::path::PathBuf;

#[test]
fn test_file_kind_re_export() {
    let kinds = vec![
        FileKind::File,
        FileKind::Directory,
        FileKind::Symlink,
        FileKind::Socket,
        FileKind::Pipe,
        FileKind::BlockDevice,
        FileKind::CharDevice,
        FileKind::Other,
    ];
    assert_eq!(kinds.len(), 8);
}

#[test]
fn test_filesystem_error_is_accessible() {
    let err = FilesystemError::NotFound("/tmp/missing".into());
    assert!(err.to_string().contains("/tmp/missing"));
}

#[test]
fn test_file_system_trait_is_object_safe() {
    fn _take(_fs: &dyn FileSystem) {}
}

#[test]
fn test_file_event_osal_roundtrip() {
    let ts = Utc::now();
    let ev = FileEvent::Created {
        path: "/tmp/x".into(),
        timestamp: ts,
    };
    let osal = ev.to_osal_event().unwrap();
    match osal {
        osal_core::OsalEvent::FileCreated { path, .. } => {
            assert_eq!(path, "/tmp/x");
        }
        _ => panic!("expected FileCreated"),
    }
}

#[test]
fn test_file_event_try_from() {
    let ts = Utc::now();
    let ev = FileEvent::Deleted {
        path: "/tmp/d".into(),
        timestamp: ts,
    };
    let osal: Result<osal_core::OsalEvent, _> = ev.try_into();
    assert!(osal.is_ok());
}

#[test]
fn test_temp_file_cleanup_on_drop() {
    let tmp = std::env::temp_dir().join("osal_test_cleanup");
    std::fs::write(&tmp, b"hello").unwrap();
    assert!(tmp.exists());

    {
        let _tf = TempFile::new(tmp.clone());
        assert!(tmp.exists());
    }
    assert!(!tmp.exists());
}

#[test]
fn test_file_metadata_default() {
    let meta = FileMetadata {
        path: PathBuf::from("/tmp/test"),
        ..Default::default()
    };
    assert_eq!(meta.path, PathBuf::from("/tmp/test"));
    assert_eq!(meta.size, 0);
}

#[test]
fn test_dir_entry_serde() {
    let ts = Utc::now();
    let entry = DirEntry {
        name: "test.txt".into(),
        path: PathBuf::from("/tmp/test.txt"),
        file_type: FileKind::File,
        size: 123,
        modified: ts,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let deserialized: DirEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(entry.name, deserialized.name);
    assert_eq!(entry.path, deserialized.path);
    assert_eq!(entry.size, deserialized.size);
}

#[test]
fn test_file_metadata_serde() {
    let ts = Utc::now();
    let meta = FileMetadata {
        path: PathBuf::from("/tmp/meta"),
        size: 999,
        created: ts,
        modified: ts,
        accessed: ts,
        permissions: 0o755,
        file_type: FileKind::Directory,
        is_hidden: false,
    };
    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: FileMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(meta.path, deserialized.path);
    assert_eq!(meta.size, deserialized.size);
    assert_eq!(meta.permissions, deserialized.permissions);
    assert!(!deserialized.is_hidden);
}
