# osal-filesystem — OSAL Filesystem Subsystem

Architecture-first trait definitions for the filesystem abstraction layer. Every operation is capability-gated.

---

## Trait: `FileSystem`

The primary trait. All methods are async and require a `CapabilityContext` for authorization.

```rust
#[async_trait]
pub trait FileSystem: Debug + Send + Sync {
    /// Read file contents at path. Returns bytes.
    /// Requires Capability::FileRead(path).
    async fn read(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<Vec<u8>>;

    /// Write bytes to file at path. Creates parent directories if needed.
    /// Requires Capability::FileWrite(path).
    async fn write(&self, path: &Path, data: &[u8], ctx: &CapabilityContext) -> OsalResult<()>;

    /// Append bytes to file.
    /// Requires Capability::FileWrite(path).
    async fn append(&self, path: &Path, data: &[u8], ctx: &CapabilityContext) -> OsalResult<()>;

    /// Read file metadata (size, permissions, timestamps, type).
    /// Requires Capability::FileMetadata(path).
    async fn metadata(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<FileMetadata>;

    /// List directory contents.
    /// Requires Capability::FileRead(path).
    async fn list(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<Vec<DirEntry>>;

    /// Create a directory and all parents.
    /// Requires Capability::FileWrite(path).
    async fn create_dir(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<()>;

    /// Remove a file.
    /// Requires Capability::FileDelete(path).
    async fn remove(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<()>;

    /// Remove a directory recursively.
    /// Requires Capability::FileDelete(path).
    async fn remove_dir(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<()>;

    /// Copy file from source to destination.
    /// Requires Capability::FileRead(src) and Capability::FileWrite(dst).
    async fn copy(&self, src: &Path, dst: &Path, ctx: &CapabilityContext) -> OsalResult<()>;

    /// Move (rename) file.
    /// Requires Capability::FileWrite(src) and Capability::FileWrite(dst).
    async fn move_file(&self, src: &Path, dst: &Path, ctx: &CapabilityContext) -> OsalResult<()>;

    /// Change file permissions.
    /// Requires Capability::FileWrite(path).
    async fn set_permissions(&self, path: &Path, perms: u32, ctx: &CapabilityContext) -> OsalResult<()>;

    /// Watch a path for changes. Returns a Receiver that yields FileEvents.
    /// Requires Capability::FileWatch(path).
    async fn watch(&self, path: &Path, ctx: &CapabilityContext) -> OsalResult<Receiver<FileEvent>>;

    /// Create a temporary file with the given prefix.
    /// Requires Capability::FileWrite(temp_dir).
    async fn temp_file(&self, prefix: &str, ctx: &CapabilityContext) -> OsalResult<TempFile>;

    /// Check if a path exists. Read-only, no capability required,
    /// but the path must be accessible.
    async fn exists(&self, path: &Path) -> OsalResult<bool>;
}
```

---

## Data Types

### `FileMetadata`

Describes a single filesystem entry.

```rust
pub struct FileMetadata {
    pub path: PathBuf,
    pub size: u64,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub accessed: DateTime<Utc>,
    pub permissions: u32,        // Unix permission bits
    pub file_type: FileType,
    pub is_hidden: bool,
}
```

### `DirEntry`

A single entry in a directory listing.

```rust
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub file_type: FileType,
    pub size: u64,
    pub modified: DateTime<Utc>,
}
```

### `FileType`

Classification of a filesystem entry.

```rust
pub enum FileType {
    File,
    Directory,
    Symlink,
    Other,
}
```

### `TempFile`

A temporary file that is automatically cleaned up on drop.

```rust
pub struct TempFile {
    pub path: PathBuf,
}

impl TempFile {
    pub fn new(path: PathBuf) -> Self;
    pub fn path(&self) -> &PathBuf;
}

impl Drop for TempFile {
    /// Removes the file from disk.
    fn drop(&mut self);
}
```

---

## Events

### `FileEvent`

Events emitted by the `watch()` method.

```rust
pub enum FileEvent {
    Created { path: PathBuf, timestamp: DateTime<Utc> },
    Modified { path: PathBuf, timestamp: DateTime<Utc> },
    Deleted { path: PathBuf, timestamp: DateTime<Utc> },
    Renamed { from: PathBuf, to: PathBuf, timestamp: DateTime<Utc> },
    MetadataChanged { path: PathBuf, timestamp: DateTime<Utc> },
}
```

---

## Error Types

### `FilesystemError`

```rust
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FilesystemError {
    NotFound(String),
    PermissionDenied(String),
    AlreadyExists(String),
    NotEmpty(String),
    NotADirectory(String),
    IsADirectory(String),
    ReadOnlyFilesystem(String),
    DiskFull(String),
    IoError(String),
    InvalidPath(String),
    WatchLimitExceeded(String),
}
```

Converts into `osal_core::OsalError::Filesystem` for uniform error handling up the stack.

---

## Capability Mapping

Every method checks a capability from `Capability` (from `osal-capabilities`) against the `CapabilityContext` before proceeding. Path-based capabilities support wildcard matching via `"*"`.

| Method                  | Capability Required              |
|------------------------|----------------------------------|
| `read`                 | `FileRead(path)`                 |
| `write`                | `FileWrite(path)`                |
| `append`               | `FileWrite(path)`                |
| `metadata`             | `FileMetadata(path)`             |
| `list`                 | `FileRead(path)`                 |
| `create_dir`           | `FileWrite(path)`                |
| `remove`               | `FileDelete(path)`               |
| `remove_dir`           | `FileDelete(path)`               |
| `copy`                 | `FileRead(src)` + `FileWrite(dst)`|
| `move_file`            | `FileWrite(src)` + `FileWrite(dst)`|
| `set_permissions`      | `FileWrite(path)`                |
| `watch`                | `FileWatch(path)`                |
| `temp_file`            | `FileWrite(temp_dir)`            |
| `exists`               | (none — accessible to all)       |

---

## Architecture

```
┌─────────────────────────────────────────────┐
│            osal-filesystem crate            │
│                                             │
│  traits.rs   types.rs   error.rs   events.rs│
│      │          │          │           │     │
│      └──────────┴──────────┴───────────┘     │
│                     │                        │
│              depends on:                     │
│         osal-core, osal-capabilities          │
└─────────────────────────────────────────────┘
                      │
                      ▼
        ┌─────────────────────────┐
        │     osal-linux crate     │ (implementor)
        │  (concrete impl using   │
        │   std::fs, inotify, etc.)│
        └─────────────────────────┘
```

The `FileSystem` trait is not the same as the simplified version in `osal_core::KernelFacade`. The facade uses a minimal subset; this crate defines the full production interface. Implementations in `osal-linux` should implement both.
