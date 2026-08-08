//! Email plugin: `email.send` (real delivery through the Gmail API) and
//! `email.inject` (inbound simulation, test/dev only).
//!
//! `email.send` reports success **only** after the Gmail provider confirms the
//! message landed in the Sent folder. When the provider is not configured the
//! tool returns a structured `ConfigurationMissing` error instead of claiming
//! success.

use std::sync::Arc;

use ai_os_mcp_server::{McpPlugin, McpServerError, McpTool, ToolOutcome};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::provider::{GmailProvider, ProviderError};
use crate::util::{now_rfc3339, provider_tool_error, str_arg, tool_error};

/// Internal shared state of the email plugin.
#[derive(Debug)]
struct EmailInner {
    /// Receipts of confirmed deliveries.
    sent: Mutex<Vec<Value>>,
    inbox: Mutex<Vec<Value>>,
}

/// A real email plugin hosted by the MCP server.
#[derive(Debug)]
pub struct EmailPlugin {
    inner: Arc<EmailInner>,
    gmail: Option<Arc<GmailProvider>>,
}

impl EmailPlugin {
    /// Create a plugin backed by the given Gmail provider.
    ///
    /// Pass `None` when Gmail credentials are absent; `email.send` then
    /// reports `ConfigurationMissing`.
    pub fn new(gmail: Option<Arc<GmailProvider>>) -> Self {
        Self {
            inner: Arc::new(EmailInner {
                sent: Mutex::new(Vec::new()),
                inbox: Mutex::new(Vec::new()),
            }),
            gmail,
        }
    }

    /// Number of confirmed deliveries.
    pub async fn sent_count(&self) -> usize {
        self.inner.sent.lock().await.len()
    }

    /// Number of emails waiting in the simulated inbox.
    pub async fn inbox_count(&self) -> usize {
        self.inner.inbox.lock().await.len()
    }
}

#[async_trait]
impl McpPlugin for EmailPlugin {
    fn name(&self) -> &'static str {
        "email"
    }

    fn tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "email.send".into(),
                description:
                    "Send an email message through the Gmail API. Confirms delivery (SENT label)."
                        .into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "to": {"type": "string"},
                        "subject": {"type": "string"},
                        "body": {"type": "string"}
                    },
                    "required": ["to"]
                }),
            },
            McpTool {
                name: "email.inject".into(),
                description:
                    "Simulate an inbound email (test/dev). Emits an inbound_message observation"
                        .into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "from": {"type": "string"},
                        "to": {"type": "string"},
                        "subject": {"type": "string"},
                        "body": {"type": "string"}
                    },
                    "required": ["from", "to"]
                }),
            },
        ]
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutcome, McpServerError> {
        match name {
            "email.send" => self.send(arguments).await,
            "email.inject" => self.inject(arguments).await,
            _ => Err(McpServerError::UnknownTool(name.to_string())),
        }
    }
}

impl EmailPlugin {
    async fn send(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(to) = str_arg(&args, "to") else {
            return Ok(tool_error("email.send requires a 'to' recipient"));
        };
        let subject = str_arg(&args, "subject").unwrap_or_default();
        let body = str_arg(&args, "body").unwrap_or_default();

        let Some(gmail) = &self.gmail else {
            return Ok(provider_tool_error(&ProviderError::configuration_missing(
                "Gmail provider is not configured: set AIOS_GMAIL_CLIENT_ID, \
                 AIOS_GMAIL_CLIENT_SECRET and AIOS_GMAIL_REFRESH_TOKEN",
            )));
        };
        let from = gmail.user().to_string();
        match gmail.send(&to, &subject, &body, &from).await {
            Ok(receipt) => {
                let sent_at = now_rfc3339();
                self.inner.sent.lock().await.push(json!({
                    "messageId": receipt.message_id,
                    "to": receipt.to,
                    "subject": receipt.subject,
                    "sentAt": sent_at,
                }));
                Ok(ToolOutcome {
                    is_error: false,
                    result: json!({
                        "messageId": receipt.message_id,
                        "to": receipt.to,
                        "subject": receipt.subject,
                        "status": "sent"
                    }),
                    observations: vec![json!({
                        "source": "email.send",
                        "kind": "status_changed",
                        "channel_id": receipt.to,
                        "text": format!("email sent to {}: {}", receipt.to, receipt.subject),
                    })],
                })
            }
            Err(err) => Ok(provider_tool_error(&err)),
        }
    }

    async fn inject(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(from) = str_arg(&args, "from") else {
            return Ok(tool_error("email.inject requires a 'from' address"));
        };
        let Some(to) = str_arg(&args, "to") else {
            return Ok(tool_error("email.inject requires a 'to' address"));
        };
        let subject = str_arg(&args, "subject").unwrap_or_default();
        let body = str_arg(&args, "body").unwrap_or_default();
        let message_id = uuid::Uuid::new_v4().to_string();

        self.inner.inbox.lock().await.push(json!({
            "messageId": message_id,
            "from": from,
            "to": to,
            "subject": subject,
            "body": body,
            "receivedAt": now_rfc3339(),
        }));

        Ok(ToolOutcome {
            is_error: false,
            result: json!({
                "messageId": message_id,
                "injected": true,
                "subject": subject
            }),
            observations: vec![json!({
                "source": "email.receive",
                "kind": "inbound_message",
                "channel_id": to,
                "sender": from,
                "subject": subject,
                "body": body,
                "text": subject,
            })],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::GmailConfig;

    fn unconfigured() -> EmailPlugin {
        EmailPlugin::new(None)
    }

    #[tokio::test]
    async fn email_send_without_config_is_structured_error() {
        let plugin = unconfigured();
        let outcome = plugin
            .call_tool(
                "email.send",
                json!({"to": "a@b.c", "subject": "hi", "body": "hello"}),
            )
            .await
            .unwrap();
        assert!(outcome.is_error);
        assert_eq!(outcome.result["error"]["code"], "ConfigurationMissing");
        assert_eq!(plugin.sent_count().await, 0);
        assert!(outcome.observations.is_empty());
    }

    #[tokio::test]
    async fn email_send_requires_to() {
        let plugin = unconfigured();
        let outcome = plugin
            .call_tool("email.send", json!({"subject": "no recipient"}))
            .await
            .unwrap();
        assert!(outcome.is_error);
        assert_eq!(
            outcome.result["error"],
            "email.send requires a 'to' recipient"
        );
        assert_eq!(plugin.sent_count().await, 0);
    }

    #[tokio::test]
    async fn email_send_with_config_hits_gmail_api() {
        // Local stand-in for the Google OAuth2 + Gmail API endpoints.
        let addr = spawn_gmail_fake();
        let cfg = GmailConfig {
            client_id: "cid".into(),
            client_secret: "csecret".into(),
            refresh_token: "rtok".into(),
            user: "me".into(),
            token_uri: format!("http://{addr}/token"),
            api_base: format!("http://{addr}"),
        };
        let plugin = EmailPlugin::new(Some(Arc::new(GmailProvider::new(cfg))));

        let outcome = plugin
            .call_tool(
                "email.send",
                json!({"to": "a@b.c", "subject": "hi", "body": "hello"}),
            )
            .await
            .unwrap();
        assert!(!outcome.is_error, "unexpected error: {}", outcome.result);
        assert_eq!(outcome.result["status"], "sent");
        assert_eq!(outcome.result["to"], "a@b.c");
        assert!(outcome.result["messageId"].as_str().is_some());
        assert_eq!(plugin.sent_count().await, 1);
        assert_eq!(outcome.observations[0]["kind"], "status_changed");
    }

    #[tokio::test]
    async fn email_inject_emits_inbound_observation() {
        let plugin = unconfigured();
        let outcome = plugin
            .call_tool(
                "email.inject",
                json!({"from": "friend@x.dev", "to": "aios@x.dev", "subject": "news"}),
            )
            .await
            .unwrap();
        assert!(!outcome.is_error);
        assert_eq!(plugin.inbox_count().await, 1);
        let obs = &outcome.observations[0];
        assert_eq!(obs["source"], "email.receive");
        assert_eq!(obs["kind"], "inbound_message");
        assert_eq!(obs["sender"], "friend@x.dev");
        assert_eq!(obs["channel_id"], "aios@x.dev");
    }

    #[tokio::test]
    async fn unknown_tool_rejected() {
        let plugin = unconfigured();
        assert!(matches!(
            plugin.call_tool("email.fax", json!({})).await,
            Err(McpServerError::UnknownTool(_))
        ));
    }

    /// A minimal fake Gmail backend covering token refresh, send, and verify.
    fn spawn_gmail_fake() -> std::net::SocketAddr {
        let (addr_tx, addr_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            addr_tx.send(listener.local_addr().unwrap()).unwrap();
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                std::thread::spawn(move || handle_fake_conn(stream));
            }
        });
        addr_rx.recv().unwrap()
    }

    fn handle_fake_conn(stream: std::net::TcpStream) {
        use std::io::{BufRead, BufReader, Read, Write};
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            return;
        }
        let mut content_length = 0usize;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                break;
            }
            if header.to_lowercase().starts_with("content-length:") {
                content_length = header
                    .split(':')
                    .nth(1)
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap_or(0);
            }
        }
        let mut body = vec![0u8; content_length];
        let _ = reader.read_exact(&mut body);

        let response = if request_line.starts_with("POST /token ") {
            json_response(
                &json!({"access_token": "at", "expires_in": 3600, "token_type": "Bearer"}),
            )
        } else if request_line.starts_with("POST /gmail/v1/users/me/messages/send") {
            json_response(&json!({"id": "msg-1", "threadId": "t-1", "labelIds": ["SENT"]}))
        } else if request_line.starts_with("GET /gmail/v1/users/me/messages/msg-1") {
            json_response(&json!({"id": "msg-1", "labelIds": ["SENT"], "threadId": "t-1"}))
        } else {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
        };
        let mut stream = stream;
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    }

    fn json_response(value: &Value) -> String {
        let body = serde_json::to_string(value).unwrap();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }
}
