//! Full `GmailSetup` integration: real wizard, real env file, real callback
//! server, real `GmailProvider` — only the external Google endpoints are
//! replaced with a local fake. This proves the `life setup gmail` flow works
//! end-to-end without the OAuth Playground.

use std::path::PathBuf;

use life::setup::env_file::EnvFile;
use life::setup::gmail::{
    GmailSetup, GMAIL_CLIENT_ID_ENV, GMAIL_CLIENT_SECRET_ENV, GMAIL_REFRESH_TOKEN_ENV,
};
use life::setup::oauth::{build_google_auth_url, CallbackServer, DEFAULT_CALLBACK_PATH};
use life::{ProviderSetup, SetupContext};

/// Serializes tests that mutate the process environment. Held for the whole
/// test body so a parallel test cannot observe another test's variables.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvGuard(Vec<String>);
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for key in &self.0 {
            std::env::remove_var(key);
        }
    }
}

fn set_vars(guard: &mut EnvGuard, vars: &[(&str, &str)]) {
    for (k, v) in vars {
        std::env::set_var(k, v);
        guard.0.push(k.to_string());
    }
}

/// A browser opener that completes the OAuth callback against the localhost
/// server, exactly like a user would after consenting in a real browser.
fn driving_browser() -> life::setup::gmail::BrowserOpener {
    use std::sync::Arc;
    Arc::new(|auth_url: &str| {
        let redirect_uri = extract_param(auth_url, "redirect_uri")
            .ok_or_else(|| life::SetupError::internal("no redirect_uri in auth url"))?;
        let state = extract_param(auth_url, "state")
            .ok_or_else(|| life::SetupError::internal("no state in auth url"))?;
        let callback = format!("{redirect_uri}?code=test-code&state={state}");
        let client = reqwest::Client::new();
        let handle = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            handle.block_on(async move {
                let _ = client.get(&callback).send().await;
            });
        });
        Ok(())
    })
}

fn extract_param(url: &str, key: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    parsed
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

/// A fake Google backend: token exchange plus profile endpoint.
fn spawn_google_fake() -> std::net::SocketAddr {
    let (addr_tx, addr_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        addr_tx.send(listener.local_addr().unwrap()).unwrap();
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            std::thread::spawn(move || handle_google_conn(stream));
        }
    });
    addr_rx.recv().unwrap()
}

fn handle_google_conn(stream: std::net::TcpStream) {
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
        json_response(&serde_json::json!({
            "access_token": "at",
            "refresh_token": "rt-123",
            "expires_in": 3600,
            "token_type": "Bearer"
        }))
    } else if request_line.starts_with("GET /gmail/v1/users/me/profile") {
        json_response(&serde_json::json!({
            "emailAddress": "agent@example.dev",
            "messagesTotal": 7
        }))
    } else {
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
    };
    let mut stream = stream;
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn json_response(value: &serde_json::Value) -> String {
    let body = serde_json::to_string(value).unwrap();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

fn temp_env_file(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("life-it-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(".env")
}

#[tokio::test]
async fn gmail_setup_flow_persists_refresh_token_and_validates() {
    let _env_lock = ENV_LOCK.lock().await;
    let mut guard = EnvGuard(Vec::new());
    let fake = spawn_google_fake();
    let token_uri = format!("http://{fake}/token");
    let api_base = format!("http://{fake}");
    set_vars(
        &mut guard,
        &[
            ("AIOS_GMAIL_TOKEN_URI", &token_uri),
            ("AIOS_GMAIL_API_BASE", &api_base),
        ],
    );

    let env_file = temp_env_file("flow");
    let mut env = EnvFile::default();
    env.set(GMAIL_CLIENT_ID_ENV, "cid");
    env.set(GMAIL_CLIENT_SECRET_ENV, "csecret");
    env.write(&env_file).unwrap();

    let ctx = SetupContext {
        env_file: env_file.clone(),
        callback_host: "127.0.0.1".into(),
        callback_port: 0,
        timeout_secs: 30,
        verify: true,
        test_recipient: None,
        force: false,
    };

    let wizard = GmailSetup::with_browser(driving_browser());
    wizard.setup(&ctx).await.expect("setup should succeed");

    let stored = EnvFile::load(&env_file).unwrap();
    assert_eq!(stored.get(GMAIL_CLIENT_ID_ENV), Some("cid"));
    assert_eq!(stored.get(GMAIL_CLIENT_SECRET_ENV), Some("csecret"));
    assert_eq!(stored.get(GMAIL_REFRESH_TOKEN_ENV), Some("rt-123"));
    // check_credentials uses the token endpoint only (no profile lookup),
    // so the sender is resolved as "me" by default and AIOS_GMAIL_USER is not written.
    assert_eq!(stored.get("AIOS_GMAIL_USER"), None);

    std::fs::remove_dir_all(env_file.parent().unwrap()).ok();
}

#[tokio::test]
async fn gmail_setup_skips_oauth_when_refresh_token_present() {
    let _env_lock = ENV_LOCK.lock().await;
    let mut guard = EnvGuard(Vec::new());
    let fake = spawn_google_fake();
    let token_uri = format!("http://{fake}/token");
    let api_base = format!("http://{fake}");
    set_vars(
        &mut guard,
        &[
            ("AIOS_GMAIL_TOKEN_URI", &token_uri),
            ("AIOS_GMAIL_API_BASE", &api_base),
        ],
    );

    let env_file = temp_env_file("skip");
    let mut env = EnvFile::default();
    env.set(GMAIL_CLIENT_ID_ENV, "cid");
    env.set(GMAIL_CLIENT_SECRET_ENV, "csecret");
    env.set(GMAIL_REFRESH_TOKEN_ENV, "existing-rt");
    env.write(&env_file).unwrap();

    // A browser opener that must NOT be called when OAuth is skipped.
    let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let called_flag = called.clone();
    let browser = std::sync::Arc::new(move |_: &str| {
        called_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    });

    let ctx = SetupContext {
        env_file: env_file.clone(),
        callback_host: "127.0.0.1".into(),
        callback_port: 0,
        timeout_secs: 30,
        verify: true,
        test_recipient: None,
        force: false,
    };

    let wizard = GmailSetup::with_browser(browser);
    wizard
        .setup(&ctx)
        .await
        .expect("setup should validate existing token");

    assert!(
        !called.load(std::sync::atomic::Ordering::SeqCst),
        "browser must not be opened when refresh token is already stored"
    );

    let stored = EnvFile::load(&env_file).unwrap();
    assert_eq!(stored.get(GMAIL_REFRESH_TOKEN_ENV), Some("existing-rt"));

    std::fs::remove_dir_all(env_file.parent().unwrap()).ok();
}

#[tokio::test]
async fn gmail_setup_force_reruns_oauth() {
    let _env_lock = ENV_LOCK.lock().await;
    let mut guard = EnvGuard(Vec::new());
    let fake = spawn_google_fake();
    let token_uri = format!("http://{fake}/token");
    let api_base = format!("http://{fake}");
    set_vars(
        &mut guard,
        &[
            ("AIOS_GMAIL_TOKEN_URI", &token_uri),
            ("AIOS_GMAIL_API_BASE", &api_base),
        ],
    );

    let env_file = temp_env_file("force");
    let mut env = EnvFile::default();
    env.set(GMAIL_CLIENT_ID_ENV, "cid");
    env.set(GMAIL_CLIENT_SECRET_ENV, "csecret");
    env.set(GMAIL_REFRESH_TOKEN_ENV, "old-rt");
    env.write(&env_file).unwrap();

    let ctx = SetupContext {
        env_file: env_file.clone(),
        callback_host: "127.0.0.1".into(),
        callback_port: 0,
        timeout_secs: 30,
        verify: false,
        test_recipient: None,
        force: true,
    };

    let wizard = GmailSetup::with_browser(driving_browser());
    wizard.setup(&ctx).await.expect("setup should succeed");

    let stored = EnvFile::load(&env_file).unwrap();
    assert_eq!(stored.get(GMAIL_REFRESH_TOKEN_ENV), Some("rt-123"));

    std::fs::remove_dir_all(env_file.parent().unwrap()).ok();
}

#[tokio::test]
async fn callback_server_and_exchange_work_together() {
    let fake = spawn_google_fake();
    let token_uri = format!("http://{fake}/token");

    let mut server = CallbackServer::bind("127.0.0.1", 0, DEFAULT_CALLBACK_PATH)
        .await
        .unwrap();
    let port = server.port();
    let redirect_uri = server.redirect_uri("127.0.0.1", port);

    let auth_url = build_google_auth_url("cid", &redirect_uri, "scope", "st").unwrap();
    let client = reqwest::Client::new();
    let t = tokio::spawn(async move { server.wait_for_code(10, "st").await });

    // Simulate the user completing consent in a browser.
    let callback = format!("{redirect_uri}?code=abc&state=st");
    let _ = client.get(&callback).send().await.unwrap();
    let code = t.await.unwrap().unwrap();
    assert_eq!(code, "abc");

    // Exchange the code for a refresh token.
    let http = reqwest::Client::new();
    let tokens = life::setup::oauth::exchange_code(
        &http,
        &token_uri,
        "cid",
        "csecret",
        "abc",
        &redirect_uri,
    )
    .await
    .unwrap();
    assert_eq!(tokens.refresh_token, "rt-123");
    assert_eq!(tokens.access_token, "at");
    let _ = auth_url;
}

#[tokio::test]
async fn validated_provider_sends_with_stored_credentials() {
    let _env_lock = ENV_LOCK.lock().await;
    let mut guard = EnvGuard(Vec::new());
    let fake = spawn_google_fake();
    let token_uri = format!("http://{fake}/token");
    let api_base = format!("http://{fake}");
    set_vars(
        &mut guard,
        &[
            ("AIOS_GMAIL_TOKEN_URI", &token_uri),
            ("AIOS_GMAIL_API_BASE", &api_base),
        ],
    );

    let env_file = temp_env_file("send");
    let mut env = EnvFile::default();
    env.set(GMAIL_CLIENT_ID_ENV, "cid");
    env.set(GMAIL_CLIENT_SECRET_ENV, "csecret");
    env.set(GMAIL_REFRESH_TOKEN_ENV, "rt-123");
    env.write(&env_file).unwrap();

    // The runtime plugin binary will build a provider straight from the env
    // file the wizard wrote. Proving that path works is the point.
    let provider =
        life::setup::gmail::gmail_provider(&env_file).expect("provider from stored credentials");
    let email = provider.validate().await.unwrap();
    assert_eq!(email, "agent@example.dev");

    std::fs::remove_dir_all(env_file.parent().unwrap()).ok();
}
