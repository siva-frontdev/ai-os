# osal-process — OSAL Process Subsystem

Architecture-first trait definitions for process lifecycle management. Every privileged operation is capability-gated.

---

## Trait: `ProcessManager`

The primary trait for spawning, controlling, and monitoring processes.

```rust
#[async_trait]
pub trait ProcessManager: Debug + Send + Sync {
    /// Spawn a new process with the given configuration.
    /// Requires Capability::ProcessSpawn.
    async fn spawn(&self, config: &ProcessConfig, ctx: &CapabilityContext) -> Result<ProcessHandle, ProcessError>;

    /// Send a signal to a process.
    /// Requires Capability::ProcessKill(pid).
    async fn kill(&self, pid: Pid, signal: Signal, ctx: &CapabilityContext) -> Result<(), ProcessError>;

    /// Wait for a process to exit and return its exit status.
    /// Requires Capability::ProcessEnumerate (or ownership of the process).
    async fn wait(&self, pid: Pid, ctx: &CapabilityContext) -> Result<ExitStatus, ProcessError>;

    /// Suspend a process (SIGSTOP).
    /// Requires Capability::ProcessSuspend(pid).
    async fn suspend(&self, pid: Pid, ctx: &CapabilityContext) -> Result<(), ProcessError>;

    /// Resume a suspended process (SIGCONT).
    /// Requires Capability::ProcessResume(pid).
    async fn resume(&self, pid: Pid, ctx: &CapabilityContext) -> Result<(), ProcessError>;

    /// List all running processes visible to the caller.
    /// Requires Capability::ProcessEnumerate.
    async fn enumerate(&self, ctx: &CapabilityContext) -> Result<Vec<ProcessInfo>, ProcessError>;

    /// Get detailed status of a specific process.
    /// Requires Capability::ProcessEnumerate.
    async fn status(&self, pid: Pid) -> Result<ProcessStatus, ProcessError>;

    /// Subscribe to process lifecycle events.
    fn watch(&self) -> Receiver<ProcessEvent>;
}
```

---

## Data Types

### `ProcessConfig`

Full configuration for spawning a new process.

```rust
pub struct ProcessConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,
    pub uid: Option<Uid>,
    pub gid: Option<Gid>,
    pub timeout: Option<Duration>,
    pub capabilities: CapabilitySet,   // capabilities granted to the child
    pub stdin: Option<ProcessStdin>,
    pub stdout: Option<ProcessStdout>,
    pub stderr: Option<ProcessStdout>,
}

pub enum ProcessStdin { Null, Pipe, Inherit }
pub enum ProcessStdout { Null, Pipe, Inherit }
```

### `ProcessHandle`

Handle to a spawned process with optional I/O streams.

```rust
pub struct ProcessHandle {
    pub pid: Pid,
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
}
```

### `ProcessInfo`

Snapshot of a running process.

```rust
pub struct ProcessInfo {
    pub pid: Pid,
    pub parent_pid: Option<Pid>,
    pub command: String,
    pub state: ProcessState,
    pub cpu_usage: f64,
    pub memory_usage: u64,
    pub user: Uid,
    pub start_time: DateTime<Utc>,
    pub running_time: Duration,
}
```

### `ProcessState`

Possible process states.

```rust
pub enum ProcessState {
    Running,
    Sleeping,
    Zombie,
    Stopped,
    Dead,
}
```

### `ProcessStatus`

Detailed status for a specific process.

```rust
pub struct ProcessStatus {
    pub pid: Pid,
    pub state: ProcessState,
    pub exit_status: Option<ExitStatus>,
    pub cpu_usage: f64,
    pub memory_usage: u64,
    pub user: Uid,
    pub running_time: Duration,
}
```

---

## Events

### `ProcessEvent`

```rust
pub enum ProcessEvent {
    Started { pid: Pid, command: String, timestamp: DateTime<Utc> },
    Exited { pid: Pid, exit_status: ExitStatus, timestamp: DateTime<Utc> },
    Suspended { pid: Pid, timestamp: DateTime<Utc> },
    Resumed { pid: Pid, timestamp: DateTime<Utc> },
}
```

---

## Error Types

### `ProcessError`

```rust
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProcessError {
    SpawnFailed(String),
    NotFound(String),
    AlreadyComplete(String),
    AccessDenied(String),
    Timeout { pid: Pid, duration: Duration },
    SignalFailed { pid: Pid, signal: i32, reason: String },
    Internal(String),
}
```

Converts into `osal_core::OsalError::Process` for uniform error handling.

---

## Capability Mapping

| Method      | Capability Required        |
|-------------|----------------------------|
| `spawn`     | `ProcessSpawn`             |
| `kill`      | `ProcessKill(pid)`         |
| `wait`      | `ProcessEnumerate`         |
| `suspend`   | `ProcessSuspend(pid)`      |
| `resume`    | `ProcessResume(pid)`       |
| `enumerate` | `ProcessEnumerate`         |
| `status`    | `ProcessEnumerate`         |
| `watch`     | (none — open to all)       |

---

## Architecture

```
┌───────────────────────────────────────────┐
│            osal-process crate             │
│                                           │
│  traits.rs  types.rs  error.rs  events.rs │
│      │          │         │          │     │
│      └──────────┴─────────┴──────────┘     │
│                     │                      │
│              depends on:                   │
│         osal-core, osal-capabilities        │
└───────────────────────────────────────────┘
                      │
                      ▼
        ┌─────────────────────────┐
        │     osal-linux crate    │ (implementor)
        │  (libc, procfs, etc.)   │
        └─────────────────────────┘
```

The `ProcessManager` trait defined here is the full production interface. The simplified version in `osal_core::KernelFacade` exists for bootstrapping; implementations should implement both.
