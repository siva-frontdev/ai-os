use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::sync::Arc;

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use osal_core::{
    ChildHandle, ExitStatus, OsalEvent, Pid, ProcessError, ProcessInfo, ProcessManager, Signal, Uid,
};
use tokio::sync::{mpsc, Mutex};

/// Linux implementation of `ProcessManager`.
///
/// Spawned child processes are tracked in an `Arc<Mutex<HashMap<u64, tokio::process::Child>>>`
/// so that `wait()` can await their exit. Processes not spawned by this manager
/// are waited on via `libc::waitpid()`.
pub struct LinuxProcessManager {
    children: Arc<Mutex<HashMap<u64, tokio::process::Child>>>,
}

impl LinuxProcessManager {
    pub fn new() -> Self {
        Self {
            children: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl fmt::Debug for LinuxProcessManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinuxProcessManager").finish()
    }
}

#[async_trait]
impl ProcessManager for LinuxProcessManager {
    async fn spawn(
        &self,
        _ctx: &CapabilityContext,
        command: &str,
        args: &[&str],
    ) -> Result<ChildHandle, ProcessError> {
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args);

        // SAFETY: pre_exec runs in the child process after fork() but before exec().
        // We create a new process group so the child is independent for job control.
        // This is safe because we only call setpgid(0, 0) which is async-signal-safe.
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }

        let child = cmd
            .spawn()
            .map_err(|e| ProcessError::ExecutionFailed(format!("failed to spawn {}: {}", command, e)))?;

        let pid = child.id().ok_or_else(|| {
            ProcessError::ExecutionFailed("child process id not available".to_string())
        })? as u64;

        let mut map = self.children.lock().await;
        map.insert(pid, child);

        Ok(ChildHandle {
            pid: Pid(pid),
        })
    }

    async fn kill(
        &self,
        _ctx: &CapabilityContext,
        pid: Pid,
        signal: Signal,
    ) -> Result<(), ProcessError> {
        if signal.0 == 0 {
            return Err(ProcessError::InvalidSignal(signal));
        }
        let pid_i32 = pid.0 as i32;
        let sig = signal.0;
        tokio::task::spawn_blocking(move || {
            // SAFETY: libc::kill is an FFI call to the POSIX kill syscall.
            // It is safe when called with a valid PID and signal number.
            // PID 0 targets the process group, which is valid.
            let ret = unsafe { libc::kill(pid_i32, sig) };
            if ret == -1 {
                let err = io::Error::last_os_error();
                match err.raw_os_error() {
                    Some(libc::ESRCH) => Err(ProcessError::NotFound(pid)),
                    Some(libc::EPERM) | Some(libc::EACCES) => {
                        Err(ProcessError::NotAllowed(format!(
                            "permission denied to signal pid {}: {}",
                            pid_i32, err
                        )))
                    }
                    Some(libc::EINVAL) => Err(ProcessError::InvalidSignal(signal)),
                    _ => Err(ProcessError::Io(format!(
                        "kill({}, {}): {}",
                        pid_i32, sig, err
                    ))),
                }
            } else {
                Ok(())
            }
        })
        .await
        .map_err(|e| ProcessError::Io(format!("spawn_blocking join: {}", e)))?
    }

    async fn suspend(
        &self,
        _ctx: &CapabilityContext,
        pid: Pid,
    ) -> Result<(), ProcessError> {
        let pid_i32 = pid.0 as i32;
        tokio::task::spawn_blocking(move || {
            // SAFETY: libc::kill with SIGSTOP is safe per POSIX semantics.
            // SIGSTOP cannot be caught, ignored, or blocked, so the target
            // will always stop unless the PID is invalid or permission denied.
            let ret = unsafe { libc::kill(pid_i32, libc::SIGSTOP) };
            if ret == -1 {
                let err = io::Error::last_os_error();
                match err.raw_os_error() {
                    Some(libc::ESRCH) => Err(ProcessError::NotFound(pid)),
                    Some(libc::EPERM) | Some(libc::EACCES) => {
                        Err(ProcessError::NotAllowed(format!(
                            "permission denied to suspend pid {}: {}",
                            pid_i32, err
                        )))
                    }
                    _ => Err(ProcessError::Io(format!("suspend({}): {}", pid_i32, err))),
                }
            } else {
                Ok(())
            }
        })
        .await
        .map_err(|e| ProcessError::Io(format!("spawn_blocking join: {}", e)))?
    }

    async fn resume(
        &self,
        _ctx: &CapabilityContext,
        pid: Pid,
    ) -> Result<(), ProcessError> {
        let pid_i32 = pid.0 as i32;
        tokio::task::spawn_blocking(move || {
            // SAFETY: libc::kill with SIGCONT is safe per POSIX semantics.
            // SIGCONT resumes a stopped process.
            let ret = unsafe { libc::kill(pid_i32, libc::SIGCONT) };
            if ret == -1 {
                let err = io::Error::last_os_error();
                match err.raw_os_error() {
                    Some(libc::ESRCH) => Err(ProcessError::NotFound(pid)),
                    Some(libc::EPERM) | Some(libc::EACCES) => {
                        Err(ProcessError::NotAllowed(format!(
                            "permission denied to resume pid {}: {}",
                            pid_i32, err
                        )))
                    }
                    _ => Err(ProcessError::Io(format!("resume({}): {}", pid_i32, err))),
                }
            } else {
                Ok(())
            }
        })
        .await
        .map_err(|e| ProcessError::Io(format!("spawn_blocking join: {}", e)))?
    }

    async fn list(
        &self,
        _ctx: &CapabilityContext,
    ) -> Result<Vec<ProcessInfo>, ProcessError> {
        let entries = fs::read_dir("/proc")
            .map_err(|e| ProcessError::Io(format!("read /proc: {}", e)))?;

        let mut processes = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| ProcessError::Io(format!("read /proc entry: {}", e)))?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let pid: u64 = match name_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            match read_process_info(pid) {
                Ok(info) => processes.push(info),
                Err(_) => continue,
            }
        }

        Ok(processes)
    }

    async fn wait(
        &self,
        _ctx: &CapabilityContext,
        pid: Pid,
    ) -> Result<ExitStatus, ProcessError> {
        let mut map = self.children.lock().await;
        let child = map.remove(&pid.0);
        drop(map);

        if let Some(mut child) = child {
            let status = child
                .wait()
                .await
                .map_err(|e| ProcessError::Io(format!("wait for pid {}: {}", pid.0, e)))?;
            return Ok(convert_exit_status(status));
        }

        // Not a child we spawned — use waitpid
        let pid_i32 = pid.0 as i32;
        tokio::task::spawn_blocking(move || {
            let mut wstatus: i32 = 0;
            // SAFETY: libc::waitpid is an FFI call to the POSIX waitpid syscall.
            // It blocks until the child process with the given PID exits.
            // The wstatus pointer must be valid (not null), which it is.
            let ret = unsafe { libc::waitpid(pid_i32, &mut wstatus as *mut i32, 0) };
            if ret == -1 {
                let err = io::Error::last_os_error();
                match err.raw_os_error() {
                    Some(libc::ESRCH) | Some(libc::ECHILD) => {
                        Err(ProcessError::NotFound(pid))
                    }
                    Some(libc::EINTR) => Err(ProcessError::Io(format!(
                        "waitpid({}) interrupted by signal",
                        pid_i32
                    ))),
                    _ => Err(ProcessError::Io(format!(
                        "waitpid({}): {}",
                        pid_i32, err
                    ))),
                }
            } else {
                Ok(convert_wstatus(wstatus))
            }
        })
        .await
        .map_err(|e| ProcessError::Io(format!("spawn_blocking join: {}", e)))?
    }

    fn events(&self) -> mpsc::Receiver<OsalEvent> {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        rx
    }
}

/// Convert a `std::process::ExitStatus` (from tokio's `Child::wait`) to `ExitStatus`.
fn convert_exit_status(status: std::process::ExitStatus) -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            ExitStatus(None, Some(Signal(signal)))
        } else {
            ExitStatus(status.code(), None)
        }
    }
    #[cfg(not(unix))]
    {
        ExitStatus(status.code(), None)
    }
}

/// Convert a raw `wstatus` integer from `libc::waitpid` to `ExitStatus`.
fn convert_wstatus(wstatus: i32) -> ExitStatus {
    if libc::WIFEXITED(wstatus) {
        ExitStatus(Some(libc::WEXITSTATUS(wstatus)), None)
    } else if libc::WIFSIGNALED(wstatus) {
        ExitStatus(None, Some(Signal(libc::WTERMSIG(wstatus))))
    } else if libc::WIFSTOPPED(wstatus) {
        ExitStatus(None, Some(Signal(libc::WSTOPSIG(wstatus))))
    } else {
        ExitStatus(None, None)
    }
}

/// Read `/proc/[pid]/cmdline`, replacing null separators with spaces.
fn read_cmdline(pid: u64) -> Result<String, io::Error> {
    let path = format!("/proc/{}/cmdline", pid);
    let mut buf = Vec::new();
    fs::File::open(&path)?.read_to_end(&mut buf)?;

    while buf.last() == Some(&0) {
        buf.pop();
    }

    let cmdline = buf
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect::<Vec<_>>()
        .join(" ");

    Ok(cmdline)
}

/// Parse `/proc/[pid]/stat` and return `(state, parent_pid, utime, stime)`.
fn parse_stat(pid: u64) -> Result<(String, u64, u64, u64), io::Error> {
    let path = format!("/proc/{}/stat", pid);
    let content = fs::read_to_string(&path)?;

    // The comm field (field 2) is enclosed in parentheses and may contain
    // parentheses itself. Find the last ") " to correctly delimit it.
    let paren_end = content
        .rfind(") ")
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("malformed /proc/{}/stat: no closing paren", pid),
            )
        })?;

    let rest = &content[paren_end + 2..];
    let fields: Vec<&str> = rest.split_whitespace().collect();

    let state = fields.first().unwrap_or(&"?").to_string();
    let ppid: u64 = fields.get(1).unwrap_or(&"0").parse().unwrap_or(0);
    let utime: u64 = fields.get(11).unwrap_or(&"0").parse().unwrap_or(0);
    let stime: u64 = fields.get(12).unwrap_or(&"0").parse().unwrap_or(0);

    Ok((state, ppid, utime, stime))
}

/// Parse `/proc/[pid]/status` and return `(user, memory_bytes)`.
fn parse_status(pid: u64) -> Result<(Uid, u64), io::Error> {
    let path = format!("/proc/{}/status", pid);
    let content = fs::read_to_string(&path)?;

    let mut uid = Uid(0);
    let mut memory_kb: u64 = 0;

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("Uid:") {
            let parts: Vec<&str> = val.trim().split_whitespace().collect();
            if let Some(first) = parts.first() {
                uid = Uid(first.parse().unwrap_or(0));
            }
        } else if let Some(val) = line.strip_prefix("VmRSS:") {
            let parts: Vec<&str> = val.trim().split_whitespace().collect();
            if let Some(kb_str) = parts.first() {
                memory_kb = kb_str.parse().unwrap_or(0);
            }
        }
    }

    Ok((uid, memory_kb * 1024))
}

/// Read all available `/proc` information for a given PID.
fn read_process_info(pid: u64) -> Result<ProcessInfo, ProcessError> {
    let cmdline = read_cmdline(pid).unwrap_or_default();
    let (state, ppid, _utime, _stime) = parse_stat(pid)
        .map_err(|e| ProcessError::Io(format!("read /proc/{}/stat: {}", pid, e)))?;
    let (user, memory) = parse_status(pid)
        .map_err(|e| ProcessError::Io(format!("read /proc/{}/status: {}", pid, e)))?;

    Ok(ProcessInfo {
        pid: Pid(pid),
        parent_pid: if ppid > 0 { Some(Pid(ppid)) } else { None },
        command: cmdline,
        user,
        state,
        cpu_usage: 0.0,
        memory_usage: memory,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_ctx() -> CapabilityContext {
        CapabilityContext::new("test")
    }

    #[tokio::test]
    async fn test_spawn_and_wait_own_child() {
        let pm = LinuxProcessManager::new();
        let ctx = dummy_ctx();
        let handle = pm.spawn(&ctx, "true", &[]).await.unwrap();
        let status = pm.wait(&ctx, handle.pid).await.unwrap();
        assert!(status.success());
    }

    async fn spawn_kill_cleanup(pm: &LinuxProcessManager, ctx: &CapabilityContext, sleep_secs: &str) {
        let handle = pm.spawn(ctx, "sleep", &[sleep_secs]).await.unwrap();
        pm.kill(ctx, handle.pid, Signal(9)).await.unwrap();
        let status = pm.wait(ctx, handle.pid).await.unwrap();
        assert_eq!(status.terminating_signal(), Some(Signal(9)));
    }

    #[tokio::test]
    async fn test_spawn_and_kill() {
        let pm = LinuxProcessManager::new();
        let ctx = dummy_ctx();
        spawn_kill_cleanup(&pm, &ctx, "5").await;
    }

    #[tokio::test]
    async fn test_suspend_and_resume() {
        let pm = LinuxProcessManager::new();
        let ctx = dummy_ctx();
        let handle = pm.spawn(&ctx, "sleep", &["5"]).await.unwrap();

        pm.suspend(&ctx, handle.pid).await.unwrap();
        pm.resume(&ctx, handle.pid).await.unwrap();

        // Clean up
        pm.kill(&ctx, handle.pid, Signal(9)).await.unwrap();
        pm.wait(&ctx, handle.pid).await.unwrap();
    }

    #[tokio::test]
    async fn test_kill_invalid_signal() {
        let pm = LinuxProcessManager::new();
        let ctx = dummy_ctx();
        let result = pm.kill(&ctx, Pid(1), Signal(0)).await;
        assert!(matches!(result.unwrap_err(), ProcessError::InvalidSignal(_)));
    }

    #[tokio::test]
    async fn test_kill_nonexistent_pid() {
        let pm = LinuxProcessManager::new();
        let ctx = dummy_ctx();
        let result = pm.kill(&ctx, Pid(999_999_999), Signal(9)).await;
        assert!(matches!(result.unwrap_err(), ProcessError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_wait_nonexistent_pid() {
        let pm = LinuxProcessManager::new();
        let ctx = dummy_ctx();
        let result = pm.wait(&ctx, Pid(999_999_999)).await;
        assert!(matches!(result.unwrap_err(), ProcessError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_list_contains_init() {
        let pm = LinuxProcessManager::new();
        let ctx = dummy_ctx();
        let processes = pm.list(&ctx).await.unwrap();
        // PID 1 (init/systemd) should always exist
        assert!(
            processes.iter().any(|p| p.pid.0 == 1),
            "expected PID 1 in process list"
        );
    }

    #[tokio::test]
    async fn test_events_returns_closed_receiver() {
        let pm = LinuxProcessManager::new();
        let mut rx = pm.events();
        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn test_spawn_fails_on_empty_command() {
        let pm = LinuxProcessManager::new();
        let ctx = dummy_ctx();
        let result = pm.spawn(&ctx, "", &[]).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_wstatus_exit() {
        // Simulate exit(42)
        let wstatus = 42 << 8;
        let status = convert_wstatus(wstatus);
        assert_eq!(status.exit_code(), Some(42));
        assert!(status.terminating_signal().is_none());
    }

    #[test]
    fn test_convert_wstatus_signal() {
        // Simulate SIGKILL (9) termination
        let wstatus = 9;
        let status = convert_wstatus(wstatus);
        assert!(status.exit_code().is_none());
        assert_eq!(status.terminating_signal(), Some(Signal(9)));
    }
}
