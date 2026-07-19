# osal-terminal — OSAL Terminal Subsystem

Architecture-first trait definitions for command execution, pseudo-terminal (PTY) management, and terminal I/O streaming.

---

## Trait: `Terminal`

The primary trait for batch command execution and interactive PTY sessions.

```rust
#[async_trait]
pub trait Terminal: Debug + Send + Sync {
    /// Execute a command and capture output (batch mode).
    /// Requires Capability::TerminalExecute.
    async fn execute(&self, config: &CommandConfig, ctx: &CapabilityContext) -> Result<CommandOutput, TerminalError>;

    /// Open an interactive PTY session.
    /// Requires Capability::TerminalPTY.
    async fn open_pty(&self, config: &PtyConfig, ctx: &CapabilityContext) -> Result<PtySession, TerminalError>;

    /// Resize an active PTY.
    /// No additional capability required (session ownership established at open_pty).
    async fn resize_pty(&self, session: &SessionId, cols: u16, rows: u16) -> Result<(), TerminalError>;

    /// Close a PTY session.
    async fn close_pty(&self, session: &SessionId) -> Result<(), TerminalError>;

    /// Write data to PTY stdin.
    async fn write_pty(&self, session: &SessionId, data: &[u8]) -> Result<(), TerminalError>;

    /// Read from PTY output stream.
    async fn read_pty(&self, session: &SessionId) -> Result<Vec<u8>, TerminalError>;

    /// Get terminal size.
    async fn size(&self, session: &SessionId) -> Result<(u16, u16), TerminalError>;

    /// Stream PTY output via a Receiver channel.
    async fn stream_output(&self, session: &SessionId) -> Result<Receiver<TerminalOutput>, TerminalError>;
}
```

---

## Data Types

### `CommandConfig`

Configuration for batch command execution.

```rust
pub struct CommandConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,
    pub timeout: Option<Duration>,
    pub env_clean: bool,        // if true, start with empty env
    pub capture_output: bool,   // if true, capture stdout/stderr
}
```

### `CommandOutput`

Result of a batch command execution.

```rust
pub struct CommandOutput {
    pub exit_status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub duration: Duration,
}
```

### `PtyConfig`

Configuration for opening a PTY session.

```rust
pub struct PtyConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,
    pub cols: u16,           // default 80
    pub rows: u16,           // default 24
    pub term_env: String,    // default "xterm-256color"
}
```

### `PtySession`

Handle for an interactive PTY session.

```rust
pub struct PtySession {
    pub pid: Pid,
    pub session_id: SessionId,
    pub stdin_tx: Sender<Vec<u8>>,       // write to PTY stdin
    pub output_rx: Receiver<TerminalOutput>,  // read from PTY output
}
```

---

## Events

### `TerminalOutput`

A single unit of output from a PTY session.

```rust
pub struct TerminalOutput {
    pub session_id: SessionId,
    pub data: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub event_type: TerminalOutputEventType,
}

pub enum TerminalOutputEventType {
    Stdout,
    Stderr,
    Exit,
}
```

---

## Error Types

### `TerminalError`

```rust
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TerminalError {
    SpawnFailed(String),
    SessionNotFound(String),
    SessionClosed(String),
    PtyAllocationFailed(String),
    ResizeFailed(String),
    Timeout(String),
    IoError(String),
}
```

Converts into `osal_core::OsalError::Terminal` for uniform error handling.

---

## Capability Mapping

| Method          | Capability Required      |
|-----------------|--------------------------|
| `execute`       | `TerminalExecute`        |
| `open_pty`      | `TerminalPTY`            |
| `resize_pty`    | (inherited from session) |
| `close_pty`     | (inherited from session) |
| `write_pty`     | (inherited from session) |
| `read_pty`      | (inherited from session) |
| `size`          | (inherited from session) |
| `stream_output` | (inherited from session) |

Session-level methods (`write_pty`, `read_pty`, `resize_pty`, `close_pty`, `size`, `stream_output`)
do not re-check capabilities — the capability check is performed once at `open_pty` time and
the session token (`SessionId`) serves as proof of authorization.

---

## Architecture

```
┌───────────────────────────────────────────┐
│           osal-terminal crate             │
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
        │  (fork/exec, posix_openpt,  │
        │   termios, poll, etc.)   │
        └─────────────────────────┘
```

The `Terminal` trait defined here is the full production interface. The simplified version in `osal_core::KernelFacade` exists for bootstrapping; implementations should implement both.

### Execution Flow: `execute()`

```
Caller                    Terminal trait              OS (Linux)
  │                            │                          │
  ├─ execute(config, ctx) ───► │                          │
  │                            ├─ check Capability ─────► │
  │                            │     (TerminalExecute)     │
  │                            ├─ fork/exec ────────────► │
  │                            │     (with PTY or pipe)    │
  │                            ├─ wait for exit ────────► │
  │                            │◄─── exit status ──────── │
  │◄──── CommandOutput ────────┤                          │
```

### Execution Flow: `open_pty()`

```
Caller                    Terminal trait              OS (Linux)
  │                            │                          │
  ├─ open_pty(config, ctx) ──►│                          │
  │                            ├─ check Capability ─────► │
  │                            │     (TerminalPTY)          │
  │                            ├─ posix_openpt ─────────► │
  │                            ├─ fork/exec ────────────► │
  │                            │     (in PTY)              │
  │◄──── PtySession ──────────┤                          │
  │     (pid, session_id,      │                          │
  │      stdin_tx, output_rx)  │                          │
```
