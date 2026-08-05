//! MCP transports: how newline-delimited JSON-RPC messages are carried.

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use crate::mcp::protocol::McpError;

/// A byte-level carrier for newline-delimited JSON-RPC messages.
///
/// Implemented for in-memory duplex pairs (tests) and child-process stdio
/// (production OpenClaw sidecar).
#[async_trait]
pub trait McpTransport: std::fmt::Debug + Send + Sync {
    /// Send one complete message plus a trailing newline.
    async fn send_line(&self, line: &str) -> Result<(), McpError>;
    /// Receive one complete message, or `None` on EOF.
    async fn recv_line(&self) -> Result<Option<String>, McpError>;
    /// Tear down the transport (close pipes / kill child).
    async fn shutdown(&self) -> Result<(), McpError>;
}

/// In-memory transport over a `tokio::io::DuplexStream`.
///
/// Used by the mock MCP server in tests. Deterministic and fast.
#[derive(Debug)]
pub struct DuplexTransport {
    read: Mutex<BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>>,
    write: Mutex<BufWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>>,
}

impl DuplexTransport {
    /// Wrap one end of a duplex pair.
    pub fn new(stream: tokio::io::DuplexStream) -> Self {
        let (read, write) = tokio::io::split(stream);
        Self {
            read: Mutex::new(BufReader::new(read)),
            write: Mutex::new(BufWriter::new(write)),
        }
    }
}

#[async_trait]
impl McpTransport for DuplexTransport {
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
        Ok(())
    }
}

/// Transport over a child process's stdio (MCP stdio transport).
///
/// This is how the adapter reaches a real OpenClaw Gateway MCP server in
/// production.
#[derive(Debug)]
pub struct ChildTransport {
    child: Mutex<Child>,
    read: Mutex<BufReader<ChildStdout>>,
    write: Mutex<BufWriter<ChildStdin>>,
}

impl ChildTransport {
    /// Spawn the MCP server subprocess.
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
}

#[async_trait]
impl McpTransport for ChildTransport {
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
        let mut child = self.child.lock().await;
        let _ = child.kill().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn duplex_transport_round_trips() {
        let (a, b) = tokio::io::duplex(65536);
        let left = DuplexTransport::new(a);
        let right = DuplexTransport::new(b);

        left.send_line(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
            .await
            .unwrap();
        let line = right.recv_line().await.unwrap().unwrap();
        assert_eq!(line, r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#);

        right
            .send_line(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#)
            .await
            .unwrap();
        let reply = left.recv_line().await.unwrap().unwrap();
        assert_eq!(reply, r#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
    }

    #[tokio::test]
    async fn duplex_transport_returns_none_on_eof() {
        let (a, b) = tokio::io::duplex(65536);
        let left = DuplexTransport::new(a);
        let right = DuplexTransport::new(b);
        right.shutdown().await.unwrap();
        drop(right);
        assert_eq!(left.recv_line().await.unwrap(), None);
    }
}
