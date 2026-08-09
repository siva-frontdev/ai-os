//! SQLite connection wrapper.
//!
//! SQLite is always accessed from `tokio::task::spawn_blocking` so the async
//! runtime is never blocked. A single connection behind a `tokio::sync::Mutex`
//! serialises writers (SQLite single-writer) and gives WAL snapshot isolation
//! for reads at personal scale (RFC-0008 §Thread Model).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::Connection;

use crate::error::{WorldStoreError, WorldStoreResult};
use crate::sql::SCHEMA;

/// Inner connection state (replaced during restore, opened lazily at `init`).
#[derive(Debug)]
struct Inner {
    conn: Option<Connection>,
    path: PathBuf,
    wal: bool,
}

/// A shared, blocking-friendly SQLite connection.
#[derive(Debug, Clone)]
pub(crate) struct Db {
    inner: Arc<tokio::sync::Mutex<Inner>>,
}

fn open_connection(path: &Path, wal: bool) -> WorldStoreResult<Connection> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| WorldStoreError::Io(format!("create db dir {}: {e}", parent.display())))?;
    }
    let conn = Connection::open(path)
        .map_err(|e| WorldStoreError::Storage(format!("open {}: {e}", path.display())))?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;\
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(|e| WorldStoreError::Storage(format!("pragma setup: {e}")))?;
    if wal {
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| WorldStoreError::Storage(format!("enable WAL: {e}")))?;
    }
    Ok(conn)
}

impl Db {
    /// Create the wrapper without opening the database yet (opened by
    /// [`Db::ensure_open`], typically from the store's `init`).
    pub(crate) fn unopened(path: PathBuf, wal: bool) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(Inner {
                conn: None,
                path,
                wal,
            })),
        }
    }

    /// Open the database and ensure the schema exists (idempotent).
    pub(crate) async fn ensure_open(&self) -> WorldStoreResult<()> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = inner.blocking_lock();
            if guard.conn.is_none() {
                let conn = open_connection(&guard.path, guard.wal)?;
                conn.execute_batch(SCHEMA)
                    .map_err(|e| WorldStoreError::Storage(format!("apply schema: {e}")))?;
                guard.conn = Some(conn);
            }
            Ok(())
        })
        .await
        .map_err(|e| WorldStoreError::Storage(format!("blocking task failed: {e}")))?
    }

    /// Open a read/verify connection to `path` without applying the schema
    /// (used by restore verification so a backup is not mutated).
    pub(crate) fn open_verify_only(path: &Path, wal: bool) -> WorldStoreResult<Self> {
        let conn = open_connection(path, wal)?;
        Ok(Self {
            inner: Arc::new(tokio::sync::Mutex::new(Inner {
                conn: Some(conn),
                path: path.to_path_buf(),
                wal,
            })),
        })
    }

    /// Run a read operation on the connection off the async runtime.
    pub(crate) async fn run<T, F>(&self, f: F) -> WorldStoreResult<T>
    where
        F: FnOnce(&Connection) -> WorldStoreResult<T> + Send + 'static,
        T: Send + 'static,
    {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = inner.blocking_lock();
            let conn = guard
                .conn
                .as_ref()
                .ok_or_else(|| WorldStoreError::Storage("database connection is closed".into()))?;
            f(conn)
        })
        .await
        .map_err(|e| WorldStoreError::Storage(format!("blocking task failed: {e}")))?
    }

    /// Run a write operation on the connection off the async runtime.
    pub(crate) async fn run_mut<T, F>(&self, f: F) -> WorldStoreResult<T>
    where
        F: FnOnce(&mut Connection) -> WorldStoreResult<T> + Send + 'static,
        T: Send + 'static,
    {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = inner.blocking_lock();
            let conn = guard
                .conn
                .as_mut()
                .ok_or_else(|| WorldStoreError::Storage("database connection is closed".into()))?;
            f(conn)
        })
        .await
        .map_err(|e| WorldStoreError::Storage(format!("blocking task failed: {e}")))?
    }

    /// Checkpoint the WAL and close the connection.
    pub(crate) async fn close(&self) -> WorldStoreResult<()> {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = inner.blocking_lock();
            if let Some(conn) = guard.conn.take() {
                let _ = conn.pragma_update(None, "wal_checkpoint", "TRUNCATE");
            }
            Ok(())
        })
        .await
        .map_err(|e| WorldStoreError::Storage(format!("blocking task failed: {e}")))?
    }

    /// Replace the live database file with `backup_file` and reopen.
    ///
    /// Used by restore: the current connection is checkpointed and closed, the
    /// WAL/SHM sidecars are removed, the backup is copied over the primary
    /// path, and a fresh connection is opened.
    pub(crate) async fn swap_file(&self, backup_file: &Path) -> WorldStoreResult<()> {
        let inner = self.inner.clone();
        let backup_file = backup_file.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let mut guard = inner.blocking_lock();
            let conn = guard
                .conn
                .take()
                .ok_or_else(|| WorldStoreError::Storage("database connection is closed".into()))?;
            let _ = conn.pragma_update(None, "wal_checkpoint", "TRUNCATE");
            drop(conn);

            let path = guard.path.clone();
            let wal_path = db_sidecar(&path, "-wal");
            let shm_path = db_sidecar(&path, "-shm");
            if wal_path.exists() {
                std::fs::remove_file(&wal_path)
                    .map_err(|e| WorldStoreError::Io(format!("remove {wal_path:?}: {e}")))?;
            }
            if shm_path.exists() {
                std::fs::remove_file(&shm_path)
                    .map_err(|e| WorldStoreError::Io(format!("remove {shm_path:?}: {e}")))?;
            }
            std::fs::copy(&backup_file, &path).map_err(|e| {
                WorldStoreError::Io(format!(
                    "copy backup {} over {}: {e}",
                    backup_file.display(),
                    path.display()
                ))
            })?;
            let conn = open_connection(&path, guard.wal)?;
            guard.conn = Some(conn);
            Ok(())
        })
        .await
        .map_err(|e| WorldStoreError::Storage(format!("blocking task failed: {e}")))?
    }
}

/// Compute a SQLite sidecar path (`<path>-wal` / `<path>-shm`).
fn db_sidecar(db_path: &Path, suffix: &str) -> PathBuf {
    let mut s = db_path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}
