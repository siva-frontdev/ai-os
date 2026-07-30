mod persistence;
pub use persistence::PersistenceManager;

pub mod settings;
pub use settings::load_settings_manager;
pub use settings::{
    CompanionSettings, DynSettingsManager, NotificationSettings, ObservationSettings,
    RetentionConfig, SettingsManager, TelegramSettings,
};

pub mod observation_loop;
pub mod sources;
pub mod ui;

pub mod telegram;

pub mod permissions;
pub use permissions::PermissionRegistry;

pub mod audit_log;
pub use audit_log::{ActivityCategory, AuditLog};

pub mod memory_control;

pub mod explain;

pub mod privacy;
pub use privacy::{EncryptionStatus, PrivacyConfig, PrivacyManager};

pub use observation_loop::{
    ObservationConfig, ObservationDebugInfo, ObservationEvent, ObservationLoop,
};
pub use ui::{CompanionUi, UiConfig};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use brain_core::types::Decision;
use intelligence_coordinator::DefaultCoordinator;
use intelligence_coordinator::world_understanding::WorldUnderstandingService;
use memory_storage::wm_store::{InMemoryWorldModelStore, WorldModelStore};
use tokio::sync::{Mutex, mpsc, watch};

use std::sync::Mutex as StdMutex;

use crate::cognitive_loop::CognitiveLoopService;
use crate::errors::CoordinatorError;

use self::sources::ObservationSource;
use self::telegram::{TelegramAdapter, TelegramStats};

/// Callback invoked when the companion makes a Communicate decision.
pub type NotificationCallback = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// CompanionHost manages the full lifecycle of the Persistent Personal Intelligence.
///
/// Includes continuous observation: polls available sources (desktop, time, system),
/// detects changes, and feeds new observations into the existing cognitive pipeline.
/// Optionally serves a web-based Companion UI on a local port.
pub struct CompanionHost {
    store: Arc<InMemoryWorldModelStore>,
    loop_svc: Arc<Mutex<CognitiveLoopService>>,
    obsv_loop: Option<ObservationLoop>,
    obsv_sources: StdMutex<Vec<Box<dyn ObservationSource>>>,
    persistence: Arc<PersistenceManager>,
    running: Arc<AtomicBool>,
    stop_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    ui: Arc<Mutex<Option<CompanionUi>>>,
    notification_cb: Arc<Mutex<Option<NotificationCallback>>>,
    settings: DynSettingsManager,
    audit_log: Arc<StdMutex<AuditLog>>,
    privacy_mgr: PrivacyManager,
    telegram: Arc<Mutex<Option<TelegramAdapter>>>,
}

impl CompanionHost {
    /// Create and initialize a CompanionHost.
    ///
    /// `wm_path` is the path to the JSON file where the World Model
    /// will be persisted. The file is created if it does not exist.
    /// On startup, existing state is loaded from this file.
    pub async fn load(wm_path: impl AsRef<Path>) -> Result<Self, CoordinatorError> {
        let wm_path = wm_path.as_ref().to_path_buf();
        let persistence = Arc::new(PersistenceManager::new(&wm_path));

        let store = Arc::new(Self::load_wm(&*persistence).await?);
        let coordinator = Arc::new(DefaultCoordinator::new());
        let understanding = WorldUnderstandingService::new(coordinator.clone());
        let loop_svc = Arc::new(Mutex::new(CognitiveLoopService::new(
            store.clone(),
            understanding,
            coordinator,
        )));

        // Load settings from default path
        let settings_manager = load_settings_manager().await;
        let settings_guard = settings_manager.lock().unwrap();
        let _settings = settings_guard.get().clone();
        let privacy_config = _settings.privacy.clone();
        drop(settings_guard);

        // Set up audit log
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let audit_path = PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("ai-os-companion")
            .join("audit_log.json");
        let audit_log = Arc::new(StdMutex::new(
            AuditLog::load(&audit_path).unwrap_or_else(|_| AuditLog::new(1000)),
        ));

        Ok(Self {
            store,
            loop_svc,
            obsv_loop: None,
            obsv_sources: StdMutex::new(Vec::new()),
            persistence,
            running: Arc::new(AtomicBool::new(false)),
            stop_tx: Arc::new(Mutex::new(None)),
            tasks: Arc::new(Mutex::new(Vec::new())),
            ui: Arc::new(Mutex::new(None)),
            notification_cb: Arc::new(Mutex::new(None)),
            settings: settings_manager,
            audit_log,
            privacy_mgr: PrivacyManager::new(privacy_config),
            telegram: Arc::new(Mutex::new(None)),
        })
    }

    /// Set the notification callback invoked when the companion decides to
    /// communicate. Used by the desktop companion to dispatch native
    /// notifications.
    pub fn set_notification_callback(&self, cb: NotificationCallback) {
        if let Ok(mut guard) = self.notification_cb.try_lock() {
            *guard = Some(cb);
        }
    }

    /// Get a reference to the settings manager.
    pub fn settings(&self) -> &DynSettingsManager {
        &self.settings
    }

    /// Apply settings to the running companion.
    /// This changes observation sources, reflection frequency, etc.
    pub async fn apply_settings(&self, new: CompanionSettings) -> Result<(), CoordinatorError> {
        {
            let mut guard = self.settings.lock().unwrap();
            guard.set(new)?;
        }
        // TODO: re-configure observation loop and reflection frequency
        // from the new settings. For now, settings persist on next start.
        Ok(())
    }

    /// Configure the observation loop with given sources.
    ///
    /// Must be called before `start()`. Replaces any previous config.
    pub fn with_observation_loop(
        &mut self,
        config: ObservationConfig,
        sources: Vec<Box<dyn ObservationSource>>,
    ) {
        let loop_ = ObservationLoop::new(config);
        for src in &sources {
            loop_.register(src.name());
        }
        if let Ok(mut stored) = self.obsv_sources.lock() {
            *stored = sources;
        }
        self.obsv_loop = Some(loop_);
    }

    /// Enable default observation sources (time, system).
    ///
    /// Desktop observation is not enabled by default because it requires
    /// a provider. Add it with `with_desktop_observation()`.
    pub fn with_default_observation(&mut self, interval_secs: u64) {
        let config = ObservationConfig {
            interval_secs,
            ..ObservationConfig::default()
        };
        let sources: Vec<Box<dyn ObservationSource>> = vec![
            Box::new(sources::TimeSource::new()),
            Box::new(sources::SystemSource::new()),
        ];
        self.with_observation_loop(config, sources);
    }

    /// Add a desktop observation source.
    pub fn with_desktop_observation(&mut self, provider: Box<dyn sources::DesktopProvider>) {
        let desktop: Box<dyn ObservationSource> = Box::new(sources::DesktopSource::new(provider));
        if let Some(loop_) = &self.obsv_loop {
            loop_.register(desktop.name());
        }
        if let Ok(mut stored) = self.obsv_sources.lock() {
            stored.push(desktop);
        }
    }

    /// Enable the Companion Web UI on the given port (default: 9876).
    ///
    /// Must be called before `start()`. The UI is served as a single-page
    /// web application at `http://localhost:<port>`.
    pub fn with_ui(&mut self, config: UiConfig) {
        let store: Arc<dyn WorldModelStore> = self.store.clone();
        let mut ui = CompanionUi::new(config, store, self.loop_svc.clone(), self.running.clone());
        ui.set_settings_manager(self.settings.clone());
        ui.set_audit_log(self.audit_log.clone());
        ui.set_privacy(&self.privacy_mgr);
        if let Ok(mut guard) = self.ui.try_lock() {
            *guard = Some(ui);
        }
    }

    /// Enable the Telegram channel.
    ///
    /// Creates a `TelegramAdapter` that polls the Telegram Bot API and
    /// feeds messages through the same cognitive pipeline as the Web UI.
    /// The Brain never knows the message originated from Telegram.
    ///
    /// Must be called before `start()`.
    pub fn with_telegram(&mut self, token: String, poll_interval_secs: u64) {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let identity_path = PathBuf::from(&home)
            .join(".local")
            .join("share")
            .join("ai-os-companion")
            .join("telegram_identities.json");
        let adapter = TelegramAdapter::new(
            self.loop_svc.clone(),
            token,
            poll_interval_secs,
            identity_path,
        );
        if let Ok(mut guard) = self.telegram.try_lock() {
            *guard = Some(adapter);
        }
        tracing::info!("Telegram channel configured (poll interval: {}s)", poll_interval_secs);
    }

    async fn load_wm(
        persistence: &PersistenceManager,
    ) -> Result<InMemoryWorldModelStore, CoordinatorError> {
        match persistence.load().await? {
            Some(persisted) => {
                let store = InMemoryWorldModelStore::new();
                for entity in &persisted.entities {
                    store.insert_entity(entity.clone()).await;
                }
                for rel in &persisted.relationships {
                    store.insert_relationship(rel).await;
                }
                tracing::info!(
                    "Loaded World Model from {} ({} entities, {} relationships)",
                    persistence.path().display(),
                    persisted.entities.len(),
                    persisted.relationships.len()
                );
                Ok(store)
            }
            None => {
                tracing::info!("No previous World Model found; starting fresh");
                Ok(InMemoryWorldModelStore::new())
            }
        }
    }

    /// Save the current World Model to disk.
    pub async fn save(&self) -> Result<(), CoordinatorError> {
        let entities = self.store.all_entities().await;
        let relationships = self.store.all_relationships().await;
        self.persistence.save(&entities, &relationships).await?;
        // Also persist the audit log
        if let Ok(guard) = self.audit_log.lock() {
            let _ = guard.save();
        }
        tracing::info!(
            "Saved World Model to {} ({} entities, {} relationships)",
            self.persistence.path().display(),
            entities.len(),
            relationships.len()
        );
        Ok(())
    }

    /// Start the Companion Host.
    ///
    /// 1. Spawns the observation loop (if configured) in a background task
    /// 2. Spawns a cognitive worker that processes observations through
    ///    the existing cognitive pipeline
    /// 3. Starts the web UI server (if configured)
    /// 4. Background cognitive ticks continue on schedule
    pub async fn start(&self) -> Result<(), CoordinatorError> {
        self.running.store(true, Ordering::Release);

        // Start the web UI server (if configured)
        if let Ok(mut guard) = self.ui.try_lock() {
            if let Some(ui) = guard.as_mut() {
                ui.start().await?;
                tracing::info!("Companion UI started on port {}", ui.port());
            }
        }

        let (stop_tx, loop_rx) = watch::channel(true);
        let save_stop_rx = loop_rx.clone();
        let telegram_stop_rx = loop_rx.clone();

        if let Some(obsv_loop) = &self.obsv_loop {
            // Filter sources by permissions from settings
            let permission_check = {
                let guard = self.settings.lock().unwrap();
                guard.get().permissions.clone()
            };
            let all_sources: Vec<Box<dyn ObservationSource>> = self
                .obsv_sources
                .lock()
                .map(|mut s| std::mem::take(&mut *s))
                .unwrap_or_default();
            let sources: Vec<Box<dyn ObservationSource>> = all_sources
                .into_iter()
                .filter(|src| {
                    let cat = PermissionRegistry::category_for_source(src.name());
                    let granted = permission_check.is_granted(cat);
                    if !granted {
                        tracing::info!("Observation source '{}' denied by permission", src.name());
                    }
                    granted
                })
                .collect();
            let (cognitive_tx, mut cognitive_rx) = mpsc::unbounded_channel::<String>();

            // Spawn observation loop
            let loop_task = obsv_loop.spawn(sources, cognitive_tx, loop_rx);
            self.tasks.lock().await.push(loop_task);

            // Spawn cognitive worker
            let loop_svc = self.loop_svc.clone();
            let stop_tx_for_worker = stop_tx.clone();
            let ui = self.ui.clone();
            let notif_cb = self.notification_cb.clone();
            let audit_log = self.audit_log.clone();
            let worker_task = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        Some(observation) = cognitive_rx.recv() => {
                            let decision = loop_svc.lock().await.cycle(&observation).await;
                            // Record observation in audit log
                            if let Ok(mut guard) = audit_log.lock() {
                                let summary = if observation.len() > 80 {
                                    format!("{}...", &observation[..77])
                                } else {
                                    observation.clone()
                                };
                                guard.record(
                                    crate::companion_host::ActivityCategory::Observation,
                                    summary,
                                    "",
                                );
                            }
                            // Notify the UI about new decisions
                            if let Ok(guard) = ui.try_lock() {
                                if let Some(ref ui_svc) = *guard {
                                    ui_svc.record_decision(&decision);
                                }
                            }
                            match &decision {
                                Decision::Communicate { message, reason, .. } => {
                                    tracing::info!(message = %message, "cognitive: communicate");
                            // Record communication in audit log
                            if let Ok(mut guard) = audit_log.lock() {
                                        // Use a shorter summary
                                        let summary = if message.len() > 80 {
                                            format!("{}...", &message[..77])
                                        } else {
                                            message.clone()
                                        };
                                        guard.record(
                                            crate::companion_host::ActivityCategory::Notification,
                                            summary,
                                            reason.clone(),
                                        );
                                    }
                                    // Dispatch native notification if callback is set
                                    if let Ok(guard) = notif_cb.try_lock() {
                                        if let Some(ref cb) = *guard {
                                            cb(message, reason);
                                        }
                                    }
                                }
                                Decision::UpdateMemory { .. } => {
                                    tracing::debug!("cognitive: update memory");
                                }
                                _ => {}
                            }
                        }
                        _ = stop_tx_for_worker.closed() => {
                            tracing::info!("cognitive worker stopping");
                            return;
                        }
                    }
                }
            });
            self.tasks.lock().await.push(worker_task);
        }

        // Spawn periodic auto-save (every 5 minutes)
        let save_store = self.store.clone();
        let save_persistence = self.persistence.clone();
        let save_tasks = self.tasks.clone();
        let mut save_stop = save_stop_rx;
        let save_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let entities = save_store.all_entities().await;
                        let relationships = save_store.all_relationships().await;
                        if let Err(e) = save_persistence.save(&entities, &relationships).await {
                            tracing::warn!("auto-save failed: {e}");
                        } else {
                            tracing::debug!("auto-saved world model ({} entities)", entities.len());
                        }
                    }
                    _ = save_stop.changed() => {
                        if !*save_stop.borrow() {
                            tracing::info!("auto-save stopping");
                            return;
                        }
                    }
                }
            }
        });
        save_tasks.lock().await.push(save_handle);

        // Start Telegram polling (if configured)
        if let Ok(guard) = self.telegram.try_lock() {
            if let Some(ref adapter) = *guard {
                adapter.start(telegram_stop_rx).await;
                tracing::info!("Telegram polling started");
            }
        }

        *self.stop_tx.lock().await = Some(stop_tx);
        tracing::info!("Companion Host started");
        Ok(())
    }

    /// Stop the Companion Host.
    ///
    /// Signals background tasks to stop, saves the World Model,
    /// and waits for tasks to complete.
    pub async fn stop(&self) -> Result<(), CoordinatorError> {
        self.running.store(false, Ordering::Release);

        // Signal UI server to stop
        if let Ok(guard) = self.ui.try_lock() {
            if let Some(ui) = guard.as_ref() {
                ui.signal_stop();
            }
        }

        // Signal Telegram polling to stop
        if let Ok(guard) = self.telegram.try_lock() {
            if let Some(ref adapter) = *guard {
                adapter.signal_stop();
            }
        }

        if let Some(stop_tx) = self.stop_tx.lock().await.take() {
            let _ = stop_tx.send(false);
        }

        let tasks = self.tasks.lock().await.drain(..).collect::<Vec<_>>();
        for task in tasks {
            let _ = task.await;
        }

        self.save().await?;
        tracing::info!("Companion Host stopped");
        Ok(())
    }

    /// Check if the host is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Access the World Model store.
    pub fn store(&self) -> &Arc<InMemoryWorldModelStore> {
        &self.store
    }

    /// Access the audit log.
    pub fn audit_log(&self) -> &Arc<StdMutex<AuditLog>> {
        &self.audit_log
    }

    /// Access the privacy manager.
    pub fn privacy(&self) -> &PrivacyManager {
        &self.privacy_mgr
    }

    /// Run a single self-initiated reflection tick.
    pub async fn tick(&self) -> Decision {
        self.loop_svc.lock().await.tick().await
    }

    /// Process a user observation through the cognitive cycle.
    pub async fn observe(&self, observation: &str) -> Decision {
        self.loop_svc.lock().await.cycle(observation).await
    }

    /// Return debug info about the observation loop.
    pub fn observation_debug(&self) -> Option<ObservationDebugInfo> {
        self.obsv_loop.as_ref().map(|l| l.debug_info())
    }

    /// Return Telegram adapter diagnostics, if enabled.
    pub fn telegram_stats(&self) -> Option<TelegramStats> {
        if let Ok(guard) = self.telegram.try_lock() {
            guard.as_ref().map(|a| a.stats())
        } else {
            None
        }
    }
}
