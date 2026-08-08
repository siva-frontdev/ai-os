//! WhatsApp provider: real messaging through the Meta WhatsApp Cloud API.
//!
//! Outbound (`send`) posts to `/{version}/{phone_number_id}/messages` and
//! reports success only when Meta returns a message id. Inbound messages
//! arrive on a webhook HTTP endpoint (long-lived token + signature
//! verified); the plugin queues them and `receive` drains the queue into
//! `inbound_message` observations for the runtime.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{json, Value};

use crate::provider::config::WhatsAppConfig;
use crate::provider::error::ProviderError;

/// A delivered WhatsApp message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatsAppReceipt {
    /// Meta message id.
    pub message_id: String,
    /// Recipient phone number in E.164 format.
    pub to: String,
}

/// One inbound WhatsApp message as delivered by the webhook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundWhatsAppMessage {
    /// Sender phone number.
    pub from: String,
    /// Message text.
    pub text: String,
    /// Message id.
    pub message_id: String,
    /// Unix timestamp of the message.
    pub timestamp: String,
}

/// The response shape of `POST .../messages`.
#[derive(Debug, serde::Deserialize)]
struct SendResponse {
    #[serde(default)]
    messages: Vec<SendMessageEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct SendMessageEntry {
    id: Option<String>,
}

/// The parsed webhook payload.
#[derive(Debug, serde::Deserialize)]
struct WebhookPayload {
    #[serde(default)]
    entry: Vec<WebhookEntry>,
}

/// The response shape of `GET /{version}/{phone_number_id}`.
#[derive(Debug, serde::Deserialize)]
struct PhoneInfoResponse {
    #[serde(default, rename = "display_phone_number")]
    display_phone_number: Option<String>,
    #[serde(default)]
    error: Option<PhoneError>,
}

#[derive(Debug, serde::Deserialize)]
struct PhoneError {
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct WebhookEntry {
    #[serde(default)]
    changes: Vec<WebhookChange>,
}

#[derive(Debug, serde::Deserialize)]
struct WebhookChange {
    value: WebhookValue,
}

#[derive(Debug, serde::Deserialize)]
struct WebhookValue {
    #[serde(default)]
    messages: Vec<WebhookMessage>,
}

#[derive(Debug, serde::Deserialize)]
struct WebhookMessage {
    id: Option<String>,
    from: Option<String>,
    #[serde(default)]
    text: Option<WebhookText>,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct WebhookText {
    body: Option<String>,
}

/// A real WhatsApp Cloud API client plus an inbound queue.
#[derive(Debug, Clone)]
pub struct WhatsAppProvider {
    http: reqwest::Client,
    config: Arc<WhatsAppConfig>,
    inbox: Arc<Mutex<Vec<InboundWhatsAppMessage>>>,
}

impl WhatsAppProvider {
    /// Build a provider for the given config.
    pub fn new(config: WhatsAppConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            http,
            config: Arc::new(config),
            inbox: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Send a text message and require a Meta-confirmed message id.
    pub async fn send(&self, to: &str, text: &str) -> Result<WhatsAppReceipt, ProviderError> {
        if to.trim().is_empty() {
            return Err(ProviderError::invalid_input(
                "whatsapp.send requires a 'to' recipient",
            ));
        }
        if text.trim().is_empty() {
            return Err(ProviderError::invalid_input(
                "whatsapp.send requires a 'text' body",
            ));
        }

        let cfg = &self.config;
        let endpoint = format!(
            "{}/{}/{}/messages",
            cfg.graph_base, cfg.api_version, cfg.phone_number_id
        );
        let response = self
            .http
            .post(&endpoint)
            .bearer_auth(&cfg.access_token)
            .json(&json!({
                "messaging_product": "whatsapp",
                "to": to,
                "type": "text",
                "text": { "body": text },
            }))
            .send()
            .await
            .map_err(|e| {
                ProviderError::provider_unavailable(format!("send request failed: {e}"))
            })?;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(ProviderError::authentication_required(format!(
                "WhatsApp rejected the access token (HTTP {status})"
            )));
        }
        if !status.is_success() {
            let detail = summarize(&response.text().await.unwrap_or_default());
            return Err(provider_http_error(status, "WhatsApp send", detail));
        }

        let body: SendResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::unexpected_response(format!("send response parse: {e}")))?;
        let message_id = body
            .messages
            .into_iter()
            .next()
            .and_then(|m| m.id)
            .ok_or_else(|| {
                ProviderError::unexpected_response("WhatsApp send returned no message id")
            })?;

        Ok(WhatsAppReceipt {
            message_id,
            to: to.to_string(),
        })
    }

    /// Drain the inbound queue (one call per webhook delivery window).
    pub fn drain(&self) -> Vec<InboundWhatsAppMessage> {
        let mut inbox = self.inbox.lock().expect("whatsapp inbox lock");
        std::mem::take(&mut *inbox)
    }

    /// Verify the stored credentials work by fetching the phone number
    /// profile from the WhatsApp Cloud API and returning its display phone
    /// number.
    ///
    /// This performs a real authenticated Graph API call, so it can be used
    /// by the setup wizard to confirm a token + phone number id before
    /// reporting success.
    pub async fn validate(&self) -> Result<String, ProviderError> {
        let cfg = &self.config;
        let endpoint = format!(
            "{}/{}/{}",
            cfg.graph_base, cfg.api_version, cfg.phone_number_id
        );
        let response = self
            .http
            .get(&endpoint)
            .bearer_auth(&cfg.access_token)
            .send()
            .await
            .map_err(|e| {
                ProviderError::provider_unavailable(format!("profile request failed: {e}"))
            })?;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(ProviderError::authentication_required(format!(
                "WhatsApp rejected the access token (HTTP {status})"
            )));
        }
        if !status.is_success() {
            let detail = summarize(&response.text().await.unwrap_or_default());
            return Err(provider_http_error(status, "WhatsApp validate", detail));
        }

        let body: PhoneInfoResponse = response.json().await.map_err(|e| {
            ProviderError::unexpected_response(format!("profile response parse: {e}"))
        })?;
        if let Some(err) = body.error {
            return Err(ProviderError::authentication_required(format!(
                "WhatsApp profile returned an error: {}",
                err.message.unwrap_or_default()
            )));
        }
        body.display_phone_number
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ProviderError::unexpected_response("WhatsApp profile returned no phone number")
            })
    }

    /// Number of queued inbound messages (for tests/diagnostics).
    pub fn pending(&self) -> usize {
        self.inbox.lock().expect("whatsapp inbox lock").len()
    }

    /// Queue inbound messages parsed from a webhook body.
    ///
    /// Returns the number of messages queued. Webhooks that carry no
    /// messages (e.g. status updates) queue nothing but are not errors.
    pub fn ingest_webhook(&self, body: &[u8]) -> Result<usize, ProviderError> {
        let payload: WebhookPayload = serde_json::from_slice(body).map_err(|e| {
            ProviderError::unexpected_response(format!("webhook body is not valid JSON: {e}"))
        })?;

        let mut queued = 0usize;
        let mut inbox = self.inbox.lock().expect("whatsapp inbox lock");
        for entry in payload.entry {
            for change in entry.changes {
                for msg in change.value.messages {
                    let Some(from) = msg.from.clone() else {
                        continue;
                    };
                    let text = msg.text.and_then(|t| t.body).unwrap_or_default();
                    if text.is_empty() {
                        continue;
                    }
                    inbox.push(InboundWhatsAppMessage {
                        from,
                        text,
                        message_id: msg.id.unwrap_or_default(),
                        timestamp: msg.timestamp.unwrap_or_default(),
                    });
                    queued += 1;
                }
            }
        }
        Ok(queued)
    }

    /// Verify a webhook request.
    ///
    /// For GET (subscription verification): returns the challenge when the
    /// `hub.verify_token` matches. For POST: verifies the
    /// `X-Hub-Signature-256` HMAC when `app_secret` is configured.
    pub fn verify_get(&self, mode: &str, token: &str, challenge: &str) -> bool {
        mode == "subscribe"
            && !token.is_empty()
            && !challenge.is_empty()
            && token == self.config.webhook_verify_token
    }

    /// Verify the HMAC signature of a webhook body.
    ///
    /// When `app_secret` is empty (no signature configured) verification is
    /// skipped; callers should then enforce other access controls. When set,
    /// a missing or mismatched signature fails verification.
    pub fn verify_signature(&self, signature: Option<&str>, body: &[u8]) -> bool {
        if self.config.app_secret.is_empty() {
            return true;
        }
        let Some(signature) = signature else {
            return false;
        };
        let expected = hmac_sha256_hex(self.config.app_secret.as_bytes(), body);
        let supplied = signature
            .strip_prefix("sha256=")
            .unwrap_or(signature)
            .to_lowercase();
        constant_time_eq(&supplied, &expected)
    }
}

/// Build an observation payload from an inbound message.
pub fn inbound_observation(msg: &InboundWhatsAppMessage) -> Value {
    json!({
        "source": "whatsapp.receive",
        "kind": "inbound_message",
        "channel_id": msg.from,
        "sender": msg.from,
        "text": msg.text,
        "message_id": msg.message_id,
        "timestamp": msg.timestamp,
    })
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

/// HMAC-SHA256 implemented over `sha2` (already in the dependency tree).
fn hmac_sha256_hex(key: &[u8], data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    const BLOCK: usize = 64;
    let mut key_pad = [0u8; BLOCK];
    if key.len() > BLOCK {
        let mut hasher = Sha256::new();
        hasher.update(key);
        let digest = hasher.finalize();
        key_pad[..32].copy_from_slice(&digest);
    } else {
        key_pad[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= key_pad[i];
        opad[i] ^= key_pad[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(data);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_digest);
    hex(&outer.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> WhatsAppConfig {
        WhatsAppConfig {
            access_token: "token".into(),
            phone_number_id: "123456".into(),
            api_version: "v21.0".into(),
            graph_base: "http://localhost:1".into(),
            webhook_verify_token: "verify-token".into(),
            app_secret: "app-secret".into(),
            webhook_host: "127.0.0.1".into(),
            webhook_port: 0,
            webhook_path: "/webhook/whatsapp".into(),
        }
    }

    #[test]
    fn empty_recipient_rejected() {
        let provider = WhatsAppProvider::new(config());
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let err = rt.block_on(provider.send("", "hi")).unwrap_err();
        assert_eq!(err.code, "InvalidInput");
    }

    #[test]
    fn validate_returns_display_phone_number() {
        let addr = spawn_whatsapp_fake();
        let cfg = WhatsAppConfig {
            access_token: "token".into(),
            phone_number_id: "123456".into(),
            api_version: "v21.0".into(),
            graph_base: format!("http://{addr}"),
            ..config()
        };
        let provider = WhatsAppProvider::new(cfg);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let phone = rt.block_on(provider.validate()).unwrap();
        assert_eq!(phone, "+15556649010");
    }

    #[test]
    fn validate_surfaces_bad_token_as_auth_error() {
        let addr = spawn_whatsapp_fake();
        let cfg = WhatsAppConfig {
            access_token: "bad".into(),
            phone_number_id: "123456".into(),
            api_version: "v21.0".into(),
            graph_base: format!("http://{addr}"),
            ..config()
        };
        let provider = WhatsAppProvider::new(cfg);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let err = rt.block_on(provider.validate()).unwrap_err();
        assert_eq!(err.code, "AuthenticationRequired");
    }

    /// A minimal fake WhatsApp Graph API backend.
    fn spawn_whatsapp_fake() -> std::net::SocketAddr {
        let (addr_tx, addr_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            addr_tx.send(listener.local_addr().unwrap()).unwrap();
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                std::thread::spawn(move || handle_whatsapp_conn(stream));
            }
        });
        addr_rx.recv().unwrap()
    }

    fn handle_whatsapp_conn(stream: std::net::TcpStream) {
        use std::io::{BufRead, BufReader, Write};
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            return;
        }
        let mut authorization = String::new();
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                break;
            }
            if header.to_lowercase().starts_with("authorization:") {
                authorization = header;
            }
        }
        let response = if request_line.starts_with("GET /v21.0/123456") {
            let bad_token = authorization.contains("Bearer bad");
            if bad_token {
                json_response(&json!({
                    "error": {
                        "message": "Invalid OAuth access token",
                        "type": "OAuthException",
                        "code": 190
                    }
                }))
            } else {
                json_response(&json!({
                    "verified_name": "AI-OS Test",
                    "display_phone_number": "+15556649010",
                    "quality_rating": "Green",
                }))
            }
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

    #[test]
    fn webhook_parses_and_queues_messages() {
        let provider = WhatsAppProvider::new(config());
        let body = br#"{
            "object": "whatsapp_business_account",
            "entry": [{
                "id": "wh",
                "changes": [{
                    "value": {
                        "messages": [{
                            "from": "15551234567",
                            "id": "wamid.1",
                            "timestamp": "1720000000",
                            "type": "text",
                            "text": { "body": "hello from test" }
                        }]
                    }
                }]
            }]
        }"#;
        let queued = provider.ingest_webhook(body).unwrap();
        assert_eq!(queued, 1);
        let drained = provider.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].from, "15551234567");
        assert_eq!(drained[0].text, "hello from test");
    }

    #[test]
    fn status_only_webhook_queues_nothing() {
        let provider = WhatsAppProvider::new(config());
        let body = br#"{"entry":[{"changes":[{"value":{"statuses":[{"id":"s1"}]}}]}]}"#;
        assert_eq!(provider.ingest_webhook(body).unwrap(), 0);
    }

    #[test]
    fn malformed_webhook_is_an_error() {
        let provider = WhatsAppProvider::new(config());
        assert!(provider.ingest_webhook(b"not json").is_err());
    }

    #[test]
    fn get_verification_matches_verify_token() {
        let provider = WhatsAppProvider::new(config());
        assert!(provider.verify_get("subscribe", "verify-token", "challenge"));
        assert!(!provider.verify_get("subscribe", "wrong", "challenge"));
        assert!(!provider.verify_get("unsubscribe", "verify-token", "challenge"));
    }

    #[test]
    fn signature_verification_accepts_valid_hmac() {
        let provider = WhatsAppProvider::new(config());
        let body = b"{\"entry\":[]}";
        let expected = hmac_sha256_hex(b"app-secret", body);
        let signature = format!("sha256={expected}");
        assert!(provider.verify_signature(Some(&signature), body));
        assert!(!provider.verify_signature(Some("sha256=deadbeef"), body));
        assert!(!provider.verify_signature(None, body));
    }

    #[test]
    fn hmac_matches_reference_vector() {
        // RFC 4231 test case 1: key = 0x0b x 20, data = "Hi There".
        let key = [0x0bu8; 20];
        let digest = hmac_sha256_hex(&key, b"Hi There");
        assert_eq!(
            digest,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
}
