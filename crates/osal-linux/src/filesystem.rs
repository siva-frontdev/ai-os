use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use osal_capabilities::CapabilityContext;
use tokio::fs;
use tokio::sync::mpsc::{self, Receiver};

use osal_core::{DirEntry, FileKind, FileMetadata, FileSystem, FilesystemError, OsalEvent, Gid, Permissions, Uid};

pub struct LinuxFileSystem;

impl LinuxFileSystem {
    pub fn new() -> Self {
        Self
    }
}

impl std::fmt::Debug for LinuxFileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxFileSystem").finish()
    }
}

fn io_error_to_filesystem(e: std::io::Error, path: &str) -> FilesystemError {
    match e.kind() {
        std::io::ErrorKind::NotFound => FilesystemError::NotFound(path.to_string()),
        std::io::ErrorKind::PermissionDenied => FilesystemError::PermissionDenied(path.to_string()),
        std::io::ErrorKind::AlreadyExists => FilesystemError::AlreadyExists(path.to_string()),
        _ => FilesystemError::Io(e.to_string()),
    }
}

fn file_kind_from_ft(ft: std::fs::FileType) -> FileKind {
    if ft.is_file() {
        FileKind::File
    } else if ft.is_dir() {
        FileKind::Directory
    } else if ft.is_symlink() {
        FileKind::Symlink
    } else if ft.is_socket() {
        FileKind::Socket
    } else if ft.is_fifo() {
        FileKind::Pipe
    } else if ft.is_block_device() {
        FileKind::BlockDevice
    } else if ft.is_char_device() {
        FileKind::CharDevice
    } else {
        FileKind::Other
    }
}

fn system_time_to_utc(t: std::time::SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(t)
}

#[async_trait]
impl FileSystem for LinuxFileSystem {
    #[allow(unused_variables)]
    async fn read(&self, ctx: &CapabilityContext, path: &str) -> Result<Vec<u8>, FilesystemError> {
        fs::read(path).await.map_err(|e| io_error_to_filesystem(e, path))
    }

    #[allow(unused_variables)]
    async fn write(&self, ctx: &CapabilityContext, path: &str, data: &[u8]) -> Result<(), FilesystemError> {
        fs::write(path, data).await.map_err(|e| io_error_to_filesystem(e, path))
    }

    #[allow(unused_variables)]
    async fn delete(&self, ctx: &CapabilityContext, path: &str) -> Result<(), FilesystemError> {
        match fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::IsADirectory => {
                fs::remove_dir(path).await.map_err(|e| io_error_to_filesystem(e, path))
            }
            Err(e) => Err(io_error_to_filesystem(e, path)),
        }
    }

    #[allow(unused_variables)]
    async fn create_dir(&self, ctx: &CapabilityContext, path: &str) -> Result<(), FilesystemError> {
        fs::create_dir_all(path).await.map_err(|e| io_error_to_filesystem(e, path))
    }

    #[allow(unused_variables)]
    async fn metadata(&self, ctx: &CapabilityContext, path: &str) -> Result<FileMetadata, FilesystemError> {
        let meta = fs::metadata(path).await.map_err(|e| io_error_to_filesystem(e, path))?;

        let kind = file_kind_from_ft(meta.file_type());
        let mode = meta.permissions().mode();
        let modified = meta.modified().map(system_time_to_utc).unwrap_or_else(|_| Utc::now());
        let created = meta.created().map(system_time_to_utc).unwrap_or_else(|_| Utc::now());

        Ok(FileMetadata {
            path: path.to_string(),
            size: meta.len(),
            kind,
            permissions: Permissions(mode & 0o7777),
            modified,
            created,
            owner: Uid(meta.uid()),
            group: Gid(meta.gid()),
        })
    }

    #[allow(unused_variables)]
    async fn list(&self, ctx: &CapabilityContext, path: &str) -> Result<Vec<DirEntry>, FilesystemError> {
        let mut rd = fs::read_dir(path).await.map_err(|e| io_error_to_filesystem(e, path))?;
        let mut entries = Vec::new();

        while let Some(entry) = rd.next_entry().await.map_err(|e| io_error_to_filesystem(e, path))? {
            let file_type = entry.file_type().await.map_err(|e| io_error_to_filesystem(e, path))?;
            let kind = file_kind_from_ft(file_type);
            let name = entry.file_name().to_string_lossy().to_string();
            let entry_path = entry.path().to_string_lossy().to_string();

            entries.push(DirEntry {
                name,
                path: entry_path,
                kind,
            });
        }

        Ok(entries)
    }

    #[allow(unused_variables)]
    async fn watch(&self, ctx: &CapabilityContext, path: &str) -> Result<Receiver<OsalEvent>, FilesystemError> {
        use inotify::{EventMask, Inotify, WatchMask};

        let (tx, rx) = mpsc::channel(256);
        let watch_path = path.to_string();

        let mut inotify = Inotify::init().map_err(|e| FilesystemError::Io(e.to_string()))?;

        inotify
            .watches()
            .add(
                &watch_path,
                WatchMask::CREATE
                    | WatchMask::MODIFY
                    | WatchMask::DELETE
                    | WatchMask::MOVED_FROM
                    | WatchMask::MOVED_TO,
            )
            .map_err(|e| FilesystemError::Io(e.to_string()))?;

        tokio::task::spawn_blocking(move || {
            let mut buffer = [0u8; 4096];
            loop {
                let events = match inotify.read_events_blocking(&mut buffer) {
                    Ok(events) => events,
                    Err(_) => break,
                };

                for event in events {
                    let name = event.name.unwrap_or_default().to_string_lossy();
                    let target = if name.is_empty() {
                        watch_path.clone()
                    } else {
                        format!("{}/{}", watch_path, name)
                    };

                    let timestamp = Utc::now();

                    let osal_event = if event.mask.contains(EventMask::CREATE) {
                        OsalEvent::FileCreated {
                            path: target,
                            file_type: "unknown".to_string(),
                            timestamp,
                        }
                    } else if event.mask.contains(EventMask::MODIFY) {
                        OsalEvent::FileModified {
                            path: target,
                            size: 0,
                            timestamp,
                        }
                    } else if event.mask.contains(EventMask::DELETE)
                        || event.mask.contains(EventMask::MOVED_FROM)
                    {
                        OsalEvent::FileDeleted {
                            path: target,
                            timestamp,
                        }
                    } else if event.mask.contains(EventMask::MOVED_TO) {
                        OsalEvent::FileCreated {
                            path: target,
                            file_type: "unknown".to_string(),
                            timestamp,
                        }
                    } else {
                        continue;
                    };

                    if tx.blocking_send(osal_event).is_err() {
                        break;
                    }
                }
            }
        });

        Ok(rx)
    }
}
