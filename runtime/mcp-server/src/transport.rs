//! Server-side transports: how newline-delimited JSON-RPC is carried.
//!
//! In production the server reads stdin and writes stdout (MCP stdio
//! transport). In tests it runs over an in-memory duplex pair.

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex;

use crate::error::McpServerError;

/// A byte-level carrier for newline-delimited JSON-RPC messages, server
/// side.
#[async_trait]
pub trait McpServerTransport: std::fmt::Debug + Send + Sync {
    /// Send one complete message plus a trailing newline.
    async fn send_line(&self, line: &str) -> Result<(), McpServerError>;
    /// Receive one complete message, or `None` on EOF.
    async fn recv_line(&self) -> Result<Option<String>, McpServerError>;
    /// Tear down the transport.
    async fn shutdown(&self) -> Result<(), McpServerError>;
}

/// The production transport: reads newline-delimited JSON from stdin and
/// writes responses to stdout. Any logging must go to **stderr** — anything
/// written to stdout corrupts the protocol.
#[derive(Debug)]
pub struct StdioServerTransport {
    read: Mutex<BufReader<tokio::io::Stdin>>,
    write: Mutex<BufWriter<tokio::io::Stdout>>,
}

impl StdioServerTransport {
    /// Wrap the process's stdin and stdout.
    pub fn new() -> Self {
        Self {
            read: Mutex::new(BufReader::new(tokio::io::stdin())),
            write: Mutex::new(BufWriter::new(tokio::io::stdout())),
        }
    }
}

impl Default for StdioServerTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpServerTransport for StdioServerTransport {
    async fn send_line(&self, line: &str) -> Result<(), McpServerError> {
        let mut write = self.write.lock().await;
        write
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpServerError::Transport(e.to_string()))?;
        write
            .write_all(b"\n")
            .await
            .map_err(|e| McpServerError::Transport(e.to_string()))?;
        write
            .flush()
            .await
            .map_err(|e| McpServerError::Transport(e.to_string()))
    }

    async fn recv_line(&self) -> Result<Option<String>, McpServerError> {
        let mut read = self.read.lock().await;
        let mut line = String::new();
        let n = read
            .read_line(&mut line)
            .await
            .map_err(|e| McpServerError::Transport(e.to_string()))?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end().to_string();
        if trimmed.is_empty() {
            return self.recv_line().await;
        }
        Ok(Some(trimmed))
    }

    async fn shutdown(&self) -> Result<(), McpServerError> {
        Ok(())
    }
}

/// In-memory transport over a `tokio::io::DuplexStream`, used by tests.
#[derive(Debug)]
pub struct DuplexServerTransport {
    read: Mutex<BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>>,
    write: Mutex<BufWriter<tokio::io::WriteHalf<tokio::io::DuplexStream>>>,
}

impl DuplexServerTransport {
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
impl McpServerTransport for DuplexServerTransport {
    async fn send_line(&self, line: &str) -> Result<(), McpServerError> {
        let mut write = self.write.lock().await;
        write
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpServerError::Transport(e.to_string()))?;
        write
            .write_all(b"\n")
            .await
            .map_err(|e| McpServerError::Transport(e.to_string()))?;
        write
            .flush()
            .await
            .map_err(|e| McpServerError::Transport(e.to_string()))
    }

    async fn recv_line(&self) -> Result<Option<String>, McpServerError> {
        let mut read = self.read.lock().await;
        let mut line = String::new();
        let n = read
            .read_line(&mut line)
            .await
            .map_err(|e| McpServerError::Transport(e.to_string()))?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end().to_string();
        if trimmed.is_empty() {
            return self.recv_line().await;
        }
        Ok(Some(trimmed))
    }

    async fn shutdown(&self) -> Result<(), McpServerError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn duplex_transport_round_trips() {
        let (a, b) = tokio::io::duplex(65536);
        let left = DuplexServerTransport::new(a);
        let right = DuplexServerTransport::new(b);

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
        let left = DuplexServerTransport::new(a);
        let right = DuplexServerTransport::new(b);
        right.shutdown().await.unwrap();
        drop(right);
        assert_eq!(left.recv_line().await.unwrap(), None);
    }
}
