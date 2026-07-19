#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OSAL terminal subsystem — command execution, PTY management, and terminal I/O.
//!
//! This crate provides types and a no-op default implementation for the
//! operating system's terminal/PTY abstraction. The [`Terminal`] trait itself
//! is defined in [`osal_core`] and re-exported here for convenience.
//!
//! # Unique types
//!
//! Types defined locally (not in `osal_core`) include:
//!
//! - [`CommandConfig`] / [`CommandOutput`] — batch command execution
//! - [`PtyConfig`] / [`PtySession`] — interactive PTY session management
//! - [`TerminalOutput`] / [`TerminalOutputEventType`] — PTY output events
//!
//! # Default implementation
//!
//! [`DefaultTerminal`] is a no-op implementation of [`Terminal`] that returns
//! [`TerminalError::NotAvailable`] for every operation. Use this when no real
//! terminal backend is configured (e.g. during testing or headless operation).

pub mod types;

pub use osal_core::{PtyHandle, Terminal, TerminalError};
pub use types::{
    CommandConfig, CommandOutput, PtyConfig, PtySession, TerminalOutput, TerminalOutputEventType,
};

use async_trait::async_trait;
use osal_capabilities::CapabilityContext;
use osal_core::{OsalEvent, Signal};
use tokio::sync::mpsc;

/// No-op implementation of [`Terminal`] that always returns
/// [`TerminalError::NotAvailable`].
///
/// Useful as a placeholder when no real terminal backend is configured
/// or in early boot stages before the OSAL platform initializes.
#[derive(Debug)]
pub struct DefaultTerminal;

#[async_trait]
impl Terminal for DefaultTerminal {
    async fn open_pty(&self, _ctx: &CapabilityContext) -> Result<PtyHandle, TerminalError> {
        Err(TerminalError::NotAvailable("open_pty".into()))
    }

    async fn write_pty(
        &self,
        _ctx: &CapabilityContext,
        _id: &str,
        _data: &[u8],
    ) -> Result<(), TerminalError> {
        Err(TerminalError::NotAvailable("write_pty".into()))
    }

    async fn read_pty(
        &self,
        _ctx: &CapabilityContext,
        _id: &str,
    ) -> Result<Vec<u8>, TerminalError> {
        Err(TerminalError::NotAvailable("read_pty".into()))
    }

    async fn resize_pty(
        &self,
        _ctx: &CapabilityContext,
        _id: &str,
        _rows: u16,
        _cols: u16,
    ) -> Result<(), TerminalError> {
        Err(TerminalError::NotAvailable("resize_pty".into()))
    }

    async fn signal_pty(
        &self,
        _ctx: &CapabilityContext,
        _id: &str,
        _signal: Signal,
    ) -> Result<(), TerminalError> {
        Err(TerminalError::NotAvailable("signal_pty".into()))
    }

    fn events(&self) -> mpsc::Receiver<OsalEvent> {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        rx
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn ctx() -> CapabilityContext {
        CapabilityContext::new("test")
    }

    fn is_not_available(result: &TerminalError, method: &str) -> bool {
        matches!(result, TerminalError::NotAvailable(m) if m == method)
    }

    #[tokio::test]
    async fn default_terminal_open_pty_returns_not_available() {
        let t = DefaultTerminal;
        let result = t.open_pty(&ctx()).await;
        assert!(result.is_err(), "open_pty should return error");
        assert!(is_not_available(result.err().as_ref().unwrap(), "open_pty"));
    }

    #[tokio::test]
    async fn default_terminal_write_pty_returns_not_available() {
        let t = DefaultTerminal;
        let result = t.write_pty(&ctx(), "pty-1", b"data").await;
        assert!(result.is_err());
        assert!(is_not_available(
            result.err().as_ref().unwrap(),
            "write_pty"
        ));
    }

    #[tokio::test]
    async fn default_terminal_read_pty_returns_not_available() {
        let t = DefaultTerminal;
        let result = t.read_pty(&ctx(), "pty-1").await;
        assert!(result.is_err());
        assert!(is_not_available(result.err().as_ref().unwrap(), "read_pty"));
    }

    #[tokio::test]
    async fn default_terminal_resize_pty_returns_not_available() {
        let t = DefaultTerminal;
        let result = t.resize_pty(&ctx(), "pty-1", 24, 80).await;
        assert!(result.is_err());
        assert!(is_not_available(
            result.err().as_ref().unwrap(),
            "resize_pty"
        ));
    }

    #[tokio::test]
    async fn default_terminal_signal_pty_returns_not_available() {
        let t = DefaultTerminal;
        let result = t.signal_pty(&ctx(), "pty-1", Signal(9)).await;
        assert!(result.is_err());
        assert!(is_not_available(
            result.err().as_ref().unwrap(),
            "signal_pty"
        ));
    }

    #[tokio::test]
    async fn default_terminal_events_returns_closed_receiver() {
        let t = DefaultTerminal;
        let mut rx = t.events();
        assert!(
            rx.recv().await.is_none(),
            "events receiver should be closed"
        );
    }

    #[test]
    fn command_config_serde_roundtrip() {
        let config = CommandConfig {
            command: "ls".into(),
            args: vec!["-la".into()],
            env: [("PATH".into(), "/usr/bin".into())].into(),
            working_dir: Some("/tmp".into()),
            timeout: Some(Duration::from_secs(30)),
            env_clean: false,
            capture_output: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CommandConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.command, deserialized.command);
        assert_eq!(config.args, deserialized.args);
        assert_eq!(config.env, deserialized.env);
        assert_eq!(config.working_dir, deserialized.working_dir);
        assert_eq!(config.timeout, deserialized.timeout);
        assert_eq!(config.env_clean, deserialized.env_clean);
        assert_eq!(config.capture_output, deserialized.capture_output);
    }

    #[test]
    fn command_output_serde_roundtrip() {
        let output = CommandOutput {
            exit_status: 0,
            stdout: b"hello".to_vec(),
            stderr: Vec::new(),
            duration: Duration::from_millis(42),
        };
        let json = serde_json::to_string(&output).unwrap();
        let deserialized: CommandOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output.exit_status, deserialized.exit_status);
        assert_eq!(output.stdout, deserialized.stdout);
        assert_eq!(output.stderr, deserialized.stderr);
        assert_eq!(output.duration, deserialized.duration);
    }

    #[test]
    fn pty_config_serde_roundtrip() {
        let config = PtyConfig {
            command: "bash".into(),
            args: vec![],
            env: [("TERM".into(), "xterm-256color".into())].into(),
            working_dir: Some("/home".into()),
            cols: 120,
            rows: 40,
            term_env: "xterm-256color".into(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: PtyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.command, deserialized.command);
        assert_eq!(config.cols, deserialized.cols);
        assert_eq!(config.rows, deserialized.rows);
        assert_eq!(config.term_env, deserialized.term_env);
    }

    #[test]
    fn terminal_output_serde_roundtrip() {
        use chrono::Utc;
        use osal_core::SessionId;

        let out = TerminalOutput {
            session_id: SessionId("sess-1".into()),
            data: b"hello".to_vec(),
            timestamp: Utc::now(),
            event_type: TerminalOutputEventType::Stdout,
        };
        let json = serde_json::to_string(&out).unwrap();
        let deserialized: TerminalOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(out.session_id, deserialized.session_id);
        assert_eq!(out.data, deserialized.data);
        assert!(matches!(
            deserialized.event_type,
            TerminalOutputEventType::Stdout
        ));
    }

    #[test]
    fn pty_config_default_values() {
        let config = PtyConfig::default();
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
        assert_eq!(config.term_env, "xterm-256color");
        assert!(config.command.is_empty());
        assert!(config.args.is_empty());
        assert!(config.env.is_empty());
        assert!(config.working_dir.is_none());
    }
}
