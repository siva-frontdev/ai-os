use std::path::PathBuf;
use chrono::{DateTime, Utc};
use osal_core::OsalEvent;

#[derive(Debug, Clone)]
pub enum FileEvent {
    Created { path: PathBuf, timestamp: DateTime<Utc> },
    Modified { path: PathBuf, timestamp: DateTime<Utc> },
    Deleted { path: PathBuf, timestamp: DateTime<Utc> },
    Renamed { from: PathBuf, to: PathBuf, timestamp: DateTime<Utc> },
    MetadataChanged { path: PathBuf, timestamp: DateTime<Utc> },
}

impl FileEvent {
    pub fn to_osal_event(&self) -> Option<OsalEvent> {
        match self {
            FileEvent::Created { path, timestamp } => Some(OsalEvent::FileCreated {
                path: path.to_string_lossy().to_string(),
                file_type: "file".into(),
                timestamp: *timestamp,
            }),
            FileEvent::Modified { path, timestamp } => Some(OsalEvent::FileModified {
                path: path.to_string_lossy().to_string(),
                size: 0,
                timestamp: *timestamp,
            }),
            FileEvent::Deleted { path, timestamp } => Some(OsalEvent::FileDeleted {
                path: path.to_string_lossy().to_string(),
                timestamp: *timestamp,
            }),
            FileEvent::Renamed { .. } | FileEvent::MetadataChanged { .. } => None,
        }
    }
}

impl TryFrom<FileEvent> for OsalEvent {
    type Error = &'static str;

    fn try_from(event: FileEvent) -> Result<Self, Self::Error> {
        event.to_osal_event().ok_or("FileEvent::Renamed and FileEvent::MetadataChanged have no OsalEvent equivalent")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(kind: &str) -> FileEvent {
        let ts = Utc::now();
        match kind {
            "created" => FileEvent::Created { path: "/tmp/f".into(), timestamp: ts },
            "modified" => FileEvent::Modified { path: "/tmp/f".into(), timestamp: ts },
            "deleted" => FileEvent::Deleted { path: "/tmp/f".into(), timestamp: ts },
            "renamed" => FileEvent::Renamed { from: "/tmp/a".into(), to: "/tmp/b".into(), timestamp: ts },
            "metadata" => FileEvent::MetadataChanged { path: "/tmp/f".into(), timestamp: ts },
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_created_converts_to_osal() {
        let ev = make_event("created");
        let osal = ev.to_osal_event().unwrap();
        assert!(matches!(osal, OsalEvent::FileCreated { .. }));
    }

    #[test]
    fn test_modified_converts_to_osal() {
        let ev = make_event("modified");
        let osal = ev.to_osal_event().unwrap();
        assert!(matches!(osal, OsalEvent::FileModified { .. }));
    }

    #[test]
    fn test_deleted_converts_to_osal() {
        let ev = make_event("deleted");
        let osal = ev.to_osal_event().unwrap();
        assert!(matches!(osal, OsalEvent::FileDeleted { .. }));
    }

    #[test]
    fn test_renamed_does_not_convert() {
        let ev = make_event("renamed");
        assert!(ev.to_osal_event().is_none());
    }

    #[test]
    fn test_metadata_changed_does_not_convert() {
        let ev = make_event("metadata");
        assert!(ev.to_osal_event().is_none());
    }

    #[test]
    fn test_try_from_created() {
        let ev = make_event("created");
        let osal: OsalEvent = ev.try_into().unwrap();
        assert!(matches!(osal, OsalEvent::FileCreated { .. }));
    }

    #[test]
    fn test_try_from_renamed_errors() {
        let ev = make_event("renamed");
        let result: Result<OsalEvent, _> = ev.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_created_path_roundtrip() {
        let ev = FileEvent::Created { path: "/home/test/file.txt".into(), timestamp: Utc::now() };
        let osal = ev.to_osal_event().unwrap();
        if let OsalEvent::FileCreated { path, .. } = osal {
            assert_eq!(path, "/home/test/file.txt");
        } else {
            panic!("expected FileCreated");
        }
    }

    #[test]
    fn test_modified_path_roundtrip() {
        let ev = FileEvent::Modified { path: "/var/log/syslog".into(), timestamp: Utc::now() };
        let osal = ev.to_osal_event().unwrap();
        if let OsalEvent::FileModified { path, .. } = osal {
            assert_eq!(path, "/var/log/syslog");
        } else {
            panic!("expected FileModified");
        }
    }

    #[test]
    fn test_deleted_path_roundtrip() {
        let ev = FileEvent::Deleted { path: "/tmp/foo".into(), timestamp: Utc::now() };
        let osal = ev.to_osal_event().unwrap();
        if let OsalEvent::FileDeleted { path, .. } = osal {
            assert_eq!(path, "/tmp/foo");
        } else {
            panic!("expected FileDeleted");
        }
    }

    #[test]
    fn test_renamed_from_to_fields() {
        let ev = FileEvent::Renamed { from: "/tmp/a".into(), to: "/tmp/b".into(), timestamp: Utc::now() };
        assert!(ev.to_osal_event().is_none());
        if let FileEvent::Renamed { from, to, .. } = ev {
            assert_eq!(from, PathBuf::from("/tmp/a"));
            assert_eq!(to, PathBuf::from("/tmp/b"));
        } else {
            panic!("expected Renamed");
        }
    }

    #[test]
    fn test_metadata_changed_path() {
        let ev = FileEvent::MetadataChanged { path: "/etc/config".into(), timestamp: Utc::now() };
        assert!(ev.to_osal_event().is_none());
        if let FileEvent::MetadataChanged { path, .. } = ev {
            assert_eq!(path, PathBuf::from("/etc/config"));
        } else {
            panic!("expected MetadataChanged");
        }
    }

    #[test]
    fn test_all_variants_have_timestamps() {
        let ts = Utc::now();
        let events = vec![
            FileEvent::Created { path: "/f".into(), timestamp: ts },
            FileEvent::Modified { path: "/f".into(), timestamp: ts },
            FileEvent::Deleted { path: "/f".into(), timestamp: ts },
            FileEvent::Renamed { from: "/a".into(), to: "/b".into(), timestamp: ts },
            FileEvent::MetadataChanged { path: "/f".into(), timestamp: ts },
        ];
        for ev in &events {
            if let Some(osal) = ev.to_osal_event() {
                match osal {
                    OsalEvent::FileCreated { timestamp, .. }
                    | OsalEvent::FileModified { timestamp, .. }
                    | OsalEvent::FileDeleted { timestamp, .. } => {
                        assert_eq!(timestamp, ts);
                    }
                    _ => panic!("unexpected event"),
                }
            }
        }
    }
}
