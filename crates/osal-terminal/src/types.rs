//! Data types for the terminal subsystem — command execution and PTY sessions.
//!
//! These types are unique to the terminal subsystem and are not part of
//! [`osal_core`]. They include batch command configuration and output,
//! interactive PTY session configuration, and PTY output event types.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use osal_core::{Pid, SessionId};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};

/// Configuration for a batch command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    /// Path or name of the command to execute.
    pub command: String,
    /// Arguments passed to the command.
    pub args: Vec<String>,
    /// Environment variables for the command.
    pub env: HashMap<String, String>,
    /// Working directory for the command.
    pub working_dir: Option<PathBuf>,
    /// Maximum execution time.
    pub timeout: Option<Duration>,
    /// Whether to start with a clean environment (no inherited vars).
    pub env_clean: bool,
    /// Whether to capture stdout/stderr.
    pub capture_output: bool,
}

/// Result of a batch command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    /// Process exit status code.
    pub exit_status: i32,
    /// Captured stdout bytes.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes.
    pub stderr: Vec<u8>,
    /// Wall-clock duration of execution.
    pub duration: Duration,
}

/// Configuration for opening an interactive PTY session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtyConfig {
    /// Path or name of the command to run inside the PTY.
    pub command: String,
    /// Arguments passed to the command.
    pub args: Vec<String>,
    /// Environment variables for the PTY session.
    pub env: HashMap<String, String>,
    /// Working directory for the PTY session.
    pub working_dir: Option<PathBuf>,
    /// Initial terminal width in columns.
    pub cols: u16,
    /// Initial terminal height in rows.
    pub rows: u16,
    /// Value of the TERM environment variable.
    pub term_env: String,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            cols: 80,
            rows: 24,
            term_env: "xterm-256color".into(),
        }
    }
}

/// An active PTY session with channels for I/O.
#[derive(Debug)]
pub struct PtySession {
    /// Process ID of the PTY child process.
    pub pid: Pid,
    /// Unique session identifier.
    pub session_id: SessionId,
    /// Sender for writing data to PTY stdin.
    pub stdin_tx: Sender<Vec<u8>>,
    /// Receiver for reading PTY output events.
    pub output_rx: Receiver<TerminalOutput>,
}

/// Type of data in a [`TerminalOutput`] event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminalOutputEventType {
    /// Standard output data.
    Stdout,
    /// Standard error data.
    Stderr,
    /// Process exit notification.
    Exit,
}

/// A single chunk of output from a PTY session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalOutput {
    /// Session that produced the output.
    pub session_id: SessionId,
    /// Raw byte data.
    pub data: Vec<u8>,
    /// When the output was produced.
    pub timestamp: DateTime<Utc>,
    /// Type of output event.
    pub event_type: TerminalOutputEventType,
}
