use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use brain_core::types::Decision;
use memory_storage::wm_store::WorldModelStore;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::sync::watch;

use crate::attention::AttentionDecision;
use crate::cognitive_loop::CognitiveLoopService;
use crate::cognitive_loop::ReflectionLogEntry;
use crate::companion_host::observation_loop::ObservationDebugInfo;
use crate::companion_host::settings::DynSettingsManager;
use crate::errors::CoordinatorError;

// ── Public types ──────────────────────────────────────────

/// Configuration for the companion web UI.
#[derive(Debug, Clone)]
pub struct UiConfig {
    pub port: u16,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { port: 9876 }
    }
}

/// A snapshot of companion state for the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UiSnapshot {
    pub version: u64,
    pub running: bool,
    pub cycle_count: u64,
    pub attention: Option<AttentionSnapshot>,
    pub last_decision: Option<DecisionSnapshot>,
    pub entity_count: usize,
    pub relationship_count: usize,
    pub recent_reflections: Vec<ReflectionLogEntry>,
    pub observation_debug: Option<ObservationDebugInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AttentionSnapshot {
    pub outcome: String,
    pub reason: String,
    pub confidence: f64,
    pub signals: Vec<SignalSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SignalSnapshot {
    pub name: String,
    pub strength: f64,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DecisionSnapshot {
    pub decision_type: String,
    pub message: Option<String>,
    pub reason: Option<String>,
}

// ── Companion UI Server ──────────────────────────────────

/// Minimal embedded web UI for the companion.
///
/// Serves a single-page application on `http://localhost:<port>`.
/// Uses SSE for live updates and REST API for state queries.
pub struct CompanionUi {
    config: UiConfig,
    store: Arc<dyn WorldModelStore>,
    loop_svc: Arc<Mutex<CognitiveLoopService>>,
    running: Arc<AtomicBool>,
    version: Arc<AtomicU64>,
    last_decision: StdMutex<Option<Decision>>,
    last_attention: StdMutex<Option<AttentionDecision>>,
    stop_tx: Option<watch::Sender<bool>>,
    settings_mgr: Option<DynSettingsManager>,
    audit_log: Option<Arc<StdMutex<super::AuditLog>>>,
    encryption_enabled: bool,
    privacy_mgr: Option<super::PrivacyManager>,
}

impl std::fmt::Debug for CompanionUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompanionUi")
            .field("port", &self.config.port)
            .field("version", &self.version)
            .finish()
    }
}

impl CompanionUi {
    pub fn new(
        config: UiConfig,
        store: Arc<dyn WorldModelStore>,
        loop_svc: Arc<Mutex<CognitiveLoopService>>,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            config,
            store,
            loop_svc,
            running,
            version: Arc::new(AtomicU64::new(0)),
            last_decision: StdMutex::new(None),
            last_attention: StdMutex::new(None),
            stop_tx: None,
            settings_mgr: None,
            audit_log: None,
            encryption_enabled: false,
            privacy_mgr: None,
        }
    }

    /// Attach the settings manager for settings API endpoints.
    pub fn set_settings_manager(&mut self, mgr: DynSettingsManager) {
        self.settings_mgr = Some(mgr);
    }

    /// Attach the audit log for the activity API.
    pub fn set_audit_log(&mut self, log: Arc<StdMutex<super::AuditLog>>) {
        self.audit_log = Some(log);
    }

    /// Attach the privacy manager and check encryption status.
    pub fn set_privacy(&mut self, mgr: &super::PrivacyManager) {
        self.encryption_enabled = mgr.is_encryption_enabled();
        self.privacy_mgr = Some(mgr.clone());
    }

    /// Get the current port.
    pub fn port(&self) -> u16 {
        self.config.port
    }

    /// Notify the UI that state changed (increments version counter for SSE).
    pub fn notify_change(&self) {
        self.version.fetch_add(1, Ordering::Release);
    }

    /// Record the last decision for the UI.
    pub fn record_decision(&self, decision: &Decision) {
        if let Ok(mut d) = self.last_decision.lock() {
            *d = Some(decision.clone());
        }
        self.notify_change();
    }

    /// Record attention state for the UI.
    pub fn record_attention(&self, attention: &AttentionDecision) {
        if let Ok(mut a) = self.last_attention.lock() {
            *a = Some(attention.clone());
        }
        self.notify_change();
    }

    /// Start the HTTP server in a background task.
    pub async fn start(&mut self) -> Result<(), CoordinatorError> {
        let addr = format!("127.0.0.1:{}", self.config.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| CoordinatorError::Internal(format!("Failed to bind UI server: {e}")))?;
        tracing::info!("Companion UI available at http://{addr}");

        let (stop_tx, mut stop_rx) = watch::channel(false);

        let store = self.store.clone();
        let loop_svc = self.loop_svc.clone();
        let running = self.running.clone();
        let version = self.version.clone();

        let last_decision = Arc::new(StdMutex::new(
            self.last_decision.lock().ok().and_then(|mut d| d.take()),
        ));
        let last_attention = Arc::new(StdMutex::new(
            self.last_attention.lock().ok().and_then(|mut a| a.take()),
        ));
        let ui_version = self.version.clone();
        let settings_mgr = self.settings_mgr.clone();
        let audit_log = self.audit_log.clone();
        let encryption_enabled = self.encryption_enabled;
        let privacy_mgr = self.privacy_mgr.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => {
                        match accept {
                            Ok((stream, _)) => {
                                let store = store.clone();
                                let loop_svc = loop_svc.clone();
                                let running = running.clone();
                                let version = version.clone();
                                let last_decision = last_decision.clone();
                                let last_attention = last_attention.clone();
                                let stop_rx = stop_rx.clone();
                                let ui_version = ui_version.clone();
                                let settings_mgr = settings_mgr.clone();
                                let audit_log = audit_log.clone();
                                let privacy_mgr = privacy_mgr.clone();
                                tokio::spawn(async move {
                                    handle_connection(
                                        stream, store, loop_svc, running, version,
                                        last_decision, last_attention, stop_rx, ui_version,
                                        settings_mgr, audit_log, encryption_enabled, privacy_mgr,
                                    ).await;
                                });
                            }
                            Err(e) => {
                                tracing::warn!("UI accept error: {e}");
                            }
                        }
                    }
                    _ = stop_rx.changed() => {
                        break;
                    }
                }
            }
            tracing::info!("UI server stopped");
        });

        self.stop_tx = Some(stop_tx);
        Ok(())
    }

    /// Signal the server to stop.
    pub fn signal_stop(&self) {
        if let Some(tx) = &self.stop_tx {
            let _ = tx.send(true);
        }
    }

    /// Build a snapshot of the current state.
    pub async fn snapshot(&self) -> UiSnapshot {
        let cycle_count = {
            let svc = self.loop_svc.lock().await;
            svc.cycle_count()
        };
        let entity_count = self.store.entity_count().await;
        let relationship_count = self.store.relationship_count().await;
        let recent_reflections = {
            let svc = self.loop_svc.lock().await;
            let log = svc.reflection_log();
            let end = log.len();
            let start = if end > 20 { end - 20 } else { 0 };
            log.range(start..).cloned().collect()
        };
        let last_decision_raw = self.last_decision.lock().ok().and_then(|d| d.clone());
        let last_attention_raw = self.last_attention.lock().ok().and_then(|a| a.clone());

        UiSnapshot {
            version: self.version.load(Ordering::Acquire),
            running: self.running.load(Ordering::Acquire),
            cycle_count,
            attention: last_attention_raw.map(|a| AttentionSnapshot {
                outcome: format!("{:?}", a.outcome),
                reason: a.reason,
                confidence: a.confidence,
                signals: a
                    .signals
                    .iter()
                    .map(|s| SignalSnapshot {
                        name: s.name.to_string(),
                        strength: s.strength,
                        description: s.description.clone(),
                    })
                    .collect(),
            }),
            last_decision: last_decision_raw.map(|d| {
                let (dt, msg, reason) = match &d {
                    Decision::Wait => ("Wait", None, None),
                    Decision::Communicate {
                        message, reason, ..
                    } => ("Communicate", Some(message.clone()), Some(reason.clone())),
                    Decision::UpdateMemory {
                        entity_name,
                        reason,
                        ..
                    } => (
                        "UpdateMemory",
                        Some(entity_name.clone()),
                        Some(reason.clone()),
                    ),
                    Decision::Execute { action, .. } => ("Execute", Some(action.clone()), None),
                };
                DecisionSnapshot {
                    decision_type: dt.into(),
                    message: msg,
                    reason,
                }
            }),
            entity_count,
            relationship_count,
            recent_reflections,
            observation_debug: None,
        }
    }
}

// ── HTTP request/response ─────────────────────────────────

/// Trust-related state passed to API handlers.
pub struct TrustState {
    pub settings_mgr: Option<DynSettingsManager>,
    pub audit_log: Option<Arc<StdMutex<super::AuditLog>>>,
    pub encryption_enabled: bool,
    pub privacy_mgr: Option<super::PrivacyManager>,
}

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

fn json_response(body: &str) -> Vec<u8> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    [headers.as_bytes(), body.as_bytes()].concat()
}

fn html_response(body: &str) -> Vec<u8> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    [headers.as_bytes(), body.as_bytes()].concat()
}

fn error_response(code: u16, message: &str) -> Vec<u8> {
    let body = format!(r#"{{"error":"{message}"}}"#);
    let headers = format!(
        "HTTP/1.1 {code} {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status_text(code),
        body.len()
    );
    [headers.as_bytes(), body.as_bytes()].concat()
}

fn not_found() -> Vec<u8> {
    error_response(404, "not found")
}

// ── Connection handler ────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn handle_connection(
    stream: tokio::net::TcpStream,
    store: Arc<dyn WorldModelStore>,
    loop_svc: Arc<Mutex<CognitiveLoopService>>,
    running: Arc<AtomicBool>,
    version: Arc<AtomicU64>,
    last_decision: Arc<StdMutex<Option<Decision>>>,
    last_attention: Arc<StdMutex<Option<AttentionDecision>>>,
    mut stop_rx: watch::Receiver<bool>,
    ui_version: Arc<AtomicU64>,
    settings_mgr: Option<DynSettingsManager>,
    audit_log: Option<Arc<StdMutex<super::AuditLog>>>,
    encryption_enabled: bool,
    privacy_mgr: Option<super::PrivacyManager>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut request_line = String::new();

    if buf_reader.read_line(&mut request_line).await.is_err() {
        return;
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    // Read headers
    let mut content_length: usize = 0;
    loop {
        let mut header = String::new();
        if buf_reader.read_line(&mut header).await.is_err() || header.trim().is_empty() {
            break;
        }
        if header.to_lowercase().starts_with("content-length:") {
            if let Some(val) = header.split(':').nth(1) {
                content_length = val.trim().parse().unwrap_or(0);
            }
        }
    }

    // Read body if present
    let mut body = Vec::new();
    if content_length > 0 {
        body.resize(content_length, 0);
        if buf_reader.read_exact(&mut body).await.is_err() {
            return;
        }
    }

    let query = path.splitn(2, '?').nth(1).unwrap_or("");
    let path_only = path.splitn(2, '?').next().unwrap_or(path.as_str());

    // SSE stream: must be handled before the normal response match to keep the writer connection open
    if method == "GET" && path_only == "/events" {
        handle_sse(&mut writer, &mut stop_rx, &version, &ui_version).await;
        return;
    }

    let response = match (method.as_str(), path_only) {
        ("GET", "/") => html_response(INDEX_HTML),
        ("GET", "/api/status") => {
            handle_status(
                &store,
                &loop_svc,
                &running,
                &last_decision,
                &last_attention,
                &version,
            )
            .await
        }
        ("GET", "/api/entities") => handle_entities(&store).await,
        ("GET", "/api/relationships") => handle_relationships(&store).await,
        ("GET", "/api/reflections") => handle_reflections(&loop_svc).await,
        ("GET", "/api/attention") => handle_attention(&last_attention).await,
        ("GET", "/api/settings") => handle_get_settings(&settings_mgr).await,
        ("POST", "/api/settings") => handle_put_settings(&settings_mgr, &body).await,
        ("GET", "/api/permissions") => handle_get_permissions(&settings_mgr).await,
        ("POST", "/api/permissions") => handle_toggle_permission(&settings_mgr, &body).await,
        ("GET", "/api/activity") => handle_get_activity(&audit_log).await,
        ("GET", "/api/explain") => handle_explain(&store, query).await,
        ("POST", "/api/forget") => handle_forget(&store, &body).await,
        ("POST", "/api/correct") => handle_correct(&store, &body).await,
        ("POST", "/api/merge") => handle_merge(&store, &body).await,
        ("GET", "/api/trust/status") => {
            handle_trust_status(&store, &audit_log, encryption_enabled, &privacy_mgr).await
        }
        ("GET", "/api/encrypt") => handle_toggle_encrypt(&privacy_mgr, &audit_log).await,
        ("POST", "/api/backup") => handle_create_backup(&privacy_mgr, &audit_log).await,
        ("POST", "/api/restore") => handle_restore_backup(&privacy_mgr, &audit_log).await,
        ("POST", "/chat") => {
            handle_chat(&loop_svc, &body, &last_decision, &last_attention, &version).await
        }
        _ => not_found(),
    };
    let _ = writer.write_all(&response).await;
}

async fn handle_status(
    store: &Arc<dyn WorldModelStore>,
    loop_svc: &Arc<Mutex<CognitiveLoopService>>,
    running: &Arc<AtomicBool>,
    last_decision: &Arc<StdMutex<Option<Decision>>>,
    last_attention: &Arc<StdMutex<Option<AttentionDecision>>>,
    version: &Arc<AtomicU64>,
) -> Vec<u8> {
    let cycle_count = {
        let svc = loop_svc.lock().await;
        svc.cycle_count()
    };
    let entity_count = store.entity_count().await;
    let relationship_count = store.relationship_count().await;

    let last_decision_raw = last_decision.lock().ok().and_then(|d| d.clone());
    let last_attention_raw = last_attention.lock().ok().and_then(|a| a.clone());

    let snapshot = serde_json::to_string(&serde_json::json!({
        "version": version.load(Ordering::Acquire),
        "running": running.load(Ordering::Acquire),
        "cycle_count": cycle_count,
        "entity_count": entity_count,
        "relationship_count": relationship_count,
        "last_decision": last_decision_raw.map(|d| {
            let (dt, msg, reason) = match &d {
                Decision::Wait => ("Wait", None::<String>, None),
                Decision::Communicate { message, reason, .. } => ("Communicate", Some(message.clone()), Some(reason.clone())),
                Decision::UpdateMemory { entity_name, reason, .. } => ("UpdateMemory", Some(entity_name.clone()), Some(reason.clone())),
                Decision::Execute { action, .. } => ("Execute", Some(action.clone()), None),
            };
            serde_json::json!({
                "type": dt,
                "message": msg,
                "reason": reason,
            })
        }),
        "last_attention": last_attention_raw.map(|a| serde_json::json!({
            "outcome": format!("{:?}", a.outcome),
            "reason": a.reason,
            "confidence": a.confidence,
            "signals": a.signals.iter().map(|s| serde_json::json!({
                "name": s.name,
                "strength": s.strength,
                "description": s.description,
            })).collect::<Vec<_>>(),
        })),
    })).unwrap();
    json_response(&snapshot)
}

async fn handle_entities(store: &Arc<dyn WorldModelStore>) -> Vec<u8> {
    let entities = store.all_entities().await;
    let json = serde_json::to_string(&serde_json::json!({
        "count": entities.len(),
        "entities": entities.iter().map(|e| serde_json::json!({
            "id": e.id.to_string(),
            "name": e.name,
            "entity_type": e.entity_type,
            "importance": e.importance,
            "confidence": e.confidence,
            "version": e.version,
            "lifecycle": format!("{:?}", e.lifecycle),
            "created_at": e.created_at.as_secs(),
            "updated_at": e.updated_at.as_secs(),
        })).collect::<Vec<_>>(),
    }))
    .unwrap();
    json_response(&json)
}

async fn handle_relationships(store: &Arc<dyn WorldModelStore>) -> Vec<u8> {
    let relationships = store.all_relationships().await;

    // Build a lookup for entity names
    let entities = store.all_entities().await;
    let names: std::collections::HashMap<_, _> =
        entities.iter().map(|e| (e.id, e.name.clone())).collect();

    let json = serde_json::to_string(&serde_json::json!({
        "count": relationships.len(),
        "relationships": relationships.iter().map(|r| serde_json::json!({
            "id": r.id.to_string(),
            "source": names.get(&r.source_id).map(|s| s.as_str()).unwrap_or("?"),
            "target": names.get(&r.target_id).map(|s| s.as_str()).unwrap_or("?"),
            "type": r.relationship_type,
            "weight": r.weight,
            "confidence": r.confidence,
        })).collect::<Vec<_>>(),
    }))
    .unwrap();
    json_response(&json)
}

async fn handle_reflections(loop_svc: &Arc<Mutex<CognitiveLoopService>>) -> Vec<u8> {
    let reflections = {
        let svc = loop_svc.lock().await;
        let log = svc.reflection_log();
        let end = log.len();
        let start = if end > 50 { end - 50 } else { 0 };
        log.range(start..).cloned().collect::<Vec<_>>()
    };
    let json = serde_json::to_string(&serde_json::json!({
        "count": reflections.len(),
        "reflections": reflections,
    }))
    .unwrap();
    json_response(&json)
}

async fn handle_attention(last_attention: &Arc<StdMutex<Option<AttentionDecision>>>) -> Vec<u8> {
    let attention = last_attention.lock().ok().and_then(|a| a.clone());
    let json = serde_json::to_string(&serde_json::json!({
        "attention": attention.map(|a| serde_json::json!({
            "outcome": format!("{:?}", a.outcome),
            "reason": a.reason,
            "confidence": a.confidence,
            "signals": a.signals.iter().map(|s| serde_json::json!({
                "name": s.name,
                "strength": s.strength,
                "description": s.description,
            })).collect::<Vec<_>>(),
        })),
    }))
    .unwrap();
    json_response(&json)
}

#[derive(serde::Deserialize)]
struct ChatMessage {
    message: String,
}

async fn handle_chat(
    loop_svc: &Arc<Mutex<CognitiveLoopService>>,
    body: &[u8],
    last_decision: &Arc<StdMutex<Option<Decision>>>,
    last_attention: &Arc<StdMutex<Option<AttentionDecision>>>,
    version: &Arc<AtomicU64>,
) -> Vec<u8> {
    let msg: ChatMessage = match serde_json::from_slice(body) {
        Ok(m) => m,
        Err(e) => return error_response(400, &format!("invalid JSON: {e}")),
    };

    let mut svc = loop_svc.lock().await;
    let decision = svc.cycle(&msg.message).await;

    // Record for UI
    if let Ok(mut d) = last_decision.lock() {
        *d = Some(decision.clone());
    }
    version.fetch_add(1, Ordering::Release);

    let (dt, msg_text, reason) = match &decision {
        Decision::Wait => ("Wait", None::<String>, None),
        Decision::Communicate {
            message, reason, ..
        } => ("Communicate", Some(message.clone()), Some(reason.clone())),
        Decision::UpdateMemory {
            entity_name,
            reason,
            ..
        } => (
            "UpdateMemory",
            Some(entity_name.clone()),
            Some(reason.clone()),
        ),
        Decision::Execute { action, .. } => ("Execute", Some(action.clone()), None),
    };

    let json = serde_json::to_string(&serde_json::json!({
        "decision": {
            "type": dt,
            "message": msg_text,
            "reason": reason,
        }
    }))
    .unwrap();
    json_response(&json)
}

async fn handle_get_settings(settings_mgr: &Option<DynSettingsManager>) -> Vec<u8> {
    let settings = match settings_mgr {
        Some(mgr) => {
            let guard = mgr.lock().unwrap();
            serde_json::to_value(guard.get()).unwrap_or_default()
        }
        None => serde_json::json!(null),
    };
    let json = serde_json::to_string(&serde_json::json!({
        "settings": settings,
    }))
    .unwrap();
    json_response(&json)
}

async fn handle_put_settings(settings_mgr: &Option<DynSettingsManager>, body: &[u8]) -> Vec<u8> {
    let new_settings: crate::companion_host::CompanionSettings = match serde_json::from_slice(body)
    {
        Ok(s) => s,
        Err(e) => return error_response(400, &format!("invalid settings JSON: {e}")),
    };

    match settings_mgr {
        Some(mgr) => {
            let mut guard = mgr.lock().unwrap();
            guard.settings = new_settings;
            if let Err(e) = guard.save() {
                return error_response(500, &format!("failed to save settings: {e}"));
            }
        }
        None => return error_response(500, "settings manager not available"),
    }

    json_response(r#"{"status":"ok"}"#)
}

// ── Trust API handlers ─────────────────────────────────────

async fn handle_get_permissions(settings_mgr: &Option<DynSettingsManager>) -> Vec<u8> {
    let permissions = match settings_mgr {
        Some(mgr) => {
            let guard = mgr.lock().unwrap();
            let settings = guard.get();
            serde_json::to_value(&settings.permissions).unwrap_or_default()
        }
        None => serde_json::json!(null),
    };
    let json = serde_json::to_string(&serde_json::json!({
        "permissions": permissions,
    }))
    .unwrap();
    json_response(&json)
}

async fn handle_toggle_permission(
    settings_mgr: &Option<DynSettingsManager>,
    body: &[u8],
) -> Vec<u8> {
    let req: std::collections::HashMap<String, String> = match serde_json::from_slice(body) {
        Ok(m) => m,
        Err(e) => return error_response(400, &format!("invalid JSON: {e}")),
    };
    let source = match req.get("source") {
        Some(s) => s,
        None => return error_response(400, "missing 'source' field"),
    };
    let state = match req.get("state") {
        Some(s) => s,
        None => return error_response(400, "missing 'state' field"),
    };

    match settings_mgr {
        Some(mgr) => {
            let mut guard = mgr.lock().unwrap();
            let cat = super::permissions::PermissionRegistry::category_for_source(source);
            match state.as_str() {
                "granted" => guard
                    .settings
                    .permissions
                    .set(&cat, super::permissions::PermissionState::Granted),
                "denied" => guard
                    .settings
                    .permissions
                    .set(&cat, super::permissions::PermissionState::Denied),
                "not_requested" => guard
                    .settings
                    .permissions
                    .set(&cat, super::permissions::PermissionState::NotRequested),
                _ => {
                    return error_response(
                        400,
                        "state must be 'granted', 'denied', or 'not_requested'",
                    );
                }
            }
            if let Err(e) = guard.save() {
                return error_response(500, &format!("failed to save settings: {e}"));
            }
        }
        None => return error_response(500, "settings manager not available"),
    }

    json_response(r#"{"status":"ok"}"#)
}

async fn handle_get_activity(audit_log: &Option<Arc<StdMutex<super::AuditLog>>>) -> Vec<u8> {
    let entries = match audit_log {
        Some(log) => {
            let guard = log.lock().unwrap();
            let all = guard.entries();
            all.into_iter().take(50).collect::<Vec<_>>()
        }
        None => Vec::new(),
    };
    let json = serde_json::to_string(&serde_json::json!({
        "count": entries.len(),
        "entries": entries,
    }))
    .unwrap();
    json_response(&json)
}

async fn handle_explain(store: &Arc<dyn WorldModelStore>, query: &str) -> Vec<u8> {
    let params: std::collections::HashMap<&str, &str> = query
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            Some((parts.next()?, parts.next()?))
        })
        .collect();
    let name_str = match params.get("name") {
        Some(n) => n,
        None => return error_response(400, "missing 'name' query parameter"),
    };

    let entities = store.search_entities_by_name(name_str).await;
    let explanation = if let Some(entity) = entities.first() {
        super::explain::explain_entity(store, entity)
    } else {
        format!("No entity found matching '{name_str}'.")
    };
    let json = serde_json::to_string(&serde_json::json!({
        "name": name_str,
        "explanation": explanation,
    }))
    .unwrap();
    json_response(&json)
}

async fn handle_forget(store: &Arc<dyn WorldModelStore>, body: &[u8]) -> Vec<u8> {
    #[derive(serde::Deserialize)]
    struct ForgetReq {
        name: Option<String>,
        all: Option<bool>,
    }
    let req: ForgetReq = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => return error_response(400, &format!("invalid JSON: {e}")),
    };

    if req.all == Some(true) {
        super::memory_control::forget_all(store).await;
        return json_response(r#"{"status":"ok","action":"forgot_all"}"#);
    }

    match req.name {
        Some(name) => {
            let result = super::memory_control::forget_entity(store, &name).await;
            match result {
                Ok(msg) => json_response(
                    &serde_json::to_string(&serde_json::json!({
                        "status":"ok","action":"forgotten","message":msg,"name":name
                    }))
                    .unwrap(),
                ),
                Err(_) => json_response(
                    &serde_json::to_string(&serde_json::json!({
                        "status":"ok","action":"not_found","name":name
                    }))
                    .unwrap(),
                ),
            }
        }
        None => error_response(400, "provide 'name' or 'all: true'"),
    }
}

async fn handle_correct(store: &Arc<dyn WorldModelStore>, body: &[u8]) -> Vec<u8> {
    #[derive(serde::Deserialize)]
    struct CorrectReq {
        name: String,
        field: String,
        new_value: String,
    }
    let req: CorrectReq = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => return error_response(400, &format!("invalid JSON: {e}")),
    };
    let new_name = if req.field == "name" {
        Some(req.new_value.as_str())
    } else {
        None
    };
    let new_type = if req.field == "entity_type" || req.field == "type" {
        Some(req.new_value.as_str())
    } else {
        None
    };
    let result = super::memory_control::correct_entity(store, &req.name, new_name, new_type).await;
    match result {
        Ok(msg) => json_response(
            &serde_json::to_string(&serde_json::json!({
                "status":"ok","updated":true,"message":msg
            }))
            .unwrap(),
        ),
        Err(e) => json_response(
            &serde_json::to_string(&serde_json::json!({
                "status":"ok","updated":false,"message":format!("{e}")
            }))
            .unwrap(),
        ),
    }
}

async fn handle_merge(store: &Arc<dyn WorldModelStore>, body: &[u8]) -> Vec<u8> {
    #[derive(serde::Deserialize)]
    struct MergeReq {
        source: String,
        target: String,
    }
    let req: MergeReq = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => return error_response(400, &format!("invalid JSON: {e}")),
    };
    match super::memory_control::merge_entities(store, &req.source, &req.target).await {
        Ok(msg) => json_response(
            &serde_json::to_string(&serde_json::json!({
                "status":"ok","message":msg
            }))
            .unwrap(),
        ),
        Err(e) => error_response(500, &format!("merge failed: {e}")),
    }
}

async fn handle_trust_status(
    store: &Arc<dyn WorldModelStore>,
    audit_log: &Option<Arc<StdMutex<super::AuditLog>>>,
    encryption_enabled: bool,
    privacy_mgr: &Option<super::PrivacyManager>,
) -> Vec<u8> {
    let all_entities = store.all_entities().await;
    let entity_count = all_entities.len();
    let all_relationships = store.all_relationships().await;
    let relationship_count = all_relationships.len();

    let activity_count = match audit_log {
        Some(log) => log.lock().unwrap().entries().len(),
        None => 0,
    };

    let backup_count = match privacy_mgr {
        Some(mgr) => {
            let backup_dir = mgr.config().backup_path.clone();
            std::fs::read_dir(&backup_dir)
                .ok()
                .map(|e| e.count())
                .unwrap_or(0)
        }
        None => 0,
    };

    let json = serde_json::to_string(&serde_json::json!({
        "encryption_enabled": encryption_enabled,
        "entity_count": entity_count,
        "relationship_count": relationship_count,
        "activity_count": activity_count,
        "backup_count": backup_count,
    }))
    .unwrap();
    json_response(&json)
}

/// Enable or disable encryption.
async fn handle_toggle_encrypt(
    privacy_mgr: &Option<super::PrivacyManager>,
    audit_log: &Option<Arc<StdMutex<super::AuditLog>>>,
) -> Vec<u8> {
    let _ = audit_log;
    let enabled = match privacy_mgr {
        Some(mgr) => {
            let result = if mgr.is_encryption_enabled() {
                mgr.disable_encryption().map(|_| false)
            } else {
                mgr.enable_encryption().map(|_| true)
            };
            match result {
                Ok(val) => val,
                Err(e) => return error_response(500, &format!("encryption toggle failed: {e}")),
            }
        }
        None => return error_response(500, "privacy manager not available"),
    };
    json_response(
        &serde_json::to_string(&serde_json::json!({
            "status":"ok","encryption_enabled":enabled
        }))
        .unwrap(),
    )
}

async fn handle_create_backup(
    privacy_mgr: &Option<super::PrivacyManager>,
    audit_log: &Option<Arc<StdMutex<super::AuditLog>>>,
) -> Vec<u8> {
    let _ = audit_log;
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let default_path = std::path::PathBuf::from(&home)
        .join(".local")
        .join("share")
        .join("ai-os-companion")
        .join("settings.json");

    match privacy_mgr {
        Some(mgr) => {
            let path = mgr.create_backup(&default_path);
            match path {
                Ok(p) => json_response(
                    &serde_json::to_string(&serde_json::json!({
                        "status":"ok","backup_path":p.to_string_lossy()
                    }))
                    .unwrap(),
                ),
                Err(e) => error_response(500, &format!("backup failed: {e}")),
            }
        }
        None => error_response(500, "privacy manager not available"),
    }
}

async fn handle_restore_backup(
    privacy_mgr: &Option<super::PrivacyManager>,
    audit_log: &Option<Arc<StdMutex<super::AuditLog>>>,
) -> Vec<u8> {
    let _ = audit_log;
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let default_path = std::path::PathBuf::from(&home)
        .join(".local")
        .join("share")
        .join("ai-os-companion")
        .join("settings.json");

    match privacy_mgr {
        Some(mgr) => match mgr.restore_latest_backup(&default_path) {
            Ok(()) => json_response(r#"{"status":"ok"}"#),
            Err(e) => error_response(500, &format!("restore failed: {e}")),
        },
        None => error_response(500, "privacy manager not available"),
    }
}

async fn handle_sse(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    stop_rx: &mut watch::Receiver<bool>,
    version: &Arc<AtomicU64>,
    _ui_version: &Arc<AtomicU64>,
) {
    let sse_headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nConnection: keep-alive\r\n\r\n";
    if writer.write_all(sse_headers.as_bytes()).await.is_err() {
        return;
    }
    let _ = writer.flush().await;

    let mut last_version = version.load(Ordering::Acquire);
    loop {
        tokio::select! {
            _ = stop_rx.changed() => {
                break;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                let v = version.load(Ordering::Acquire);
                if v != last_version {
                    last_version = v;
                    let data = format!("data: {v}\n\n");
                    if writer.write_all(data.as_bytes()).await.is_err() {
                        break;
                    }
                    let _ = writer.flush().await;
                }
            }
        }
    }
}

// ── Embedded HTML/CSS/JS ──────────────────────────────────

const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>AI Companion</title>
<style>
  :root {
    --bg: #0d1117;
    --surface: #161b22;
    --border: #30363d;
    --text: #c9d1d9;
    --text-dim: #8b949e;
    --accent: #58a6ff;
    --green: #3fb950;
    --red: #f85149;
    --yellow: #d29922;
    --font: 'SFMono-Regular', 'Consolas', 'Liberation Mono', monospace;
  }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { background: var(--bg); color: var(--text); font-family: var(--font); font-size: 13px; height: 100vh; display: flex; flex-direction: column; }
  .status-bar { background: var(--surface); border-bottom: 1px solid var(--border); padding: 8px 16px; display: flex; gap: 20px; align-items: center; font-size: 12px; flex-shrink: 0; }
  .status-dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
  .status-dot.on { background: var(--green); box-shadow: 0 0 6px var(--green); }
  .status-dot.off { background: var(--red); }
  .tab-bar { background: var(--surface); border-bottom: 1px solid var(--border); display: flex; flex-shrink: 0; }
  .tab { padding: 10px 18px; cursor: pointer; color: var(--text-dim); border-bottom: 2px solid transparent; user-select: none; }
  .tab:hover { color: var(--text); background: var(--hover); }
  .tab.active { color: var(--accent); border-bottom-color: var(--accent); }
  .settings-group { margin-bottom:16px; }
  .settings-group h4 { margin:0 0 8px 0; color:var(--text-dim); font-weight:500; font-size:13px; text-transform:uppercase; letter-spacing:0.5px; }
  .setting-row { display:flex; align-items:center; justify-content:space-between; padding:6px 0; border-bottom:1px solid var(--border); }
  .setting-row label { font-size:13px; color:var(--text); }
  .setting-row input[type="number"] { width:80px; padding:4px 6px; border:1px solid var(--border); border-radius:4px; background:var(--bg); color:var(--text); font-size:13px; text-align:right; }
  .setting-row input[type="text"] { width:200px; padding:4px 6px; border:1px solid var(--border); border-radius:4px; background:var(--bg); color:var(--text); font-size:13px; }
  .setting-row select { padding:4px 6px; border:1px solid var(--border); border-radius:4px; background:var(--bg); color:var(--text); font-size:13px; }
  .setting-row .toggle { position:relative; width:40px; height:22px; cursor:pointer; }
  .setting-row .toggle input { opacity:0; width:0; height:0; }
  .setting-row .toggle .slider { position:absolute; inset:0; background:var(--border); border-radius:11px; transition:0.2s; }
  .setting-row .toggle .slider::before { content:''; position:absolute; height:18px; width:18px; left:2px; bottom:2px; background:var(--text-dim); border-radius:50%; transition:0.2s; }
  .setting-row .toggle input:checked + .slider { background:var(--accent); }
  .setting-row .toggle input:checked + .slider::before { transform:translateX(18px); background:white; }
  #saveSettingsBtn { margin-top:12px; padding:8px 20px; background:var(--accent); color:white; border:none; border-radius:6px; cursor:pointer; font-size:13px; }
  #saveSettingsBtn:hover { filter:brightness(1.1); }
  #settingsStatus { margin-top:8px; font-size:12px; }
  .content { flex: 1; overflow-y: auto; padding: 16px; }
  .chat-area { border-top: 1px solid var(--border); background: var(--surface); padding: 12px 16px; display: flex; gap: 8px; flex-shrink: 0; }
  .chat-area input { flex: 1; background: var(--bg); border: 1px solid var(--border); color: var(--text); padding: 8px 12px; border-radius: 6px; font-family: var(--font); font-size: 13px; outline: none; }
  .chat-area input:focus { border-color: var(--accent); }
  .chat-area button { background: var(--accent); color: #fff; border: none; padding: 8px 16px; border-radius: 6px; cursor: pointer; font-family: var(--font); font-size: 13px; }
  .chat-area button:hover { opacity: 0.9; }
  .panel { display: none; }
  .panel.active { display: block; }
  .stat-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 12px; margin-bottom: 16px; }
  .stat-card { background: var(--surface); border: 1px solid var(--border); border-radius: 6px; padding: 12px; }
  .stat-label { color: var(--text-dim); font-size: 11px; text-transform: uppercase; margin-bottom: 4px; }
  .stat-value { font-size: 18px; font-weight: bold; }
  .entity-item, .reflection-item { background: var(--surface); border: 1px solid var(--border); border-radius: 4px; padding: 8px 12px; margin-bottom: 4px; font-size: 12px; }
  .entity-name { color: var(--accent); }
  .entity-type { color: var(--text-dim); }
  .signal-bar { height: 4px; border-radius: 2px; background: var(--border); margin-top: 4px; overflow: hidden; }
  .signal-fill { height: 100%; background: var(--accent); border-radius: 2px; transition: width 0.3s; }
  .chat-log { max-height: 200px; overflow-y: auto; margin-bottom: 8px; padding: 8px; background: var(--bg); border: 1px solid var(--border); border-radius: 4px; font-size: 12px; }
  .chat-msg { padding: 4px 0; border-bottom: 1px solid var(--border); }
  .chat-msg:last-child { border-bottom: none; }
  .msg-user { color: var(--accent); }
  .msg-companion { color: var(--green); }
  h3 { color: var(--text-dim); font-size: 12px; text-transform: uppercase; margin-bottom: 8px; margin-top: 16px; letter-spacing: 0.5px; }
  .empty { color: var(--text-dim); font-style: italic; font-size: 12px; }
  .signal-row { display: flex; justify-content: space-between; font-size: 11px; margin-top: 2px; }
</style>
</head>
<body>
<div class="status-bar">
  <span><span class="status-dot" id="statusDot"></span> <span id="statusText">offline</span></span>
  <span>Cycle: <span id="cycleCount">0</span></span>
  <span>Entities: <span id="entityCount">0</span></span>
  <span>Relationships: <span id="relCount">0</span></span>
  <span style="color:var(--text-dim)">|</span>
  <span id="attentionDisplay" style="color:var(--text-dim)">attention: —</span>
</div>

<div class="tab-bar">
  <div class="tab active" data-tab="status">Status</div>
  <div class="tab" data-tab="world">World Model</div>
  <div class="tab" data-tab="chat">Chat</div>
  <div class="tab" data-tab="reflections">Reflections</div>
  <div class="tab" data-tab="diagnostics">Diagnostics</div>
  <div class="tab" data-tab="settings">Settings</div>
  <div class="tab" data-tab="dashboard" id="devDashboardTab" style="display:none">Dashboard</div>
  <div class="tab" data-tab="trust">Trust</div>
</div>

<div class="content">
  <!-- Status Panel -->
  <div class="panel active" id="panel-status">
    <h3>Latest Decision</h3>
    <div id="lastDecision" class="empty">No decision yet</div>
    <h3>Recent Reflections</h3>
    <div id="recentReflections"><div class="empty">No reflections yet</div></div>
    <h3>Attention State</h3>
    <div id="attentionDetails"><div class="empty">No attention data</div></div>
  </div>

  <!-- World Model Panel -->
  <div class="panel" id="panel-world">
    <div class="stat-grid">
      <div class="stat-card"><div class="stat-label">Entities</div><div class="stat-value" id="wmEntities">0</div></div>
      <div class="stat-card"><div class="stat-label">Relationships</div><div class="stat-value" id="wmRels">0</div></div>
    </div>
    <h3>Entities</h3>
    <div id="entityList"><div class="empty">No entities</div></div>
    <h3>Relationships</h3>
    <div id="relList"><div class="empty">No relationships</div></div>
  </div>

  <!-- Chat Panel -->
  <div class="panel" id="panel-chat">
    <div id="chatLog" class="chat-log"></div>
  </div>

  <!-- Reflections Panel -->
  <div class="panel" id="panel-reflections">
    <div id="reflectionList"><div class="empty">No reflections recorded yet</div></div>
  </div>

  <!-- Diagnostics Panel -->
  <div class="panel" id="panel-diagnostics">
    <div id="diagContent"><div class="empty">Waiting for diagnostics data...</div></div>
  </div>

  <!-- Settings Panel -->
  <div class="panel" id="panel-settings">
    <h3>Settings</h3>
    <div id="settingsForm">
      <div class="empty">Loading settings...</div>
    </div>
    <div id="settingsStatus" style="margin-top:8px;font-size:12px;color:var(--green)"></div>
  </div>

  <!-- Dashboard Panel (Developer Mode) -->
  <div class="panel" id="panel-dashboard">
    <h3>Developer Dashboard</h3>
    <div id="dashboardContent"><div class="empty">Waiting for data...</div></div>
  </div>

  <!-- Trust Panel -->
  <div class="panel" id="panel-trust">
    <h3>Trust, Privacy & Security</h3>
    <div class="stat-grid">
      <div class="stat-card"><div class="stat-label">Encryption</div><div class="stat-value" id="trustEncryption">—</div></div>
      <div class="stat-card"><div class="stat-label">Entities</div><div class="stat-value" id="trustEntities">0</div></div>
      <div class="stat-card"><div class="stat-label">Relationships</div><div class="stat-value" id="trustRelationships">0</div></div>
      <div class="stat-card"><div class="stat-label">Activity Events</div><div class="stat-value" id="trustActivityCount">0</div></div>
      <div class="stat-card"><div class="stat-label">Backups</div><div class="stat-value" id="trustBackups">0</div></div>
    </div>

    <div class="settings-group" style="margin-top:12px">
      <h4>Encryption</h4>
      <div class="setting-row">
        <label>Encrypt World Model at Rest</label>
        <label class="toggle"><input type="checkbox" id="trustEncryptToggle"><span class="slider"></span></label>
      </div>
    </div>

    <div class="settings-group">
      <h4>Backup & Restore</h4>
      <button id="trustBackupBtn" style="padding:6px 14px;background:var(--accent);color:white;border:none;border-radius:4px;cursor:pointer;font-size:12px;margin-right:8px">Create Backup</button>
      <button id="trustRestoreBtn" style="padding:6px 14px;background:var(--yellow);color:white;border:none;border-radius:4px;cursor:pointer;font-size:12px">Restore Latest</button>
      <div id="trustBackupStatus" style="margin-top:6px;font-size:11px;color:var(--text-dim)"></div>
    </div>

    <div class="settings-group">
      <h4>Observation Permissions</h4>
      <div id="trustPermissions"><div class="empty">Loading...</div></div>
    </div>

    <div class="settings-group">
      <h4>Recent Activity</h4>
      <div id="trustActivity"><div class="empty">Loading...</div></div>
    </div>

    <div class="settings-group">
      <h4>Memory Management</h4>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:8px">
        <input type="text" id="trustEntityName" placeholder="Entity name" style="flex:1;min-width:120px;padding:5px 8px;border:1px solid var(--border);border-radius:4px;background:var(--bg);color:var(--text);font-size:12px">
        <button id="trustExplainBtn" style="padding:5px 12px;background:var(--accent);color:white;border:none;border-radius:4px;cursor:pointer;font-size:12px">Explain</button>
        <button id="trustForgetBtn" style="padding:5px 12px;background:var(--red);color:white;border:none;border-radius:4px;cursor:pointer;font-size:12px">Forget</button>
        <button id="trustForgetAllBtn" style="padding:5px 12px;background:var(--red);color:white;border:none;border-radius:4px;cursor:pointer;font-size:12px">Forget All</button>
      </div>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:4px">
        <input type="text" id="trustCorrectField" placeholder="Field to correct (name/type/etc)" style="flex:1;min-width:100px;padding:5px 8px;border:1px solid var(--border);border-radius:4px;background:var(--bg);color:var(--text);font-size:12px">
        <input type="text" id="trustCorrectValue" placeholder="New value" style="flex:1;min-width:100px;padding:5px 8px;border:1px solid var(--border);border-radius:4px;background:var(--bg);color:var(--text);font-size:12px">
        <button id="trustCorrectBtn" style="padding:5px 12px;background:var(--yellow);color:white;border:none;border-radius:4px;cursor:pointer;font-size:12px">Correct</button>
      </div>
      <div id="trustMemoryResult" style="margin-top:4px;font-size:11px;color:var(--text-dim)"></div>
    </div>

    <div class="settings-group">
      <h4>Data Retention</h4>
      <p style="font-size:11px;color:var(--text-dim);margin-bottom:4px">Observations: 7 days &bull; Reflections: 30 days &bull; Activity log: 1000 entries</p>
    </div>
  </div>
</div>

<div class="chat-area">
  <input type="text" id="chatInput" placeholder="Type a message..." autofocus>
  <button id="chatSend">Send</button>
</div>

<script>
const API = '';
const STATUS_POLL = 2000;

// ── Tab Switching ─────────────────────────────────────────
document.querySelectorAll('.tab').forEach(tab => {
  tab.addEventListener('click', () => {
    document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.panel').forEach(p => p.classList.remove('active'));
    tab.classList.add('active');
    document.getElementById('panel-' + tab.dataset.tab).classList.add('active');
    refreshCurrentPanel();
  });
});

function refreshCurrentPanel() {
  const active = document.querySelector('.tab.active');
  if (active) refreshPanel(active.dataset.tab);
}

// ── SSE for live updates ──────────────────────────────────
let evtSource = null;
function connectSSE() {
  if (evtSource) evtSource.close();
  evtSource = new EventSource('/events');
  evtSource.onmessage = () => { refreshStatus(); };
  evtSource.onerror = () => { setTimeout(connectSSE, 3000); };
}
connectSSE();
setInterval(() => { refreshStatus(); }, STATUS_POLL);

// ── Chat ──────────────────────────────────────────────────
const chatInput = document.getElementById('chatInput');
const chatSend = document.getElementById('chatSend');
const chatLog = document.getElementById('chatLog');

chatSend.addEventListener('click', sendMessage);
chatInput.addEventListener('keydown', e => { if (e.key === 'Enter') sendMessage(); });

async function sendMessage() {
  const msg = chatInput.value.trim();
  if (!msg) return;
  chatInput.value = '';

  addChatMsg('user', msg);

  try {
    const resp = await fetch('/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: msg }),
    });
    const data = await resp.json();
    if (data.decision && data.decision.message) {
      addChatMsg('companion', data.decision.message);
    }
    refreshStatus();
  } catch (e) {
    addChatMsg('companion', '[error: ' + e.message + ']');
  }
}

function addChatMsg(role, text) {
  const div = document.createElement('div');
  div.className = 'chat-msg msg-' + role;
  div.textContent = (role === 'user' ? 'You: ' : 'AI: ') + text;
  chatLog.appendChild(div);
  chatLog.scrollTop = chatLog.scrollHeight;
}

// ── Status Refresh ────────────────────────────────────────
async function refreshStatus() {
  try {
    const resp = await fetch('/api/status');
    const data = await resp.json();

    // Status bar
    const dot = document.getElementById('statusDot');
    dot.className = 'status-dot ' + (data.running ? 'on' : 'off');
    document.getElementById('statusText').textContent = data.running ? 'online' : 'offline';
    document.getElementById('cycleCount').textContent = data.cycle_count;
    document.getElementById('entityCount').textContent = data.entity_count;
    document.getElementById('relCount').textContent = data.relationship_count;

    // Attention
    const att = data.last_attention;
    if (att) {
      document.getElementById('attentionDisplay').textContent = 'attention: ' + att.outcome;
    }

    // Last decision
    const dec = data.last_decision;
    const decEl = document.getElementById('lastDecision');
    if (dec) {
      let html = '<div class="entity-item"><strong>' + dec.type + '</strong>';
      if (dec.message) html += '<br><span style="color:var(--green)">' + escHtml(dec.message) + '</span>';
      if (dec.reason) html += '<br><span style="color:var(--text-dim);font-size:11px">' + escHtml(dec.reason) + '</span>';
      html += '</div>';
      decEl.innerHTML = html;
    }

    // Attention details
    const attEl = document.getElementById('attentionDetails');
    if (att) {
      let html = '';
      html += '<div class="entity-item"><strong>Outcome:</strong> ' + att.outcome + ' (confidence: ' + (att.confidence * 100).toFixed(0) + '%)</div>';
      html += '<div class="entity-item" style="margin-top:4px">' + escHtml(att.reason) + '</div>';
      if (att.signals && att.signals.length) {
        html += '<h3 style="margin-top:8px">Signals</h3>';
        for (const s of att.signals) {
          const pct = Math.min(s.strength * 100, 100);
          html += '<div class="entity-item">';
          html += '<div class="signal-row"><span>' + escHtml(s.name) + '</span><span>' + pct.toFixed(0) + '%</span></div>';
          html += '<div class="signal-bar"><div class="signal-fill" style="width:' + pct + '%"></div></div>';
          html += '<div style="font-size:11px;color:var(--text-dim);margin-top:2px">' + escHtml(s.description) + '</div>';
          html += '</div>';
        }
      }
      attEl.innerHTML = html;
    } else {
      attEl.innerHTML = '<div class="empty">No attention data</div>';
    }

    // Recent reflections in status panel
    try {
      const refResp = await fetch('/api/reflections');
      const refData = await refResp.json();
      const refs = refData.reflections || [];
      const recentEl = document.getElementById('recentReflections');
      if (refs.length > 0) {
        const lastFew = refs.slice(-5).reverse();
        recentEl.innerHTML = lastFew.map(r =>
          '<div class="reflection-item">' +
          '  <div><span style="color:var(--accent)">#' + r.cycle + '</span> ' +
          '    <span style="color:var(--green)">' + r.outcome + '</span></div>' +
          '  <div style="color:var(--text-dim);font-size:11px">' + escHtml(r.summary) + '</div>' +
          '</div>'
        ).join('');
      } else {
        recentEl.innerHTML = '<div class="empty">No reflections yet</div>';
      }
    } catch(e) {}

    // Refresh other panels based on active tab
    refreshPanel(document.querySelector('.tab.active')?.dataset?.tab);
  } catch(e) { /* silently retry */ }
}

async function refreshPanel(tab) {
  if (!tab) return;

  if (tab === 'world') {
    try {
      const [eResp, rResp] = await Promise.all([
        fetch('/api/entities'),
        fetch('/api/relationships')
      ]);
      const eData = await eResp.json();
      const rData = await rResp.json();

      document.getElementById('wmEntities').textContent = eData.count || 0;
      document.getElementById('wmRels').textContent = rData.count || 0;

      const el = document.getElementById('entityList');
      if (eData.entities && eData.entities.length) {
        el.innerHTML = eData.entities.map(e =>
          '<div class="entity-item">' +
          '  <span class="entity-name">' + escHtml(e.name) + '</span> ' +
          '  <span class="entity-type">[' + escHtml(e.entity_type) + ']</span>' +
          '  <span style="color:var(--text-dim);font-size:11px"> i=' + e.importance.toFixed(2) + ' c=' + e.confidence.toFixed(2) + '</span>' +
          '</div>'
        ).join('');
      } else {
        el.innerHTML = '<div class="empty">No entities</div>';
      }

      const rl = document.getElementById('relList');
      if (rData.relationships && rData.relationships.length) {
        rl.innerHTML = rData.relationships.map(r =>
          '<div class="entity-item">' +
          '  <span style="color:var(--accent)">' + escHtml(r.source) + '</span> ' +
          '  <span style="color:var(--text-dim)">' + escHtml(r.type) + '</span> ' +
          '  <span style="color:var(--accent)">' + escHtml(r.target) + '</span>' +
          '  <span style="color:var(--text-dim);font-size:11px"> w=' + r.weight.toFixed(2) + '</span>' +
          '</div>'
        ).join('');
      } else {
        rl.innerHTML = '<div class="empty">No relationships</div>';
      }
    } catch(e) {}
  }

  if (tab === 'reflections') {
    try {
      const resp = await fetch('/api/reflections');
      const data = await resp.json();
      const el = document.getElementById('reflectionList');
      if (data.reflections && data.reflections.length) {
        el.innerHTML = data.reflections.slice().reverse().map(r =>
          '<div class="reflection-item">' +
          '  <div><span style="color:var(--accent)">#' + r.cycle + '</span> ' +
          '    <span style="color:var(--green)">' + r.outcome + '</span> ' +
          '    <span style="color:var(--text-dim);font-size:11px">' + new Date(r.timestamp?.secs * 1000).toLocaleString() + '</span></div>' +
          '  <div style="color:var(--text-dim);font-size:11px">' + escHtml(r.summary) + '</div>' +
          '</div>'
        ).join('');
      } else {
        el.innerHTML = '<div class="empty">No reflections recorded yet</div>';
      }
    } catch(e) {}
  }

  if (tab === 'diagnostics') {
    try {
      const eResp = await fetch('/api/entities');
      const rResp = await fetch('/api/relationships');
      const sResp = await fetch('/api/status');
      const eData = await eResp.json();
      const rData = await rResp.json();
      const sData = await sResp.json();

      let html = '<div class="stat-grid">';
      html += '<div class="stat-card"><div class="stat-label">Cycles</div><div class="stat-value">' + sData.cycle_count + '</div></div>';
      html += '<div class="stat-card"><div class="stat-label">Entities</div><div class="stat-value">' + sData.entity_count + '</div></div>';
      html += '<div class="stat-card"><div class="stat-label">Relationships</div><div class="stat-value">' + sData.relationship_count + '</div></div>';
      html += '<div class="stat-card"><div class="stat-label">Version</div><div class="stat-value">' + sData.version + '</div></div>';
      html += '</div>';

      html += '<h3>All Entities</h3>';
      if (eData.entities && eData.entities.length) {
        html += '<table style="width:100%;border-collapse:collapse;font-size:12px">';
        html += '<tr style="color:var(--text-dim);border-bottom:1px solid var(--border)"><th style="text-align:left;padding:4px">Name</th><th style="text-align:left">Type</th><th style="text-align:right">Imp</th><th style="text-align:right">Conf</th><th style="text-align:right">V</th></tr>';
        for (const e of eData.entities) {
          html += '<tr style="border-bottom:1px solid var(--border)"><td style="padding:4px;color:var(--accent)">' + escHtml(e.name) + '</td><td>' + escHtml(e.entity_type) + '</td><td style="text-align:right">' + e.importance.toFixed(2) + '</td><td style="text-align:right">' + e.confidence.toFixed(2) + '</td><td style="text-align:right">' + e.version + '</td></tr>';
        }
        html += '</table>';
      } else {
        html += '<div class="empty">No entities</div>';
      }

      html += '<h3>All Relationships</h3>';
      if (rData.relationships && rData.relationships.length) {
        html += '<table style="width:100%;border-collapse:collapse;font-size:12px">';
        html += '<tr style="color:var(--text-dim);border-bottom:1px solid var(--border)"><th style="text-align:left;padding:4px">Source</th><th style="text-align:left">Type</th><th style="text-align:left">Target</th><th style="text-align:right">Weight</th></tr>';
        for (const r of rData.relationships) {
          html += '<tr style="border-bottom:1px solid var(--border)"><td style="padding:4px;color:var(--accent)">' + escHtml(r.source) + '</td><td>' + escHtml(r.type) + '</td><td style="color:var(--accent)">' + escHtml(r.target) + '</td><td style="text-align:right">' + r.weight.toFixed(2) + '</td></tr>';
        }
        html += '</table>';
      } else {
        html += '<div class="empty">No relationships</div>';
      }

      document.getElementById('diagContent').innerHTML = html;
    } catch(e) {}
  }

  if (tab === 'settings') {
    loadSettings();
  }

  if (tab === 'dashboard') {
    refreshDashboard();
  }

  if (tab === 'trust') {
    refreshTrust();
  }
}

// ── Settings ───────────────────────────────────────────────
let currentSettings = {};
let llmProviderList = ['openai','anthropic','ollama','custom'];

function buildSettingsForm(settings) {
  currentSettings = settings;
  let html = '';

  // LLM section
  html += '<div class="settings-group"><h4>LLM Provider</h4>';
  html += settingSelect('llm_provider', 'Provider', llmProviderList, settings.llm_provider);
  html += settingInput('llm_api_key', 'API Key', settings.llm_api_key || '', 'text', '(stored in config)');
  html += settingInput('llm_model', 'Model', settings.llm_model || '');
  html += '</div>';

  // Observation section
  html += '<div class="settings-group"><h4>Observation</h4>';
  html += settingInput('obs_interval', 'Interval (secs)', settings.observation?.interval_secs ?? 60, 'number');
  html += settingInput('obs_observation_window', 'Window (secs)', settings.observation?.observation_window ?? 300, 'number');
  html += settingToggle('obs_linux_processes', 'Linux Processes', settings.observation?.linux_processes ?? true);
  html += settingToggle('obs_linux_system', 'Linux System Load', settings.observation?.linux_system ?? true);
  html += '</div>';

  // Notification section
  html += '<div class="settings-group"><h4>Notifications</h4>';
  html += settingToggle('notif_enabled', 'Enabled', settings.notifications?.enabled ?? true);
  html += settingToggle('notif_sound', 'Play Sound', settings.notifications?.sound ?? true);
  html += settingInput('notif_quiet_hours_start', 'Quiet Hours Start', settings.notifications?.quiet_hours_start ?? '');
  html += settingInput('notif_quiet_hours_end', 'Quiet Hours End', settings.notifications?.quiet_hours_end ?? '');
  html += '</div>';

  // Cognitive section
  html += '<div class="settings-group"><h4>Cognitive</h4>';
  html += settingInput('attention_sensitivity', 'Attention Sensitivity', settings.attention_sensitivity ?? 0.7, 'number');
  html += settingInput('reflection_frequency', 'Reflection Frequency', settings.reflection_frequency ?? 5, 'number');
  html += '</div>';

  // Platform section
  html += '<div class="settings-group"><h4>Platform</h4>';
  html += settingToggle('autostart', 'Autostart on Login', settings.autostart ?? false);
  html += settingToggle('dev_mode', 'Developer Mode', settings.dev_mode ?? false);
  html += settingToggle('debug_logging', 'Debug Logging', settings.debug_logging ?? false);
  html += settingInput('wm_storage_path', 'WM Storage Path', settings.wm_storage_path || '');
  html += settingInput('ui_port', 'UI Port', settings.ui_port ?? 3030, 'number');
  html += '</div>';

  html += '<button id="saveSettingsBtn">Save Settings</button>';
  document.getElementById('settingsForm').innerHTML = html;

  // Wire save button
  document.getElementById('saveSettingsBtn').addEventListener('click', saveSettings);
}

function settingInput(key, label, value, type, placeholder) {
  const t = type || 'text';
  const ph = placeholder || '';
  return '<div class="setting-row">' +
    '<label for="s-' + key + '">' + label + '</label>' +
    '<input id="s-' + key + '" type="' + t + '" value="' + escHtml(String(value ?? '')) + '" placeholder="' + escHtml(ph) + '">' +
    '</div>';
}

function settingToggle(key, label, checked) {
  return '<div class="setting-row">' +
    '<label for="s-' + key + '">' + label + '</label>' +
    '<label class="toggle"><input id="s-' + key + '" type="checkbox"' + (checked ? ' checked' : '') + '><span class="slider"></span></label>' +
    '</div>';
}

function settingSelect(key, label, options, value) {
  let html = '<div class="setting-row">' +
    '<label for="s-' + key + '">' + label + '</label>' +
    '<select id="s-' + key + '">';
  for (const opt of options) {
    html += '<option value="' + opt + '"' + (opt === value ? ' selected' : '') + '>' + opt + '</option>';
  }
  html += '</select></div>';
  return html;
}

function collectSettings() {
  const v = (id) => document.getElementById('s-' + id);
  return {
    llm_provider: v('llm_provider').value,
    llm_api_key: v('llm_api_key').value,
    llm_model: v('llm_model').value,
    observation: {
      interval_secs: parseFloat(v('obs_interval').value) || 60,
      observation_window: parseFloat(v('obs_observation_window').value) || 300,
      linux_processes: v('obs_linux_processes').checked,
      linux_system: v('obs_linux_system').checked,
    },
    notifications: {
      enabled: v('notif_enabled').checked,
      sound: v('notif_sound').checked,
      quiet_hours_start: v('notif_quiet_hours_start').value,
      quiet_hours_end: v('notif_quiet_hours_end').value,
    },
    attention_sensitivity: parseFloat(v('attention_sensitivity').value) || 0.7,
    reflection_frequency: parseInt(v('reflection_frequency').value) || 5,
    autostart: v('autostart').checked,
    dev_mode: v('dev_mode').checked,
    debug_logging: v('debug_logging').checked,
    wm_storage_path: v('wm_storage_path').value,
    ui_port: parseInt(v('ui_port').value) || 3030,
  };
}

async function loadSettings() {
  try {
    const resp = await fetch('/api/settings');
    const settings = await resp.json();
    buildSettingsForm(settings);
    // Show/hide dev dashboard tab
    const devTab = document.getElementById('devDashboardTab');
    if (devTab) devTab.style.display = settings.dev_mode ? '' : 'none';
  } catch(e) {
    document.getElementById('settingsForm').innerHTML = '<div style="color:var(--red)">Failed to load settings: ' + e.message + '</div>';
  }
}

async function saveSettings() {
  const settings = collectSettings();
  const statusEl = document.getElementById('settingsStatus');
  try {
    const resp = await fetch('/api/settings', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings),
    });
    const data = await resp.json();
    if (data.status === 'ok') {
      statusEl.textContent = 'Settings saved.';
      statusEl.style.color = 'var(--green)';
      // Refresh dev dashboard visibility
      const devTab = document.getElementById('devDashboardTab');
      if (devTab) devTab.style.display = settings.dev_mode ? '' : 'none';
    } else {
      statusEl.textContent = 'Error: ' + (data.error || 'unknown');
      statusEl.style.color = 'var(--red)';
    }
  } catch(e) {
    statusEl.textContent = 'Error: ' + e.message;
    statusEl.style.color = 'var(--red)';
  }
  setTimeout(() => { statusEl.textContent = ''; }, 3000);
}

// ── Developer Dashboard ────────────────────────────────────
async function refreshDashboard() {
  try {
    const [sResp, eResp, rResp, refResp] = await Promise.all([
      fetch('/api/status'),
      fetch('/api/entities'),
      fetch('/api/relationships'),
      fetch('/api/reflections'),
    ]);
    const sData = await sResp.json();
    const eData = await eResp.json();
    const rData = await rResp.json();
    const refData = await refResp.json();

    let html = '<div class="stat-grid">';
    html += '<div class="stat-card"><div class="stat-label">Version</div><div class="stat-value">' + (sData.version || '?') + '</div></div>';
    html += '<div class="stat-card"><div class="stat-label">Cycles</div><div class="stat-value">' + sData.cycle_count + '</div></div>';
    html += '<div class="stat-card"><div class="stat-label">Entities</div><div class="stat-value">' + sData.entity_count + '</div></div>';
    html += '<div class="stat-card"><div class="stat-label">Relationships</div><div class="stat-value">' + sData.relationship_count + '</div></div>';
    html += '<div class="stat-card"><div class="stat-label">Reflections</div><div class="stat-value">' + (refData.reflections?.length || 0) + '</div></div>';
    html += '</div>';

    // Entity table
    html += '<h3>Entity Details</h3>';
    if (eData.entities && eData.entities.length) {
      html += '<table style="width:100%;border-collapse:collapse;font-size:12px">';
      html += '<tr style="color:var(--text-dim);border-bottom:1px solid var(--border)"><th style="text-align:left;padding:4px">Name</th><th style="text-align:left">Type</th><th style="text-align:right">Imp</th><th style="text-align:right">Conf</th><th style="text-align:right">V</th></tr>';
      for (const e of eData.entities) {
        html += '<tr style="border-bottom:1px solid var(--border)"><td style="padding:4px;color:var(--accent)">' + escHtml(e.name) + '</td><td>' + escHtml(e.entity_type) + '</td><td style="text-align:right">' + e.importance.toFixed(2) + '</td><td style="text-align:right">' + e.confidence.toFixed(2) + '</td><td style="text-align:right">' + e.version + '</td></tr>';
      }
      html += '</table>';
    } else {
      html += '<div class="empty">No entities</div>';
    }

    // Relationship table
    html += '<h3>Relationship Details</h3>';
    if (rData.relationships && rData.relationships.length) {
      html += '<table style="width:100%;border-collapse:collapse;font-size:12px">';
      html += '<tr style="color:var(--text-dim);border-bottom:1px solid var(--border)"><th style="text-align:left;padding:4px">Source</th><th style="text-align:left">Type</th><th style="text-align:left">Target</th><th style="text-align:right">Weight</th></tr>';
      for (const r of rData.relationships) {
        html += '<tr style="border-bottom:1px solid var(--border)"><td style="padding:4px;color:var(--accent)">' + escHtml(r.source) + '</td><td>' + escHtml(r.type) + '</td><td style="color:var(--accent)">' + escHtml(r.target) + '</td><td style="text-align:right">' + r.weight.toFixed(2) + '</td></tr>';
      }
      html += '</table>';
    } else {
      html += '<div class="empty">No relationships</div>';
    }

    document.getElementById('dashboardContent').innerHTML = html;
  } catch(e) {
    document.getElementById('dashboardContent').innerHTML = '<div style="color:var(--red)">Failed to load dashboard: ' + e.message + '</div>';
  }
}

// ── Trust Dashboard ─────────────────────────────────────────
async function refreshTrust() {
  try {
    const [tResp, pResp, aResp] = await Promise.all([
      fetch('/api/trust/status'),
      fetch('/api/permissions'),
      fetch('/api/activity'),
    ]);
    const tData = await tResp.json();
    const pData = await pResp.json();
    const aData = await aResp.json();

    // Status cards
    document.getElementById('trustEncryption').textContent = tData.encryption_enabled ? 'Enabled' : 'Disabled';
    document.getElementById('trustEncryption').style.color = tData.encryption_enabled ? 'var(--green)' : 'var(--red)';
    document.getElementById('trustEntities').textContent = tData.entity_count || 0;
    document.getElementById('trustRelationships').textContent = tData.relationship_count || 0;
    document.getElementById('trustActivityCount').textContent = tData.activity_count || 0;
    document.getElementById('trustBackups').textContent = tData.backup_count || 0;

    // Encryption toggle
    document.getElementById('trustEncryptToggle').checked = tData.encryption_enabled;

    // Permissions table
    const permEl = document.getElementById('trustPermissions');
    const perms = pData.permissions?.permissions || {};
    const keys = Object.keys(perms);
    if (keys.length) {
      let html = '<table style="width:100%;border-collapse:collapse;font-size:12px">';
      html += '<tr style="color:var(--text-dim);border-bottom:1px solid var(--border)"><th style="text-align:left;padding:4px">Source</th><th style="text-align:left">Description</th><th style="text-align:center;width:80px">Status</th><th style="width:60px"></th></tr>';
      for (const k of keys) {
        const p = perms[k];
        const granted = p.state === 'Granted';
        const desc = p.description || '';
        html += '<tr style="border-bottom:1px solid var(--border)">';
        html += '<td style="padding:4px;color:var(--accent)">' + escHtml(k) + '</td>';
        html += '<td style="font-size:11px;color:var(--text-dim)">' + escHtml(desc) + '</td>';
        html += '<td style="text-align:center;color:' + (granted ? 'var(--green)' : 'var(--red)') + '">' + (granted ? 'Granted' : 'Denied') + '</td>';
        html += '<td><button class="perm-toggle" data-source="' + k + '" data-state="' + (granted ? 'denied' : 'granted') + '" style="padding:2px 8px;font-size:10px;border:1px solid var(--border);border-radius:3px;background:var(--bg);color:var(--text);cursor:pointer">Toggle</button></td>';
        html += '</tr>';
      }
      html += '</table>';
      permEl.innerHTML = html;

      // Wire permission toggle buttons
      document.querySelectorAll('.perm-toggle').forEach(btn => {
        btn.addEventListener('click', async () => {
          const source = btn.dataset.source;
          const newState = btn.dataset.state;
          await fetch('/api/permissions', {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify({source, state: newState}),
          });
          refreshTrust();
        });
      });
    } else {
      permEl.innerHTML = '<div class="empty">No permission entries</div>';
    }

    // Activity log
    const actEl = document.getElementById('trustActivity');
    const entries = aData.entries || [];
    if (entries.length) {
      let html = '<table style="width:100%;border-collapse:collapse;font-size:11px">';
      html += '<tr style="color:var(--text-dim);border-bottom:1px solid var(--border)"><th style="text-align:left;padding:4px">Time</th><th style="text-align:left">Event</th><th style="text-align:left">Detail</th></tr>';
      for (const e of entries.slice(0, 20)) {
        html += '<tr style="border-bottom:1px solid var(--border)">';
        html += '<td style="padding:3px 4px;white-space:nowrap">' + (e.timestamp || '') + '</td>';
        html += '<td style="padding:3px 4px;color:var(--accent)">' + escHtml(e.category || '') + '</td>';
        html += '<td style="padding:3px 4px;font-size:10px;color:var(--text-dim)">' + escHtml(e.detail || '') + '</td>';
        html += '</tr>';
      }
      html += '</table>';
      actEl.innerHTML = html;
    } else {
      actEl.innerHTML = '<div class="empty">No activity recorded yet</div>';
    }
  } catch(e) {
    document.getElementById('trustPermissions').innerHTML = '<div style="color:var(--red)">Failed to load trust data: ' + e.message + '</div>';
  }
}

// ── Trust Event Wiring ──────────────────────────────────────
// Encryption toggle
document.getElementById('trustEncryptToggle')?.addEventListener('change', async function() {
  await fetch('/api/encrypt');
  refreshTrust();
});

// Backup
document.getElementById('trustBackupBtn')?.addEventListener('click', async () => {
  const resp = await fetch('/api/backup', { method: 'POST' });
  const data = await resp.json();
  const el = document.getElementById('trustBackupStatus');
  if (data.status === 'ok') {
    el.textContent = 'Backup created: ' + (data.backup_path || 'ok');
    el.style.color = 'var(--green)';
  } else {
    el.textContent = 'Backup failed: ' + (data.error || 'unknown');
    el.style.color = 'var(--red)';
  }
  refreshTrust();
});

// Restore
document.getElementById('trustRestoreBtn')?.addEventListener('click', async () => {
  if (!confirm('Restore the latest backup? This will overwrite current settings.')) return;
  const resp = await fetch('/api/restore', { method: 'POST' });
  const data = await resp.json();
  const el = document.getElementById('trustBackupStatus');
  if (data.status === 'ok') {
    el.textContent = 'Restored latest backup.';
    el.style.color = 'var(--green)';
  } else {
    el.textContent = 'Restore failed: ' + (data.error || 'unknown');
    el.style.color = 'var(--red)';
  }
  refreshTrust();
});

// Explain
document.getElementById('trustExplainBtn')?.addEventListener('click', async () => {
  const name = document.getElementById('trustEntityName').value.trim();
  if (!name) return;
  const resp = await fetch('/api/explain?name=' + encodeURIComponent(name));
  const data = await resp.json();
  const el = document.getElementById('trustMemoryResult');
  if (data.explanation) {
    el.innerHTML = '<strong>Explain: ' + escHtml(data.name) + '</strong><br>' + escHtml(data.explanation);
    el.style.color = 'var(--text)';
  } else {
    el.textContent = 'Not found: ' + name;
    el.style.color = 'var(--red)';
  }
});

// Forget
document.getElementById('trustForgetBtn')?.addEventListener('click', async () => {
  const name = document.getElementById('trustEntityName').value.trim();
  if (!name) return;
  const resp = await fetch('/api/forget', {
    method: 'POST',
    headers: {'Content-Type': 'application/json'},
    body: JSON.stringify({name}),
  });
  const data = await resp.json();
  const el = document.getElementById('trustMemoryResult');
  el.textContent = data.action === 'forgotten' ? 'Forgot: ' + name : 'Not found: ' + name;
  el.style.color = data.action === 'forgotten' ? 'var(--green)' : 'var(--yellow)';
  refreshTrust();
});

// Forget All
document.getElementById('trustForgetAllBtn')?.addEventListener('click', async () => {
  if (!confirm('Forget ALL entities? This cannot be undone.')) return;
  const resp = await fetch('/api/forget', {
    method: 'POST',
    headers: {'Content-Type': 'application/json'},
    body: JSON.stringify({all: true}),
  });
  const data = await resp.json();
  const el = document.getElementById('trustMemoryResult');
  el.textContent = data.status === 'ok' ? 'All entities forgotten.' : 'Error: ' + (data.error || '');
  el.style.color = data.status === 'ok' ? 'var(--green)' : 'var(--red)';
  refreshTrust();
});

// Correct
document.getElementById('trustCorrectBtn')?.addEventListener('click', async () => {
  const name = document.getElementById('trustEntityName').value.trim();
  const field = document.getElementById('trustCorrectField').value.trim();
  const newValue = document.getElementById('trustCorrectValue').value.trim();
  if (!name || !field || !newValue) return;
  const resp = await fetch('/api/correct', {
    method: 'POST',
    headers: {'Content-Type': 'application/json'},
    body: JSON.stringify({name, field, new_value: newValue}),
  });
  const data = await resp.json();
  const el = document.getElementById('trustMemoryResult');
  if (data.status === 'ok') {
    el.textContent = data.updated ? 'Corrected ' + field + ' for ' + name : 'Entity not found: ' + name;
    el.style.color = data.updated ? 'var(--green)' : 'var(--yellow)';
  } else {
    el.textContent = 'Error: ' + (data.error || '');
    el.style.color = 'var(--red)';
  }
  refreshTrust();
});

function escHtml(s) {
  if (!s) return '';
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

// Initial load
refreshStatus();
</script>
</body>
</html>
"##;
