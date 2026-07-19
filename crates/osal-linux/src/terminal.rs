use std::collections::HashMap;
use std::ffi::CString;
use std::fmt;
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use osal_core::{
    Terminal, PtyHandle, TerminalError, Signal, OsalEvent,
};
use tokio::sync::{Mutex, mpsc};
use tokio::sync::mpsc::Receiver;

/// Internal state for an open PTY.
struct PtyState {
    master_fd: RawFd,
    slave_name: CString,
}

impl Drop for PtyState {
    fn drop(&mut self) {
        // SAFETY: close is safe to call on any valid file descriptor.
        // master_fd was opened by posix_openpt in open_pty and is guaranteed
        // to be a valid fd at that point. We never duplicate or close it
        // before dropping.
        unsafe { libc::close(self.master_fd); }
    }
}

/// Linux implementation of the [`Terminal`] trait using POSIX PTY APIs.
///
/// Each PTY is tracked by a unique identifier in an internal
/// `Arc<Mutex<HashMap<String, PtyState>>>`. All libc operations
/// are offloaded to `tokio::task::spawn_blocking` to avoid blocking
/// the async runtime.
pub struct LinuxTerminal {
    ptys: Arc<Mutex<HashMap<String, PtyState>>>,
    next_id: AtomicU64,
}

impl LinuxTerminal {
    pub fn new() -> Self {
        Self {
            ptys: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
        }
    }
}

impl fmt::Debug for LinuxTerminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxTerminal").finish()
    }
}

#[async_trait]
impl Terminal for LinuxTerminal {
    async fn open_pty(&self, _ctx: &CapabilityContext) -> Result<PtyHandle, TerminalError> {
        let (master_fd, slave_name) = tokio::task::spawn_blocking(|| {
            // SAFETY: posix_openpt allocates a new pseudo-terminal and returns
            // its master file descriptor. O_RDWR | O_NOCTTY are standard flags;
            // the kernel validates them. Returns -1 on error with errno set.
            let fd = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
            if fd < 0 {
                return Err(TerminalError::NotAvailable(format!(
                    "posix_openpt failed: {}",
                    std::io::Error::last_os_error()
                )));
            }

            // SAFETY: grantpt changes the ownership and permissions of the
            // slave pseudo-terminal device so that the caller's user can
            // open it. fd must be a master pty fd from posix_openpt.
            let ret = unsafe { libc::grantpt(fd) };
            if ret != 0 {
                // SAFETY: fd is still a valid master pty fd; close before returning error.
                unsafe { libc::close(fd); }
                return Err(TerminalError::NotAvailable(format!(
                    "grantpt failed: {}",
                    std::io::Error::last_os_error()
                )));
            }

            // SAFETY: unlockpt removes the lock on the slave device so it can
            // be opened. fd must be a master pty fd returned by posix_openpt.
            let ret = unsafe { libc::unlockpt(fd) };
            if ret != 0 {
                // SAFETY: fd is still a valid master pty fd; close before returning error.
                unsafe { libc::close(fd); }
                return Err(TerminalError::NotAvailable(format!(
                    "unlockpt failed: {}",
                    std::io::Error::last_os_error()
                )));
            }

            // SAFETY: ptsname returns a pointer to a static, null-terminated
            // string containing the path of the slave pty device (e.g.
            // "/dev/pts/5"). The returned pointer is valid until the next
            // call to ptsname with the same fd, and we only call it once here.
            let name_ptr = unsafe { libc::ptsname(fd) };
            if name_ptr.is_null() {
                // SAFETY: fd is still valid; close it before returning error.
                unsafe { libc::close(fd); }
                return Err(TerminalError::NotAvailable(format!(
                    "ptsname failed: {}",
                    std::io::Error::last_os_error()
                )));
            }

            // SAFETY: name_ptr is a valid pointer to a null-terminated string
            // containing only ASCII characters (a device path). It is safe to
            // construct a CStr from this pointer and convert it to a Rust string.
            let slave_name = unsafe { std::ffi::CStr::from_ptr(name_ptr) }
                .to_string_lossy()
                .into_owned();

            Ok((fd, slave_name))
        })
        .await
        .map_err(|e| TerminalError::Io(e.to_string()))??;

        let slave_cstring = CString::new(slave_name.as_str())
            .map_err(|_| TerminalError::Io("slave name contained null byte".to_string()))?;

        let id = format!("pty-{}", self.next_id.fetch_add(1, Ordering::SeqCst));

        let state = PtyState {
            master_fd,
            slave_name: slave_cstring,
        };

        self.ptys.lock().await.insert(id.clone(), state);

        Ok(PtyHandle { id, pid: None })
    }

    async fn write_pty(
        &self,
        _ctx: &CapabilityContext,
        id: &str,
        data: &[u8],
    ) -> Result<(), TerminalError> {
        let fd = {
            let ptys = self.ptys.lock().await;
            let state = ptys
                .get(id)
                .ok_or_else(|| TerminalError::NotFound(id.to_string()))?;
            state.master_fd
        };

        let owned_data = data.to_vec();

        tokio::task::spawn_blocking(move || {
            // SAFETY: fd is a valid master pty file descriptor (validated by
            // the PTY lookup above). owned_data is a valid buffer of known
            // length. write() reads up to owned_data.len() bytes from the
            // buffer and returns the number of bytes written (or -1 on error).
            let ret = unsafe {
                libc::write(
                    fd,
                    owned_data.as_ptr() as *const libc::c_void,
                    owned_data.len(),
                )
            };
            if ret < 0 {
                return Err(TerminalError::Io(
                    std::io::Error::last_os_error().to_string(),
                ));
            }
            Ok(())
        })
        .await
        .map_err(|e| TerminalError::Io(e.to_string()))?
    }

    async fn read_pty(
        &self,
        _ctx: &CapabilityContext,
        id: &str,
    ) -> Result<Vec<u8>, TerminalError> {
        let fd = {
            let ptys = self.ptys.lock().await;
            let state = ptys
                .get(id)
                .ok_or_else(|| TerminalError::NotFound(id.to_string()))?;
            state.master_fd
        };

        tokio::task::spawn_blocking(move || {
            let mut buf = vec![0u8; 4096];
            // SAFETY: fd is a valid master pty file descriptor. buf is a
            // mutable buffer of 4096 bytes. read() writes up to 4096 bytes
            // into the buffer and returns the number of bytes read (0 for EOF,
            // -1 on error).
            let ret = unsafe {
                libc::read(
                    fd,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if ret < 0 {
                return Err(TerminalError::Io(
                    std::io::Error::last_os_error().to_string(),
                ));
            }
            if ret == 0 {
                return Err(TerminalError::Disconnected);
            }
            buf.truncate(ret as usize);
            Ok(buf)
        })
        .await
        .map_err(|e| TerminalError::Io(e.to_string()))?
    }

    async fn resize_pty(
        &self,
        _ctx: &CapabilityContext,
        id: &str,
        rows: u16,
        cols: u16,
    ) -> Result<(), TerminalError> {
        let fd = {
            let ptys = self.ptys.lock().await;
            let state = ptys
                .get(id)
                .ok_or_else(|| TerminalError::NotFound(id.to_string()))?;
            state.master_fd
        };

        tokio::task::spawn_blocking(move || {
            // SAFETY: winsize is a plain-old-data struct from libc. Setting
            // terminal window size via TIOCSWINSZ on the master side
            // propagates the new dimensions to the slave side, which may
            // generate SIGWINCH for the foreground process group.
            let ws = libc::winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            // SAFETY: fd is a valid master pty fd. TIOCSWINSZ is the
            // correct ioctl request for setting terminal window dimensions.
            // &ws is a valid, properly aligned pointer to a winsize struct.
            let ret = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &ws) };
            if ret != 0 {
                return Err(TerminalError::Io(
                    std::io::Error::last_os_error().to_string(),
                ));
            }
            Ok(())
        })
        .await
        .map_err(|e| TerminalError::Io(e.to_string()))?
    }

    async fn signal_pty(
        &self,
        _ctx: &CapabilityContext,
        id: &str,
        signal: Signal,
    ) -> Result<(), TerminalError> {
        let slave_name = {
            let ptys = self.ptys.lock().await;
            let state = ptys
                .get(id)
                .ok_or_else(|| TerminalError::NotFound(id.to_string()))?;
            state.slave_name.clone()
        };

        tokio::task::spawn_blocking(move || {
            // SAFETY: open() is safe to call with a valid device path.
            // slave_name is the slave pty path obtained from ptsname() and
            // is guaranteed to be null-terminated. We open the slave side
            // in order to read the foreground process group via tcgetpgrp().
            let slave_fd = unsafe { libc::open(slave_name.as_ptr(), libc::O_RDWR) };
            if slave_fd < 0 {
                return Err(TerminalError::Io(format!(
                    "failed to open slave pty: {}",
                    std::io::Error::last_os_error()
                )));
            }

            // SAFETY: tcgetpgrp() reads the foreground process group ID from
            // the terminal referenced by slave_fd. slave_fd is a valid fd
            // to the slave side of the pty. Returns the pgid on success,
            // or -1 on error (with errno set).
            let pgid = unsafe { libc::tcgetpgrp(slave_fd) };

            // SAFETY: close() is safe to call on valid file descriptors.
            unsafe { libc::close(slave_fd); }

            if pgid < 0 {
                return Err(TerminalError::NotAvailable(
                    "no foreground process group on this pty".to_string(),
                ));
            }

            // SAFETY: kill(-pgid, signal.0) sends the given signal to every
            // process in process group pgid. A negative pid with kill(2)
            // indicates a process group. signal.0 is a valid signal number.
            let ret = unsafe { libc::kill(-pgid, signal.0) };
            if ret != 0 {
                return Err(TerminalError::Io(
                    std::io::Error::last_os_error().to_string(),
                ));
            }
            Ok(())
        })
        .await
        .map_err(|e| TerminalError::Io(e.to_string()))?
    }

    fn events(&self) -> Receiver<OsalEvent> {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        rx
    }
}
