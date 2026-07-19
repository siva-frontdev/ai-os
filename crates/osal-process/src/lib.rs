#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OSAL process subsystem — extended types and default implementations for
//! process lifecycle management on top of [`osal_core`].
//!
//! # Overview
//!
//! This crate re-exports the core process abstractions from [`osal_core`]
//! and adds process-specific types that are too detailed for the core layer:
//!
//! | Type | Purpose |
//! |---|---|
//! | [`ProcessConfig`] | Rich spawn configuration (env, I/O, security) |
//! | [`ProcessHandle`] | Active child with I/O stream handles |
//! | [`ProcessState`] | Typed lifecycle states (Running, Sleeping, …) |
//! | [`ProcessStatus`] | Full process snapshot with exit status |
//! | [`DefaultProcessManager`] | Fallback no-op implementation |
//!
//! # Re-exports from `osal_core`
//!
//! * [`ProcessManager`](osal_core::ProcessManager) — trait for OS process management
//! * [`ProcessError`](osal_core::ProcessError) — process-related error variants
//! * [`ChildHandle`](osal_core::ChildHandle) — lightweight process identifier
//! * [`ProcessInfo`](osal_core::ProcessInfo) — basic process snapshot from the OS
//! * [`OsalEvent`](osal_core::OsalEvent) — system-wide event enum (includes
//!   `ProcessStarted` / `ProcessExited`)

/// Extended process types unique to this crate.
pub mod types;

pub use osal_core::{ChildHandle, OsalEvent, ProcessError, ProcessInfo, ProcessManager};

pub use types::{ProcessConfig, ProcessHandle, ProcessState, ProcessStatus};

use std::fmt::Debug;

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use osal_core::{ExitStatus, Pid, Signal};
use tokio::sync::mpsc;

/// No-op implementation of [`ProcessManager`] that rejects every operation.
///
/// Every method returns [`ProcessError::NotAllowed`] with a descriptive
/// message indicating the default implementation does not support the
/// operation. The `events()` method returns a closed channel.
///
/// This is useful as a default or fallback in configurations where no
/// real process manager has been wired in.
#[derive(Debug, Clone)]
pub struct DefaultProcessManager;

#[async_trait]
impl ProcessManager for DefaultProcessManager {
    async fn spawn(
        &self,
        _ctx: &CapabilityContext,
        _command: &str,
        _args: &[&str],
    ) -> Result<ChildHandle, ProcessError> {
        Err(ProcessError::NotAllowed(
            "DefaultProcessManager: spawn not available".into(),
        ))
    }

    async fn kill(
        &self,
        _ctx: &CapabilityContext,
        _pid: Pid,
        _signal: Signal,
    ) -> Result<(), ProcessError> {
        Err(ProcessError::NotAllowed(
            "DefaultProcessManager: kill not available".into(),
        ))
    }

    async fn suspend(&self, _ctx: &CapabilityContext, _pid: Pid) -> Result<(), ProcessError> {
        Err(ProcessError::NotAllowed(
            "DefaultProcessManager: suspend not available".into(),
        ))
    }

    async fn resume(&self, _ctx: &CapabilityContext, _pid: Pid) -> Result<(), ProcessError> {
        Err(ProcessError::NotAllowed(
            "DefaultProcessManager: resume not available".into(),
        ))
    }

    async fn list(&self, _ctx: &CapabilityContext) -> Result<Vec<ProcessInfo>, ProcessError> {
        Err(ProcessError::NotAllowed(
            "DefaultProcessManager: list not available".into(),
        ))
    }

    async fn wait(&self, _ctx: &CapabilityContext, _pid: Pid) -> Result<ExitStatus, ProcessError> {
        Err(ProcessError::NotAllowed(
            "DefaultProcessManager: wait not available".into(),
        ))
    }

    fn events(&self) -> mpsc::Receiver<OsalEvent> {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> CapabilityContext {
        CapabilityContext::new("test")
    }

    #[tokio::test]
    async fn test_default_spawn_returns_not_allowed() {
        let pm = DefaultProcessManager;
        let ctx = test_context();
        let result = pm.spawn(&ctx, "ls", &["-la"]).await;
        assert!(matches!(result, Err(ProcessError::NotAllowed(_))));
    }

    #[tokio::test]
    async fn test_default_kill_returns_not_allowed() {
        let pm = DefaultProcessManager;
        let ctx = test_context();
        let result = pm.kill(&ctx, Pid(1), Signal(9)).await;
        assert!(matches!(result, Err(ProcessError::NotAllowed(_))));
    }

    #[tokio::test]
    async fn test_default_suspend_returns_not_allowed() {
        let pm = DefaultProcessManager;
        let ctx = test_context();
        let result = pm.suspend(&ctx, Pid(1)).await;
        assert!(matches!(result, Err(ProcessError::NotAllowed(_))));
    }

    #[tokio::test]
    async fn test_default_resume_returns_not_allowed() {
        let pm = DefaultProcessManager;
        let ctx = test_context();
        let result = pm.resume(&ctx, Pid(1)).await;
        assert!(matches!(result, Err(ProcessError::NotAllowed(_))));
    }

    #[tokio::test]
    async fn test_default_list_returns_not_allowed() {
        let pm = DefaultProcessManager;
        let ctx = test_context();
        let result = pm.list(&ctx).await;
        assert!(matches!(result, Err(ProcessError::NotAllowed(_))));
    }

    #[tokio::test]
    async fn test_default_wait_returns_not_allowed() {
        let pm = DefaultProcessManager;
        let ctx = test_context();
        let result = pm.wait(&ctx, Pid(1)).await;
        assert!(matches!(result, Err(ProcessError::NotAllowed(_))));
    }

    #[tokio::test]
    async fn test_default_events_returns_closed_receiver() {
        let pm = DefaultProcessManager;
        let mut rx = pm.events();
        assert!(rx.recv().await.is_none());
    }

    #[test]
    fn test_default_manager_debug_and_clone() {
        let pm = DefaultProcessManager;
        let pm2 = pm.clone();
        let _dbg = format!("{:?}", pm2);
    }

    #[test]
    fn test_process_config_serde_roundtrip() {
        use std::collections::HashMap;

        let config = ProcessConfig {
            command: "cat".into(),
            args: vec!["/etc/hostname".into()],
            env: HashMap::new(),
            working_dir: None,
            uid: Some(osal_core::Uid(1000)),
            gid: None,
            timeout: None,
            capabilities: osal_capabilities::CapabilitySet::new(),
            stdin: None,
            stdout: None,
            stderr: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: ProcessConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.command, "cat");
        assert_eq!(deserialized.args, vec!["/etc/hostname"]);
    }

    #[test]
    fn test_process_state_debug_and_eq() {
        assert_eq!(ProcessState::Running, ProcessState::Running);
        assert_ne!(ProcessState::Running, ProcessState::Dead);
        let _ = format!("{:?}", ProcessState::Sleeping);
    }

    #[test]
    fn test_process_status_serde_roundtrip() {
        let status = ProcessStatus {
            pid: Pid(42),
            state: ProcessState::Running,
            exit_status: None,
            cpu_usage: 2.5,
            memory_usage: 1024,
            user: osal_core::Uid(1000),
            running_time: std::time::Duration::from_secs(60),
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: ProcessStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.pid.0, 42);
        assert_eq!(deserialized.state, ProcessState::Running);
    }
}
