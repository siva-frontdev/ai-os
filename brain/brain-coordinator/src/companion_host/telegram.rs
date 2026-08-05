use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;

use brain_core::types::Decision;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::{Duration, interval};

use crate::cognitive_loop::CognitiveLoopService;
use crate::errors::CoordinatorError;

/// Persistent identity mapping for Telegram users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUserIdentity {
    pub telegram_user_id: u64,
    pub username: String,
    pub display_name: String,
    pub first_seen: String,
    pub last_active: String,
}

/// Runtime statistics exposed in developer diagnostics.
#[derive(Debug, Clone)]
pub struct TelegramStats {
    pub connected: bool,
    pub polls: u64,
    pub updates_received: u64,
    pub messages_processed: u64,
    pub errors: u64,
    pub last_received_at: Option<String>,
    pub last_sent_at: Option<String>,
    pub current_chat_id: Option<i64>,
    pub avg_latency_ms: u64,
}

#[derive(Debug, Deserialize)]
struct TelegramUpdate {
    #[serde(default)]
    update_id: u64,
    #[serde(default)]
    message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
struct TelegramMessage {
    message_id: u64,
    date: u64,
    #[serde(default)]
    text: Option<String>,
    chat: TelegramChat,
    #[serde(default)]
    from: Option<TelegramUser>,
}

#[derive(Debug, Deserialize)]
struct TelegramChat {
    id: i64,
    #[serde(rename = "type")]
    chat_type: String,
}

#[derive(Debug, Deserialize)]
struct TelegramUser {
    id: u64,
    #[serde(default)]
    is_bot: bool,
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetUpdatesResponse {
    ok: bool,
    #[serde(default)]
    result: Vec<TelegramUpdate>,
}

#[derive(Debug, Deserialize)]
struct SendMessageResponse {
    ok: bool,
}

pub struct TelegramAdapter {
    loop_svc: Arc<Mutex<CognitiveLoopService>>,
    http: reqwest::Client,
    bot_token: String,
    api_base: String,
    poll_interval: Duration,
    running: Arc<AtomicBool>,
    offset: Arc<AtomicU64>,
    identity_path: PathBuf,
    identities: Arc<StdMutex<Vec<TelegramUserIdentity>>>,
    stats: Arc<StdMutex<TelegramStats>>,
    latency_accum: Arc<StdMutex<(u64, u64)>>,
}

impl TelegramAdapter {
    pub fn new(
        loop_svc: Arc<Mutex<CognitiveLoopService>>,
        bot_token: String,
        poll_interval_secs: u64,
        identity_path: impl AsRef<Path>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let api_base = format!("https://api.telegram.org/bot{}", bot_token);

        let identity_path = identity_path.as_ref().to_path_buf();
        let identities = Self::load_identities(&identity_path);

        Self {
            loop_svc,
            http,
            bot_token,
            api_base,
            poll_interval: Duration::from_secs(poll_interval_secs.max(1)),
            running: Arc::new(AtomicBool::new(false)),
            offset: Arc::new(AtomicU64::new(0)),
            identity_path,
            identities: Arc::new(StdMutex::new(identities)),
            stats: Arc::new(StdMutex::new(TelegramStats {
                connected: false,
                polls: 0,
                updates_received: 0,
                messages_processed: 0,
                errors: 0,
                last_received_at: None,
                last_sent_at: None,
                current_chat_id: None,
                avg_latency_ms: 0,
            })),
            latency_accum: Arc::new(StdMutex::new((0, 0))),
        }
    }

    pub async fn start(&self, mut stop_rx: tokio::sync::watch::Receiver<bool>) {
        self.running.store(true, Ordering::Release);
        let offset = self.offset.clone();
        let http = self.http.clone();
        let api_base = self.api_base.clone();
        let poll_interval = self.poll_interval;
        let running = self.running.clone();
        let identities = self.identities.clone();
        let identity_path = self.identity_path.clone();
        let loop_svc = self.loop_svc.clone();
        let stats = self.stats.clone();
        let latency_accum = self.latency_accum.clone();

        match Self::verify_token(&http, &api_base).await {
            Ok(bot_name) => {
                tracing::info!("Telegram bot @{} connected", bot_name);
            }
            Err(e) => {
                tracing::error!("Telegram bot token invalid: {e}");
                running.store(false, Ordering::Release);
                return;
            }
        }

        tracing::info!(
            "Telegram polling started (interval: {}s)",
            poll_interval.as_secs()
        );
        tokio::spawn(async move {
            let mut timer = interval(poll_interval);
            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        if !running.load(Ordering::Acquire) {
                            break;
                        }
                        Self::poll_once(
                            &http, &api_base, &offset, &loop_svc,
                            &identities, &identity_path, &stats, &latency_accum,
                        ).await;
                    }
                    _ = stop_rx.changed() => {
                        if !*stop_rx.borrow() {
                            tracing::info!("Telegram polling stopping");
                            break;
                        }
                    }
                }
            }
            running.store(false, Ordering::Release);
            if let Ok(mut s) = stats.lock() {
                s.connected = false;
            }
            tracing::info!("Telegram polling stopped");
        });
    }

    async fn poll_once(
        http: &reqwest::Client,
        api_base: &str,
        offset: &AtomicU64,
        loop_svc: &Arc<Mutex<CognitiveLoopService>>,
        identities: &Arc<StdMutex<Vec<TelegramUserIdentity>>>,
        identity_path: &Path,
        stats: &Arc<StdMutex<TelegramStats>>,
        latency_accum: &Arc<StdMutex<(u64, u64)>>,
    ) {
        let poll_start = Instant::now();
        let current_offset = offset.load(Ordering::Acquire);

        let resp: GetUpdatesResponse = match http
            .get(&format!("{}/getUpdates", api_base))
            .query(&[
                ("offset", &current_offset.to_string()),
                ("timeout", &"10".to_string()),
                ("allowed_updates", &"[\"message\"]".to_string()),
            ])
            .send()
            .await
        {
            Ok(r) => r.json().await.unwrap_or(GetUpdatesResponse {
                ok: false,
                result: vec![],
            }),
            Err(e) => {
                tracing::warn!("Telegram poll request failed: {e}");
                if let Ok(mut s) = stats.lock() {
                    s.errors += 1;
                }
                return;
            }
        };

        if !resp.ok {
            tracing::warn!("Telegram getUpdates returned ok=false");
            if let Ok(mut s) = stats.lock() {
                s.errors += 1;
            }
            return;
        }

        let count = resp.result.len() as u64;
        if let Ok(mut s) = stats.lock() {
            s.polls += 1;
            s.updates_received += count;
            s.connected = true;
        }

        for update in &resp.result {
            if let Some(ref msg) = update.message {
                if let Some(ref from) = msg.from {
                    if from.is_bot {
                        continue;
                    }
                }
                let text = match msg.text.as_ref() {
                    Some(t) if !t.is_empty() => t.clone(),
                    _ => continue,
                };

                tracing::info!(text = %text, chat_id = %msg.chat.id, "Telegram received message");

                offset.store(update.update_id + 1, Ordering::Release);

                if let Some(ref from) = msg.from {
                    Self::record_identity(identities, identity_path, from, &msg.chat);
                }

                let now = chrono::Utc::now().to_rfc3339();
                if let Ok(mut s) = stats.lock() {
                    s.messages_processed += 1;
                    s.last_received_at = Some(now.clone());
                    s.current_chat_id = Some(msg.chat.id);
                }

                let _ = http
                    .post(&format!("{}/sendChatAction", api_base))
                    .json(&serde_json::json!({
                        "chat_id": msg.chat.id,
                        "action": "typing",
                    }))
                    .send()
                    .await;

                let process_start = Instant::now();
                let decision = loop_svc.lock().await.cycle(&text).await;
                let latency = process_start.elapsed().as_millis() as u64;

                let decision_type = match &decision {
                    Decision::Wait => "Wait",
                    Decision::Communicate { .. } => "Communicate",
                    Decision::UpdateMemory { .. } => "UpdateMemory",
                    Decision::Execute { .. } => "Execute",
                };
                tracing::info!(
                    decision_type,
                    latency_ms = latency,
                    "Telegram cognitive result"
                );

                if let Ok(mut acc) = latency_accum.lock() {
                    acc.0 += latency;
                    acc.1 += 1;
                    if let Ok(mut s) = stats.lock() {
                        s.avg_latency_ms = if acc.1 > 0 { acc.0 / acc.1 } else { 0 };
                    }
                }

                if let Decision::Communicate { message, .. } = &decision {
                    Self::send_reply(http, api_base, msg.chat.id, message, stats).await;
                }
            }
        }

        let elapsed = poll_start.elapsed();
        if elapsed > Duration::from_secs(15) {
            tracing::warn!(
                "Telegram poll took {:.1}s ({} updates)",
                elapsed.as_secs_f64(),
                count
            );
        }
    }

    async fn send_reply(
        http: &reqwest::Client,
        api_base: &str,
        chat_id: i64,
        text: &str,
        stats: &Arc<StdMutex<TelegramStats>>,
    ) {
        let safe_text = Self::escape_markdown(text);
        let resp: SendMessageResponse = match http
            .post(&format!("{}/sendMessage", api_base))
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": safe_text,
                "parse_mode": "MarkdownV2",
                "disable_web_page_preview": true,
            }))
            .send()
            .await
        {
            Ok(r) => r.json().await.unwrap_or(SendMessageResponse { ok: false }),
            Err(e) => {
                tracing::warn!("Telegram sendMessage failed: {e}");
                let _ = http
                    .post(&format!("{}/sendMessage", api_base))
                    .json(&serde_json::json!({
                        "chat_id": chat_id,
                        "text": text,
                    }))
                    .send()
                    .await;
                if let Ok(mut s) = stats.lock() {
                    s.errors += 1;
                }
                return;
            }
        };

        if resp.ok {
            let now = chrono::Utc::now().to_rfc3339();
            if let Ok(mut s) = stats.lock() {
                s.last_sent_at = Some(now.clone());
            }
            tracing::info!(chat_id, text = %text, "Telegram reply sent");
        } else {
            tracing::warn!("Telegram sendMessage returned ok=false");
            if let Ok(mut s) = stats.lock() {
                s.errors += 1;
            }
        }
    }

    async fn verify_token(
        http: &reqwest::Client,
        api_base: &str,
    ) -> Result<String, CoordinatorError> {
        #[derive(Deserialize)]
        struct User {
            username: Option<String>,
        }
        #[derive(Deserialize)]
        struct MeResponse {
            ok: bool,
            result: Option<User>,
        }
        let resp = http
            .get(&format!("{}/getMe", api_base))
            .send()
            .await
            .map_err(|e| CoordinatorError::Internal(format!("Telegram getMe failed: {e}")))?;
        let body: MeResponse = resp
            .json()
            .await
            .map_err(|e| CoordinatorError::Internal(format!("Telegram getMe parse: {e}")))?;
        if !body.ok {
            return Err(CoordinatorError::Internal(
                "Telegram getMe returned ok=false".into(),
            ));
        }
        Ok(body.result.and_then(|u| u.username).unwrap_or_default())
    }

    fn load_identities(path: &Path) -> Vec<TelegramUserIdentity> {
        match std::fs::read_to_string(path) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    fn save_identities(path: &Path, identities: &[TelegramUserIdentity]) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(identities) {
            let _ = std::fs::write(path, &json);
        }
    }

    fn record_identity(
        identities: &Arc<StdMutex<Vec<TelegramUserIdentity>>>,
        path: &Path,
        from: &TelegramUser,
        _chat: &TelegramChat,
    ) {
        let now = chrono::Utc::now().to_rfc3339();
        if let Ok(mut guard) = identities.lock() {
            if let Some(existing) = guard.iter_mut().find(|i| i.telegram_user_id == from.id) {
                existing.last_active = now.clone();
                existing.display_name = from.first_name.clone();
                if let Some(ref u) = from.username {
                    existing.username = u.clone();
                }
            } else {
                guard.push(TelegramUserIdentity {
                    telegram_user_id: from.id,
                    username: from.username.clone().unwrap_or_default(),
                    display_name: from.first_name.clone(),
                    first_seen: now.clone(),
                    last_active: now,
                });
            }
            Self::save_identities(path, &guard);
        }
    }

    fn escape_markdown(text: &str) -> String {
        let special = [
            '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.',
            '!',
        ];
        let mut out = String::with_capacity(text.len() + 16);
        for ch in text.chars() {
            if special.contains(&ch) {
                out.push('\\');
            }
            out.push(ch);
        }
        out
    }

    pub fn stats(&self) -> TelegramStats {
        self.stats
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|e| e.into_inner().clone())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub fn signal_stop(&self) {
        self.running.store(false, Ordering::Release);
    }
}
