use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};
use osal_core::{Pid, Uid, Gid, ExitStatus};
use osal_capabilities::CapabilitySet;

/// Full configuration for spawning a child process.
///
/// Provides fine-grained control over the process environment,
/// I/O stream routing, security context, and resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    /// Executable command or path.
    pub command: String,
    /// Command-line arguments.
    pub args: Vec<String>,
    /// Environment variable overrides.
    pub env: HashMap<String, String>,
    /// Working directory for the child process.
    pub working_dir: Option<PathBuf>,
    /// User ID to run as.
    pub uid: Option<Uid>,
    /// Group ID to run as.
    pub gid: Option<Gid>,
    /// Optional timeout after which the process is killed.
    pub timeout: Option<Duration>,
    /// Capabilities granted to the child process.
    pub capabilities: CapabilitySet,
    /// Standard input configuration.
    pub stdin: Option<ProcessStdin>,
    /// Standard output configuration.
    pub stdout: Option<ProcessStdout>,
    /// Standard error configuration.
    pub stderr: Option<ProcessStdout>,
}

/// Standard input routing for a spawned process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessStdin {
    /// Read from `/dev/null`.
    Null,
    /// Create a pipe for the parent to write.
    Pipe,
    /// Inherit the parent's stdin.
    Inherit,
}

/// Standard output/error routing for a spawned process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessStdout {
    /// Discard output.
    Null,
    /// Create a pipe for the parent to read.
    Pipe,
    /// Inherit the parent's stdout/stderr.
    Inherit,
}

/// A handle to a running child process with access to its I/O streams.
///
/// This wraps the raw [`tokio::process::Child`] handles so callers can
/// interact with the process's stdin, stdout, and stderr directly.
#[derive(Debug)]
pub struct ProcessHandle {
    /// Process ID.
    pub pid: Pid,
    /// Optional handle to the child's standard input.
    pub stdin: Option<ChildStdin>,
    /// Optional handle to the child's standard output.
    pub stdout: Option<ChildStdout>,
    /// Optional handle to the child's standard error.
    pub stderr: Option<ChildStderr>,
}

/// Cross-platform process state classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessState {
    /// Process is currently running.
    Running,
    /// Process is sleeping (idle / waiting).
    Sleeping,
    /// Process has terminated but not yet been reaped.
    Zombie,
    /// Process has been stopped (e.g., by SIGSTOP).
    Stopped,
    /// Process has terminated and been reaped.
    Dead,
}

/// Snapshot of a process at a point in time.
///
/// Unlike [`osal_core::ProcessInfo`], this type carries richer state
/// information via [`ProcessState`] and includes the exit status
/// for terminated processes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStatus {
    /// Process ID.
    pub pid: Pid,
    /// Current state of the process.
    pub state: ProcessState,
    /// Exit status, if the process has terminated.
    pub exit_status: Option<ExitStatus>,
    /// CPU usage as a percentage of one core.
    pub cpu_usage: f64,
    /// Memory usage in bytes.
    pub memory_usage: u64,
    /// User ID that owns the process.
    pub user: Uid,
    /// Wall-clock running time.
    pub running_time: Duration,
}
