//! Gmail provider: real delivery through the Gmail API.
//!
//! Flow:
//! 1. Exchange the refresh token for an access token (OAuth2).
//! 2. POST the message (RFC 2822, base64url) to `users.messages.send`.
//! 3. Re-fetch the message and verify the `SENT` label is present.
//!
//! Success is reported **only** after step 3 confirms delivery.

use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use reqwest::StatusCode;
use serde_json::{json, Value};

use crate::provider::config::GmailConfig;
use crate::provider::error::ProviderError;

/// A successfully delivered Gmail message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailReceipt {
    /// Gmail message id.
    pub message_id: String,
    /// Recipient address.
    pub to: String,
    /// Subject line.
    pub subject: String,
}

/// The response shape of the OAuth2 token endpoint.
#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// The response shape of `users.messages.send`.
#[derive(Debug, serde::Deserialize)]
struct SendResponse {
    id: Option<String>,
}

/// The response shape of `users.messages.get` (metadata only).
#[derive(Debug, serde::Deserialize)]
struct MessageResponse {
    #[serde(default, rename = "labelIds")]
    label_ids: Vec<String>,
}

/// The response shape of `users.profile`.
#[derive(Debug, serde::Deserialize)]
struct ProfileResponse {
    #[serde(default, rename = "emailAddress")]
    email_address: Option<String>,
}

/// A minimal HTTP client that drives the Gmail API.
#[derive(Debug, Clone)]
pub struct GmailProvider {
    http: reqwest::Client,
    config: Arc<GmailConfig>,
}

impl GmailProvider {
    /// Build a provider for the given config.
    pub fn new(config: GmailConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            http,
            config: Arc::new(config),
        }
    }

    /// The authenticated user (defaults to `me`), used as the sender.
    pub fn user(&self) -> &str {
        &self.config.user
    }

    /// Verify the stored credentials work by exchanging the refresh token for
    /// an access token.
    ///
    /// This is the scope-independent validation: it proves the client id,
    /// client secret and refresh token are valid and authorized without
    /// requiring a read scope. The setup wizard uses this before reporting
    /// success.
    pub async fn check_credentials(&self) -> Result<(), ProviderError> {
        self.access_token().await.map(|_| ())
    }

    /// Verify the stored credentials work by fetching the profile and
    /// returning the authenticated account email address.
    ///
    /// Unlike [`GmailProvider::check_credentials`], this requires a read
    /// scope (e.g. `gmail.readonly` or `gmail.metadata`); the minimal
    /// `gmail.send` scope is **not** enough for `users.profile`.
    pub async fn validate(&self) -> Result<String, ProviderError> {
        let token = self.access_token().await?;
        let endpoint = format!(
            "{}/gmail/v1/users/{}/profile",
            self.config.api_base, self.config.user
        );
        let response = self
            .http
            .get(&endpoint)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| {
                ProviderError::provider_unavailable(format!("profile request failed: {e}"))
            })?;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            let detail = summarize(&response.text().await.unwrap_or_default());
            let detail = if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            };
            return Err(ProviderError::authentication_required(format!(
                "Gmail rejected the access token (HTTP {status}){detail}"
            )));
        }
        if !status.is_success() {
            let detail = summarize(&response.text().await.unwrap_or_default());
            return Err(provider_http_error(status, "Gmail validate", detail));
        }

        let body: ProfileResponse = response.json().await.map_err(|e| {
            ProviderError::unexpected_response(format!("profile response parse: {e}"))
        })?;
        body.email_address
            .filter(|e| !e.trim().is_empty())
            .ok_or_else(|| {
                ProviderError::unexpected_response("Gmail profile returned no email address")
            })
    }

    /// Exchange the refresh token for a fresh access token.
    async fn access_token(&self) -> Result<String, ProviderError> {
        let cfg = &self.config;
        let response = self
            .http
            .post(&cfg.token_uri)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", cfg.client_id.as_str()),
                ("client_secret", cfg.client_secret.as_str()),
                ("refresh_token", cfg.refresh_token.as_str()),
            ])
            .send()
            .await
            .map_err(|e| {
                ProviderError::provider_unavailable(format!("token request failed: {e}"))
            })?;

        let status = response.status();
        let body: TokenResponse = response.json().await.map_err(|e| {
            ProviderError::unexpected_response(format!("token response parse: {e}"))
        })?;

        if status == StatusCode::UNAUTHORIZED
            || status == StatusCode::BAD_REQUEST
            || body.error.is_some()
        {
            let detail = body
                .error_description
                .or(body.error)
                .unwrap_or_else(|| status.to_string());
            return Err(ProviderError::authentication_required(format!(
                "OAuth2 token refresh failed: {detail}"
            )));
        }
        if !status.is_success() {
            return Err(ProviderError::provider_unavailable(format!(
                "token endpoint returned HTTP {status}"
            )));
        }
        body.access_token.ok_or_else(|| {
            ProviderError::unexpected_response("token endpoint returned no access token")
        })
    }

    /// Send a message and confirm it landed in the Sent folder.
    pub async fn send(
        &self,
        to: &str,
        subject: &str,
        body_text: &str,
        from: &str,
    ) -> Result<GmailReceipt, ProviderError> {
        if to.trim().is_empty() {
            return Err(ProviderError::invalid_input(
                "email.send requires a 'to' address",
            ));
        }

        let token = self.access_token().await?;
        let raw = build_rfc2822(from, to, subject, body_text);
        let encoded = URL_SAFE_NO_PAD.encode(raw.as_bytes());

        let endpoint = format!(
            "{}/gmail/v1/users/{}/messages/send",
            self.config.api_base, self.config.user
        );
        let response = self
            .http
            .post(&endpoint)
            .bearer_auth(&token)
            .json(&json!({ "raw": encoded }))
            .send()
            .await
            .map_err(|e| {
                ProviderError::provider_unavailable(format!("send request failed: {e}"))
            })?;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            let detail = summarize(&response.text().await.unwrap_or_default());
            let detail = if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            };
            return Err(ProviderError::authentication_required(format!(
                "Gmail rejected the access token (HTTP {status}){detail}"
            )));
        }
        if !status.is_success() {
            let detail = summarize(&response.text().await.unwrap_or_default());
            return Err(provider_http_error(status, "Gmail send", detail));
        }

        let send_body: SendResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::unexpected_response(format!("send response parse: {e}")))?;
        let message_id = send_body.id.ok_or_else(|| {
            ProviderError::unexpected_response("Gmail send returned no message id")
        })?;

        self.verify_sent(&token, &message_id).await?;

        Ok(GmailReceipt {
            message_id,
            to: to.to_string(),
            subject: subject.to_string(),
        })
    }

    /// Re-fetch the message and require the `SENT` label.
    async fn verify_sent(&self, token: &str, message_id: &str) -> Result<(), ProviderError> {
        let endpoint = format!(
            "{}/gmail/v1/users/{}/messages/{}?format=metadata",
            self.config.api_base, self.config.user, message_id
        );
        let response = self
            .http
            .get(&endpoint)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| {
                ProviderError::provider_unavailable(format!("verify request failed: {e}"))
            })?;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            let detail = summarize(&response.text().await.unwrap_or_default());
            let detail = if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            };
            return Err(ProviderError::authentication_required(format!(
                "Gmail rejected the access token while verifying (HTTP {status}){detail}"
            )));
        }
        if !status.is_success() {
            let detail = summarize(&response.text().await.unwrap_or_default());
            return Err(provider_http_error(status, "Gmail verify", detail));
        }

        let body: MessageResponse = response.json().await.map_err(|e| {
            ProviderError::unexpected_response(format!("verify response parse: {e}"))
        })?;
        if !body.label_ids.iter().any(|l| l == "SENT") {
            return Err(ProviderError::unexpected_response(format!(
                "Gmail did not confirm delivery: message {message_id} missing the SENT label \
                 (labels: {:?})",
                body.label_ids
            )));
        }
        Ok(())
    }
}

/// Build a minimal RFC 2822 message body.
fn build_rfc2822(from: &str, to: &str, subject: &str, body: &str) -> String {
    format!(
        "From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nMIME-Version: 1.0\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\n{body}"
    )
}

fn provider_http_error(status: StatusCode, action: &str, detail: String) -> ProviderError {
    if status.is_server_error() {
        ProviderError::provider_unavailable(format!("{action} failed (HTTP {status}): {detail}"))
    } else {
        ProviderError::unexpected_response(format!("{action} failed (HTTP {status}): {detail}"))
    }
}

fn summarize(body: &str) -> String {
    let trimmed = body.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(msg) = value["error"]["message"].as_str() {
            return msg.to_string();
        }
    }
    if trimmed.len() > 500 {
        trimmed.chars().take(500).collect()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc2822_message_is_well_formed() {
        let raw = build_rfc2822("me@x.dev", "you@y.dev", "Hi", "body text");
        assert!(raw.contains("From: me@x.dev"));
        assert!(raw.contains("To: you@y.dev"));
        assert!(raw.contains("Subject: Hi"));
        assert!(raw.ends_with("body text"));
    }

    #[test]
    fn empty_recipient_is_rejected_as_invalid_input() {
        let provider = GmailProvider::new(GmailConfig {
            client_id: "c".into(),
            client_secret: "s".into(),
            refresh_token: "r".into(),
            user: "me".into(),
            token_uri: "http://localhost:1/token".into(),
            api_base: "http://localhost:1/gmail".into(),
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let err = rt
            .block_on(provider.send("", "subj", "body", "me@x.dev"))
            .unwrap_err();
        assert_eq!(err.code, "InvalidInput");
    }

    #[test]
    fn validate_returns_authenticated_email() {
        let addr = spawn_profile_fake();
        let provider = GmailProvider::new(GmailConfig {
            client_id: "cid".into(),
            client_secret: "csecret".into(),
            refresh_token: "rtok".into(),
            user: "me".into(),
            token_uri: format!("http://{addr}/token"),
            api_base: format!("http://{addr}"),
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let email = rt.block_on(provider.validate()).unwrap();
        assert_eq!(email, "agent@example.dev");
    }

    #[test]
    fn validate_surfaces_bad_credentials_as_auth_error() {
        let addr = spawn_profile_fake();
        let provider = GmailProvider::new(GmailConfig {
            client_id: "cid".into(),
            client_secret: "csecret".into(),
            refresh_token: "bad".into(),
            user: "me".into(),
            token_uri: format!("http://{addr}/token"),
            api_base: format!("http://{addr}"),
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt.block_on(provider.validate()).unwrap_err();
        assert_eq!(err.code, "AuthenticationRequired");
    }

    /// A minimal fake Gmail backend: refresh-token endpoint plus profile.
    fn spawn_profile_fake() -> std::net::SocketAddr {
        let (addr_tx, addr_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            addr_tx.send(listener.local_addr().unwrap()).unwrap();
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                std::thread::spawn(move || handle_profile_conn(stream));
            }
        });
        addr_rx.recv().unwrap()
    }

    fn handle_profile_conn(stream: std::net::TcpStream) {
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
            let rejected = !body.is_empty()
                && body
                    .split(|&b| b == b'&')
                    .any(|kv| kv == b"refresh_token=bad");
            if rejected {
                json_response(&json!({
                    "error": "invalid_grant",
                    "error_description": "Refresh token is invalid",
                }))
            } else {
                json_response(&json!({"access_token": "at", "expires_in": 3600}))
            }
        } else if request_line.starts_with("GET /gmail/v1/users/me/profile") {
            json_response(&json!({
                "emailAddress": "agent@example.dev",
                "messagesTotal": 42,
            }))
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
