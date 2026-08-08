//! WhatsApp webhook receiver.
//!
//! Meta posts inbound messages to the configured webhook URL. This module
//! serves that endpoint (a minimal HTTP/1.1 listener over tokio) and hands
//! verified payloads to the [`WhatsAppProvider`] inbox queue, from which the
//! runtime drains them as observations.

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

use super::whatsapp::WhatsAppProvider;
use crate::provider::error::ProviderError;

/// Configuration for the webhook listener.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Host/interface to bind, e.g. `127.0.0.1`.
    pub host: String,
    /// Port to bind.
    pub port: u16,
    /// URL path, e.g. `/webhook/whatsapp`.
    pub path: String,
}

/// An active webhook server handle. Dropping it stops the accept loop.
#[derive(Debug)]
pub struct WebhookServer {
    stop: tokio::sync::watch::Sender<bool>,
}

impl WebhookServer {
    /// Bind the listener (synchronously, via a std listener) and spawn the
    /// accept loop on the current tokio runtime.
    pub fn start(
        config: WebhookConfig,
        provider: Arc<WhatsAppProvider>,
    ) -> Result<Self, ProviderError> {
        let addr = format!("{}:{}", config.host, config.port);
        let std_listener = std::net::TcpListener::bind(&addr).map_err(|e| {
            ProviderError::provider_unavailable(format!("webhook bind {addr}: {e}"))
        })?;
        std_listener.set_nonblocking(true).map_err(|e| {
            ProviderError::provider_unavailable(format!("webhook nonblocking: {e}"))
        })?;
        let listener = tokio::net::TcpListener::from_std(std_listener)
            .map_err(|e| ProviderError::provider_unavailable(format!("webhook from_std: {e}")))?;

        let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(false);
        let _task = tokio::spawn(async move {
            tracing::info!(addr, path = %config.path, "whatsapp webhook listening");
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { continue };
                        let provider = provider.clone();
                        let path = config.path.clone();
                        tokio::spawn(async move {
                            handle_connection(stream, provider, &path).await;
                        });
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            tracing::info!("whatsapp webhook stopping");
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self { stop: stop_tx })
    }

    /// Request the server to shut down.
    pub fn stop(&self) {
        let _ = self.stop.send(true);
    }
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    provider: Arc<WhatsAppProvider>,
    path: &str,
) {
    let (reader, mut writer) = stream.split();
    let mut buf_reader = tokio::io::BufReader::new(reader);

    let mut request_line = String::new();
    if buf_reader.read_line(&mut request_line).await.is_err() {
        return;
    }
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0].to_string();
    let target = parts[1].to_string();

    // Read headers.
    let mut content_length: usize = 0;
    let mut signature: Option<String> = None;
    loop {
        let mut header = String::new();
        if buf_reader.read_line(&mut header).await.is_err() || header.trim().is_empty() {
            break;
        }
        let lower = header.to_lowercase();
        if lower.starts_with("content-length:") {
            if let Some(val) = header.split(':').nth(1) {
                content_length = val.trim().parse().unwrap_or(0);
            }
        }
        if lower.starts_with("x-hub-signature-256:") {
            if let Some(val) = header.split_once(':') {
                signature = Some(val.1.trim().to_string());
            }
        }
    }

    // Read body if present.
    let mut body = Vec::new();
    if content_length > 0 {
        body.resize(content_length, 0);
        if buf_reader.read_exact(&mut body).await.is_err() {
            return;
        }
    }

    let path_only = target.split('?').next().unwrap_or(&target);
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");

    if path_only != path {
        let _ = respond(&mut writer, 404, "not found").await;
        return;
    }

    match method.as_str() {
        "GET" => {
            let params = parse_query(query);
            let mode = params.get("hub.mode").map(String::as_str).unwrap_or("");
            let token = params
                .get("hub.verify_token")
                .map(String::as_str)
                .unwrap_or("");
            let challenge = params
                .get("hub.challenge")
                .map(String::as_str)
                .unwrap_or("");
            if provider.verify_get(mode, token, challenge) {
                let _ = respond(&mut writer, 200, challenge).await;
            } else {
                let _ = respond(&mut writer, 403, "verification failed").await;
            }
        }
        "POST" => {
            if !provider.verify_signature(signature.as_deref(), &body) {
                tracing::warn!("whatsapp webhook signature verification failed");
                let _ = respond(&mut writer, 401, "signature mismatch").await;
                return;
            }
            match provider.ingest_webhook(&body) {
                Ok(queued) => {
                    tracing::debug!(queued, "whatsapp webhook ingested");
                    let _ = respond(&mut writer, 200, "ok").await;
                }
                Err(e) => {
                    tracing::warn!("whatsapp webhook rejected: {e}");
                    let _ = respond(&mut writer, 400, &e.to_string()).await;
                }
            }
        }
        _ => {
            let _ = respond(&mut writer, 405, "method not allowed").await;
        }
    }
}

async fn respond(
    writer: &mut tokio::net::tcp::WriteHalf<'_>,
    code: u16,
    body: &str,
) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    writer.write_all(response.as_bytes()).await?;
    writer.flush().await
}

fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            out.insert(k.to_string(), url_decode(v).unwrap_or_default());
        }
    }
    out
}

fn url_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
                let byte = u8::from_str_radix(hex, 16).ok()?;
                out.push(byte);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_params_are_decoded() {
        let params = parse_query("hub.mode=subscribe&hub.verify_token=abc%20def&hub.challenge=7");
        assert_eq!(
            params.get("hub.mode").map(String::as_str),
            Some("subscribe")
        );
        assert_eq!(
            params.get("hub.verify_token").map(String::as_str),
            Some("abc def")
        );
        assert_eq!(params.get("hub.challenge").map(String::as_str), Some("7"));
    }
}
