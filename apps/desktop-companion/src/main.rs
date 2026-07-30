use std::path::PathBuf;
use std::process;

use brain_coordinator::companion_host::settings::load_settings_manager;
use brain_coordinator::companion_host::{CompanionHost, UiConfig};

mod autostart;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // ── Load settings (determines all paths and behaviour) ──
    let settings_mgr = load_settings_manager().await;
    let settings = settings_mgr.lock().unwrap().get().clone();

    let wm_path = expand_path(&settings.wm_storage_path);
    let ui_port = settings.ui_port;

    tracing::info!("AI-OS Companion v{} starting...", env!("CARGO_PKG_VERSION"));
    tracing::info!("World Model: {}", wm_path.display());
    tracing::info!("Web UI: http://localhost:{}", ui_port);

    // ── Telegram configuration ──
    let telegram_token = std::env::var("AI_OS_TELEGRAM_BOT_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .or_else(|| {
            let t = settings.telegram.bot_token.clone();
            if t.is_empty() { None } else { Some(t) }
        });
    let telegram_enabled = settings.telegram.enabled && telegram_token.is_some();
    if telegram_enabled {
        tracing::info!("Telegram channel configured");
    }

    // ── Configure LLM provider env vars ──
    // The DefaultModelProvider reads these env vars at construction time.
    // Set them before any intelligence components are initialized.
    match settings.llm_provider.as_str() {
        "nvapi" => {
            if !settings.nvapi_token.is_empty() {
                std::env::set_var("AI_OS_LLM_PROVIDER", "nvapi");
                std::env::set_var("AI_OS_NVAPI_TOKEN", &settings.nvapi_token);
                std::env::set_var("AI_OS_LLM_API_BASE", "https://integrate.api.nvidia.com/v1");
                tracing::info!("NVAPI provider configured");
            } else {
                tracing::warn!("llm_provider is 'nvapi' but nvapi_token is not set in settings");
            }
        }
        "openai" => {
            std::env::set_var("AI_OS_LLM_PROVIDER", "openai");
            tracing::info!("OpenAI provider configured");
        }
        "anthropic" => {
            std::env::set_var("AI_OS_LLM_PROVIDER", "anthropic");
            tracing::info!("Anthropic provider configured");
        }
        _ => {
            std::env::set_var("AI_OS_LLM_PROVIDER", "auto");
        }
    }

    // ── Configure autostart if enabled ──
    if settings.autostart {
        let bin_path = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "desktop-companion".into());
        match autostart::enable_autostart(&bin_path) {
            Ok(()) => tracing::info!("Autostart enabled"),
            Err(e) => tracing::warn!("Failed to enable autostart: {e}"),
        }
    } else {
        let _ = autostart::disable_autostart();
    }

    // ── Ensure WM storage directory exists ──
    if let Some(parent) = wm_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // ── Build and start the Companion Host ──
    let mut host = CompanionHost::load(&wm_path).await?;

    // Configure observation sources
    host.with_default_observation(settings.observation.interval_secs);

    // Configure native notification callback
    host.set_notification_callback(std::sync::Arc::new(|message, reason| {
        // Native notification dispatch (sync, best-effort)
        let _ = notify(message, reason);
    }));

    // Start the web UI
    host.with_ui(UiConfig { port: ui_port });

    // Start the Telegram channel (if enabled)
    if telegram_enabled {
        if let Some(token) = telegram_token {
            host.with_telegram(token, settings.telegram.poll_interval_secs);
        }
    }

    // ── Signal handling for graceful shutdown ──
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    // Handle SIGINT (Ctrl+C)
    let shutdown_tx_for_sigint = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Received SIGINT, shutting down...");
        let _ = shutdown_tx_for_sigint.send(true);
    });

    // Handle SIGTERM (systemd/service manager stop)
    let shutdown_tx_for_sigterm = shutdown_tx.clone();
    tokio::spawn(async move {
        let mut sigterm = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        ).expect("Failed to create SIGTERM handler");
        sigterm.recv().await;
        tracing::info!("Received SIGTERM, shutting down...");
        let _ = shutdown_tx_for_sigterm.send(true);
    });

    // ── Start the host ──
    host.start().await?;
    tracing::info!("Companion is running. Press Ctrl+C to stop.");

    // ── Wait for shutdown signal ──
    let _ = shutdown_rx.changed().await;

    // ── Graceful shutdown ──
    tracing::info!("Stopping companion...");
    host.stop().await?;
    tracing::info!("Companion stopped. Goodbye!");

    Ok(())
}

/// Send a native desktop notification.
///
/// Uses `notify-send` on Linux (best-effort, no extra dependencies).
/// On systems without a notification daemon, this silently logs.
fn notify(message: &str, reason: &str) -> Result<(), std::io::Error> {
    let _ = process::Command::new("notify-send")
        .args([
            "--app-name",
            "AI-OS Companion",
            "--urgency",
            "normal",
            "--expire-time",
            "10000",
            &format!("AI Companion: {reason}"),
            message,
        ])
        .output();
    tracing::info!(message = %message, reason = %reason, "native notification");
    Ok(())
}

/// Expand `~` to the user's home directory.
fn expand_path(path: &std::path::Path) -> PathBuf {
    let s = path.to_string_lossy().to_string();
    if s.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(s.replacen('~', &home, 1))
    } else {
        path.to_path_buf()
    }
}
