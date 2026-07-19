//! Base OSAL value types — lightweight newtype wrappers.
use serde::{Deserialize, Serialize};

/// Process ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pid(pub u64);

impl std::fmt::Display for Pid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// User ID (numeric).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Uid(pub u32);

/// Group ID (numeric).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Gid(pub u32);

/// File descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fd(pub u64);

/// Signal number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Signal(pub i32);

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signal({})", self.0)
    }
}

/// Process exit status — either an exit code or the signal that terminated it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitStatus(pub Option<i32>, pub Option<Signal>);

/// Unix permission bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permissions(pub u32);

/// String-based user identifier (e.g. "alice" or "uuid:...").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub String);

/// String-based session identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl ExitStatus {
    /// Returns the exit code if the process exited normally.
    pub fn exit_code(&self) -> Option<i32> {
        self.0
    }

    /// Returns the terminating signal if the process was killed.
    pub fn terminating_signal(&self) -> Option<Signal> {
        self.1
    }

    /// Returns `true` if the process exited normally (not killed by a signal).
    pub fn success(&self) -> bool {
        self.0 == Some(0)
    }
}
