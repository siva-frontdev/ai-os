//! Localhost OAuth plumbing shared by every provider setup.
//!
//! This module owns the parts that make a real OAuth flow possible from a
//! CLI without the OAuth Playground:
//!
//! 1. [`CallbackServer`] — binds a `127.0.0.1:<port>` listener and captures
//!    the authorization code (or denial) the provider redirects back to.
//! 2. [`open_browser`] — opens the consent URL in the user's default browser.
//! 3. [`exchange_code`] — POSTs the code to the provider's token endpoint
//!    and returns the refresh token.
//!
//! All failures are mapped to [`SetupError`] codes, never swallowed.

use std::time::Duration;

use reqwest::StatusCode;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::error::SetupError;

/// The default path the callback server serves.
pub const DEFAULT_CALLBACK_PATH: &str = "/oauth/google/callback";

/// A successfully exchanged token pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenPair {
    /// Long-lived refresh token (never persisted to the env file).
    pub access_token: String,
    /// Long-lived refresh token (persisted).
    pub refresh_token: String,
    /// Access token lifetime in seconds, when the provider reports it.
    pub expires_in: Option<u64>,
}

/// A short-lived listener that captures the provider's redirect callback.
#[derive(Debug)]
pub struct CallbackServer {
    listener: TcpListener,
    path: String,
}

impl CallbackServer {
    /// Bind to `host:port` and serve the given callback `path`.
    ///
    /// Port `0` asks the OS to pick a free port; the caller reads the real
    /// port via [`CallbackServer::port`] to build the redirect URI.
    pub async fn bind(host: &str, port: u16, path: &str) -> Result<Self, SetupError> {
        let addr = format!("{host}:{port}");
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| SetupError::callback_server(format!("bind {addr}: {e}")))?;
        Ok(Self {
            listener,
            path: path.trim_start_matches('/').to_string(),
        })
    }

    /// The actual port the listener bound to.
    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .expect("listener has local addr")
            .port()
    }

    /// The full redirect URI the provider must send the user to.
    pub fn redirect_uri(&self, host: &str, port: u16) -> String {
        format!("http://{host}:{port}/{}", self.path)
    }

    /// Wait for one callback and extract the authorization code.
    ///
    /// Validates the `state` parameter against `expected_state` to prevent
    /// CSRF, and returns the `code` on success. A `?error=` callback is
    /// mapped to [`SetupError::access_denied`]; no callback before the
    /// timeout maps to [`SetupError::timeout`]. Incidental requests (e.g.
    /// `/favicon.ico`) are answered and ignored.
    pub async fn wait_for_code(
        &mut self,
        timeout_secs: u64,
        expected_state: &str,
    ) -> Result<String, SetupError> {
        let deadline = Duration::from_secs(timeout_secs);
        loop {
            let (mut socket, peer) = tokio::time::timeout(deadline, self.listener.accept())
                .await
                .map_err(|_| {
                    SetupError::timeout(format!(
                        "no authorization callback received within {timeout_secs}s"
                    ))
                })?
                .map_err(|e| {
                    SetupError::callback_server(format!("accept callback connection: {e}"))
                })?;
            let _ = peer;

            let target = match read_request_target(&mut socket).await {
                Ok(target) => target,
                Err(_) => continue,
            };
            if target == "/favicon.ico" {
                let _ = send_no_content(&mut socket).await;
                continue;
            }
            let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
            let params = parse_query(query);

            if let Some(error) = params.get("error") {
                let description = params
                    .get("error_description")
                    .map(|d| format!(": {d}"))
                    .unwrap_or_default();
                let message = format!("authorization failed ({error}){description}");
                let _ = send_page(&mut socket, "Authorization failed", &message).await;
                return Err(SetupError::access_denied(message));
            }

            let state = params.get("state").map(String::as_str).unwrap_or_default();
            if state != expected_state {
                let message = "callback state mismatch (possible CSRF); re-run the setup";
                let _ = send_page(&mut socket, "Invalid request", message).await;
                return Err(SetupError::internal(message.to_string()));
            }

            let code = params.get("code").cloned().ok_or_else(|| {
                let message = "callback carried neither code nor error";
                SetupError::internal(message.to_string())
            })?;

            send_ok_page(&mut socket).await?;
            return Ok(code);
        }
    }
}

/// Open `url` in the user's default browser.
pub fn open_browser(url: &str) -> Result<(), SetupError> {
    let (program, args) = match std::env::consts::OS {
        "macos" => ("open", vec![url.to_string()]),
        "windows" => (
            "cmd",
            vec!["/C".into(), "start".into(), "".into(), url.to_string()],
        ),
        _ => ("xdg-open", vec![url.to_string()]),
    };
    let status = std::process::Command::new(program)
        .args(&args)
        .spawn()
        .and_then(|mut child| child.wait())
        .map_err(|e| SetupError::browser(format!("failed to launch {program}: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(SetupError::browser(format!(
            "{program} exited with {status}; open this URL manually: {url}"
        )))
    }
}

/// Build a Google OAuth2 authorization URL.
///
/// Uses `access_type=offline&prompt=consent` so Google returns a refresh
/// token on the first exchange, and includes the `state` parameter for CSRF
/// validation by the callback server.
pub fn build_google_auth_url(
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
) -> Result<String, SetupError> {
    let mut url = reqwest::Url::parse("https://accounts.google.com/o/oauth2/auth")
        .map_err(|e| SetupError::internal(format!("cannot parse auth endpoint: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", scope)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", state);
    Ok(url.to_string())
}

/// The response shape of the OAuth2 token endpoint.
#[derive(Debug, serde::Deserialize)]
struct TokenEndpointResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// Exchange an authorization `code` for a refresh token at `token_uri`.
///
/// The `redirect_uri` must exactly match the one used to build the auth URL.
/// A `redirect_uri_mismatch` or invalid-code rejection surfaces as
/// [`SetupError::token_exchange`]; a network failure as
/// [`SetupError::token_network`].
pub async fn exchange_code(
    http: &reqwest::Client,
    token_uri: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<TokenPair, SetupError> {
    let response = http
        .post(token_uri)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .map_err(|e| SetupError::token_network(format!("token request failed: {e}")))?;

    let status = response.status();
    let body: TokenEndpointResponse = response
        .json()
        .await
        .map_err(|e| SetupError::token_exchange(format!("token response parse: {e}")))?;

    if status == StatusCode::UNAUTHORIZED
        || status == StatusCode::BAD_REQUEST
        || body.error.is_some()
    {
        let detail = body
            .error_description
            .or(body.error)
            .unwrap_or_else(|| status.to_string());
        return Err(SetupError::token_exchange(format!(
            "token exchange rejected: {detail}"
        )));
    }
    if !status.is_success() {
        return Err(SetupError::token_network(format!(
            "token endpoint returned HTTP {status}"
        )));
    }

    let access_token = body
        .access_token
        .ok_or_else(|| SetupError::token_exchange("token endpoint returned no access token"))?;
    let refresh_token = body.refresh_token.ok_or_else(|| {
        SetupError::token_exchange(
            "token endpoint returned no refresh token; the client may be missing \
             the 'access_type=offline' consent (re-authorize)",
        )
    })?;

    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: body.expires_in,
    })
}

/// Read a single HTTP request and return its request target (path + query).
async fn read_request_target(socket: &mut TcpStream) -> Result<String, SetupError> {
    let mut buffer = [0u8; 8192];
    let n = socket
        .read(&mut buffer)
        .await
        .map_err(|e| SetupError::callback_server(format!("read callback request: {e}")))?;
    let head = String::from_utf8_lossy(&buffer[..n]);
    let request_line = head.lines().next().unwrap_or_default();
    let target = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| SetupError::callback_server("malformed callback request line"))?;
    Ok(target.to_string())
}

/// Parse a query string into key/value pairs (last value wins).
fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    let parsed = reqwest::Url::parse(&format!("http://localhost/?{query}"));
    if let Ok(url) = parsed {
        for (key, value) in url.query_pairs() {
            params.insert(key.into_owned(), value.into_owned());
        }
    }
    params
}

/// Send a simple success page back to the browser.
async fn send_ok_page(socket: &mut TcpStream) -> Result<(), SetupError> {
    send_page(
        socket,
        "Authorization complete",
        "You can close this window and return to the terminal.",
    )
    .await
}

/// Send an HTML status page back to the browser.
async fn send_page(socket: &mut TcpStream, title: &str, body: &str) -> Result<(), SetupError> {
    let html = format!(
        "<!doctype html><html><head><title>{title}</title></head><body>\
         <h1>{title}</h1><p>{body}</p></body></html>"
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    socket
        .write_all(response.as_bytes())
        .await
        .map_err(|e| SetupError::callback_server(format!("write callback response: {e}")))?;
    Ok(())
}

/// Answer an incidental request (e.g. favicon) without terminating the flow.
async fn send_no_content(socket: &mut TcpStream) -> Result<(), SetupError> {
    let response = "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    socket
        .write_all(response.as_bytes())
        .await
        .map_err(|e| SetupError::callback_server(format!("write 204 response: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_query_extracts_pairs() {
        let params = parse_query("code=abc&state=s1&scope=a%20b");
        assert_eq!(params.get("code").map(String::as_str), Some("abc"));
        assert_eq!(params.get("state").map(String::as_str), Some("s1"));
        assert_eq!(params.get("scope").map(String::as_str), Some("a b"));
    }

    #[test]
    fn parse_query_handles_empty() {
        let params = parse_query("");
        assert!(params.is_empty());
    }

    #[test]
    fn google_auth_url_has_offline_consent() {
        let url = build_google_auth_url("cid", "http://127.0.0.1:8765/cb", "scope", "st").unwrap();
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/auth?"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("client_id=cid"));
        assert!(url.contains("state=st"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8765%2Fcb"));
    }

    #[tokio::test]
    async fn callback_server_returns_code_and_validates_state() {
        let mut server = CallbackServer::bind("127.0.0.1", 0, DEFAULT_CALLBACK_PATH)
            .await
            .unwrap();
        let port = server.port();
        let redirect = server.redirect_uri("127.0.0.1", port);

        let client = reqwest::Client::new();
        let t = tokio::spawn(async move { server.wait_for_code(10, "expected-state").await });

        let url = format!("{redirect}?code=good-code&state=expected-state");
        let _ = client.get(&url).send().await.unwrap();
        let code = t.await.unwrap().unwrap();
        assert_eq!(code, "good-code");
    }

    #[tokio::test]
    async fn callback_server_rejects_wrong_state() {
        let mut server = CallbackServer::bind("127.0.0.1", 0, DEFAULT_CALLBACK_PATH)
            .await
            .unwrap();
        let port = server.port();
        let redirect = server.redirect_uri("127.0.0.1", port);

        let client = reqwest::Client::new();
        let t = tokio::spawn(async move { server.wait_for_code(10, "expected-state").await });

        let url = format!("{redirect}?code=good-code&state=evil");
        let _ = client.get(&url).send().await.unwrap();
        let err = t.await.unwrap().unwrap_err();
        assert_eq!(err.code, "ProviderError");
        assert!(err.message.contains("state mismatch"));
    }

    #[tokio::test]
    async fn callback_server_maps_denial_to_access_denied() {
        let mut server = CallbackServer::bind("127.0.0.1", 0, DEFAULT_CALLBACK_PATH)
            .await
            .unwrap();
        let port = server.port();
        let redirect = server.redirect_uri("127.0.0.1", port);

        let client = reqwest::Client::new();
        let t = tokio::spawn(async move { server.wait_for_code(10, "expected-state").await });

        let url = format!("{redirect}?error=access_denied&error_description=user%20said%20no");
        let _ = client.get(&url).send().await.unwrap();
        let err = t.await.unwrap().unwrap_err();
        assert_eq!(err.code, "AuthenticationRequired");
        assert!(err.message.contains("user said no"));
    }
}
