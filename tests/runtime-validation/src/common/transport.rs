//! A stdio transport over a child process whose handle the test can reach,
//! so failure-recovery scenarios can kill the MCP server mid-session.

use std::path::Path;
use std::sync::Arc;

use super::binary::mcp_server_bin;
use ai_os_openclaw_runtime::mcp::transport::McpTransport;
use ai_os_openclaw_runtime::mcp::McpError;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

/// A child-process stdio transport identical in behavior to
/// `ChildTransport`, but exposing the [`Child`] so a test can kill it.
#[derive(Debug)]
pub struct TestChildTransport {
    child: Mutex<Child>,
    read: Mutex<BufReader<ChildStdout>>,
    write: Mutex<BufWriter<ChildStdin>>,
}

impl TestChildTransport {
    /// Spawn `command` with `args` over piped stdio.
    pub fn spawn(command: &str, args: &[String]) -> Result<Self, McpError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| McpError::Transport(format!("failed to spawn '{command}': {e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Transport("child stdin not available".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Transport("child stdout not available".into()))?;
        Ok(Self {
            child: Mutex::new(child),
            read: Mutex::new(BufReader::new(stdout)),
            write: Mutex::new(BufWriter::new(stdin)),
        })
    }

    /// Kill the MCP server subprocess (simulating a runtime crash).
    pub async fn kill(&self) -> Result<(), std::io::Error> {
        self.child.lock().await.kill().await
    }

    /// Reap the child process, ensuring its pipes are closed so that
    /// subsequent writes fail immediately rather than buffering.
    pub async fn wait(&self) -> Result<std::process::ExitStatus, std::io::Error> {
        self.child.lock().await.wait().await
    }
}

/// Spawn the real `ai-os-mcp-server` binary hosting `plugins`, rooted at
/// `root`.
pub fn spawn_mcp_server(
    plugins: &[&str],
    root: &Path,
) -> Result<Arc<TestChildTransport>, McpError> {
    let bin = mcp_server_bin();
    let bin = bin.display().to_string();
    TestChildTransport::spawn(
        &bin,
        &[
            "--plugins".into(),
            plugins.join(","),
            "--root".into(),
            root.display().to_string(),
        ],
    )
    .map(Arc::new)
}

#[async_trait]
impl McpTransport for TestChildTransport {
    async fn send_line(&self, line: &str) -> Result<(), McpError> {
        let mut write = self.write.lock().await;
        write
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;
        write
            .write_all(b"\n")
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;
        write
            .flush()
            .await
            .map_err(|e| McpError::Transport(e.to_string()))
    }

    async fn recv_line(&self) -> Result<Option<String>, McpError> {
        let mut read = self.read.lock().await;
        let mut line = String::new();
        let n = read
            .read_line(&mut line)
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end().to_string();
        if trimmed.is_empty() {
            return self.recv_line().await;
        }
        Ok(Some(trimmed))
    }

    async fn shutdown(&self) -> Result<(), McpError> {
        let _ = self.kill().await;
        Ok(())
    }
}
