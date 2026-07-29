use ai_os_core::events::Event;
use async_trait::async_trait;
use memory_core::Timestamp;
use perception_anomaly::AnomalyDetector;
use perception_context::ContextEnricher;
use perception_core::DesktopObservationProvider;
use perception_core::*;
use perception_detector::StateDetector;
use perception_entities::EntityExtractor;
use perception_fusion::FusionEngine;
use perception_normalizer::Normalizer;
use perception_observer::Observer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::mpsc;

// ── Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    pub channel_capacity: usize,
    pub stage_timeout_ms: u64,
    pub max_stage_restarts: u32,
    pub restart_window_seconds: u64,
    pub enabled_stages: Vec<String>,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 10000,
            stage_timeout_ms: 5000,
            max_stage_restarts: 5,
            restart_window_seconds: 60,
            enabled_stages: vec![
                "observer".into(),
                "normalizer".into(),
                "detector".into(),
                "entities".into(),
                "context".into(),
                "anomaly".into(),
                "fusion".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub stage_order: Vec<String>,
    pub channel_capacity: usize,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            stage_order: vec![
                "observer".into(),
                "normalizer".into(),
                "detector".into(),
                "entities".into(),
                "context".into(),
                "anomaly".into(),
                "fusion".into(),
            ],
            channel_capacity: 10000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub running: bool,
    pub stages: Vec<StageStatus>,
    pub total_observations_processed: u64,
    pub total_observations_dropped: u64,
    pub backpressure_events: u64,
    pub uptime_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageStatus {
    pub name: String,
    pub active: bool,
    pub bypass: bool,
    pub observations_in: u64,
    pub observations_out: u64,
    pub errors: u64,
    pub p99_latency_us: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageChannelConfig {
    pub name: String,
    pub input_capacity: usize,
    pub output_capacity: usize,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageHandle {
    pub name: String,
    pub bypass: bool,
}

// ── PipelineManager trait ──────────────────────────────────

#[async_trait]
pub trait PipelineManager: Debug + Send + Sync {
    fn register_stage(&self, name: &str, config: StageChannelConfig) -> CoordinatorResult<()>;

    fn connect(&self, from: &str, to: &str, capacity: usize) -> CoordinatorResult<()>;

    fn channel_fill_levels(&self) -> HashMap<String, (usize, usize)>;

    fn set_bypass(&self, stage: &str, bypass: bool) -> CoordinatorResult<()>;

    fn remove_stage(&self, name: &str) -> CoordinatorResult<()>;
}

// ── PerceptionCoordinator trait ────────────────────────────

#[async_trait]
pub trait PerceptionCoordinator: Debug + Send + Sync {
    async fn register_observer(&self, observer: Arc<dyn Observer>) -> CoordinatorResult<()>;

    async fn register_desktop_observer(
        &self,
        observer: Arc<dyn DesktopObservationProvider>,
    ) -> CoordinatorResult<()>;

    async fn observe_desktop(&self) -> perception_core::PerceptionResult<DesktopState>;

    async fn register_normalizer(&self, normalizer: Arc<dyn Normalizer>) -> CoordinatorResult<()>;

    async fn register_state_detector(
        &self,
        detector: Arc<dyn StateDetector>,
    ) -> CoordinatorResult<()>;

    async fn register_entity_extractor(
        &self,
        extractor: Arc<dyn EntityExtractor>,
    ) -> CoordinatorResult<()>;

    async fn register_context_enricher(
        &self,
        enricher: Arc<dyn ContextEnricher>,
    ) -> CoordinatorResult<()>;

    async fn register_anomaly_detector(
        &self,
        detector: Arc<dyn AnomalyDetector>,
    ) -> CoordinatorResult<()>;

    async fn register_fusion_engine(&self, engine: Arc<dyn FusionEngine>) -> CoordinatorResult<()>;

    async fn start(&self) -> CoordinatorResult<()>;

    async fn stop(&self) -> CoordinatorResult<()>;

    async fn reload_config(&self, config: CoordinatorConfig) -> CoordinatorResult<()>;

    async fn status(&self) -> PipelineStatus;

    async fn observation_stream(&self) -> tokio::sync::mpsc::Receiver<Observation>;
}

// ── CoordinatorResult / CoordinatorError ───────────────────

pub type CoordinatorResult<T> = Result<T, CoordinatorError>;

#[derive(Debug, thiserror::Error)]
pub enum CoordinatorError {
    #[error("pipeline assembly failed: {0}")]
    AssemblyFailed(String),
    #[error("stage not found: {0}")]
    StageNotFound(String),
    #[error("stage already registered: {0}")]
    StageAlreadyRegistered(String),
    #[error("channel closed: {0}")]
    ChannelClosed(String),
    #[error("pipeline not running")]
    NotRunning,
    #[error("pipeline already running")]
    AlreadyRunning,
    #[error("configuration error: {0}")]
    ConfigError(String),
}

impl From<CoordinatorError> for perception_core::PerceptionError {
    fn from(e: CoordinatorError) -> Self {
        perception_core::PerceptionError::ConfigurationError(e.to_string())
    }
}

// ── DefaultPipelineManager ─────────────────────────────────

#[derive(Debug)]
pub struct DefaultPipelineManager {
    stages: RwLock<HashMap<String, StageChannelConfig>>,
    connections: RwLock<HashMap<String, (String, usize)>>,
    bypass: RwLock<HashMap<String, bool>>,
    fill_levels: RwLock<HashMap<String, (usize, usize)>>,
    next_stage: RwLock<HashMap<String, String>>,
}

impl DefaultPipelineManager {
    pub fn new() -> Self {
        Self {
            stages: RwLock::new(HashMap::new()),
            connections: RwLock::new(HashMap::new()),
            bypass: RwLock::new(HashMap::new()),
            fill_levels: RwLock::new(HashMap::new()),
            next_stage: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultPipelineManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PipelineManager for DefaultPipelineManager {
    fn register_stage(&self, name: &str, config: StageChannelConfig) -> CoordinatorResult<()> {
        let mut stages = self
            .stages
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        if stages.contains_key(name) {
            return Err(CoordinatorError::StageAlreadyRegistered(name.into()));
        }
        stages.insert(name.into(), config);
        Ok(())
    }

    fn connect(&self, from: &str, to: &str, capacity: usize) -> CoordinatorResult<()> {
        let mut next = self
            .next_stage
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        next.insert(from.into(), to.into());

        let mut conns = self
            .connections
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        conns.insert(from.into(), (to.into(), capacity));
        Ok(())
    }

    fn channel_fill_levels(&self) -> HashMap<String, (usize, usize)> {
        self.fill_levels
            .read()
            .map(|f| f.clone())
            .unwrap_or_default()
    }

    fn set_bypass(&self, stage: &str, bypass: bool) -> CoordinatorResult<()> {
        let mut b = self
            .bypass
            .write()
            .map_err(|_| CoordinatorError::StageNotFound(stage.into()))?;
        b.insert(stage.into(), bypass);
        Ok(())
    }

    fn remove_stage(&self, name: &str) -> CoordinatorResult<()> {
        let mut stages = self
            .stages
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        stages.remove(name);
        Ok(())
    }
}

// ── DefaultPerceptionCoordinator ───────────────────────────

#[derive(Debug)]
pub struct DefaultPerceptionCoordinator {
    config: RwLock<CoordinatorConfig>,
    pipeline_manager: Arc<dyn PipelineManager>,
    observer: RwLock<Option<Arc<dyn Observer>>>,
    desktop_observer: RwLock<Option<Arc<dyn DesktopObservationProvider>>>,
    normalizer: RwLock<Option<Arc<dyn Normalizer>>>,
    state_detector: RwLock<Option<Arc<dyn StateDetector>>>,
    entity_extractor: RwLock<Option<Arc<dyn EntityExtractor>>>,
    context_enricher: RwLock<Option<Arc<dyn ContextEnricher>>>,
    anomaly_detector: RwLock<Option<Arc<dyn AnomalyDetector>>>,
    fusion_engine: RwLock<Option<Arc<dyn FusionEngine>>>,
    running: AtomicBool,
    start_time: RwLock<Option<Timestamp>>,
    total_processed: AtomicU64,
    total_dropped: AtomicU64,
    backpressure_events: AtomicU64,
    stage_statuses: RwLock<HashMap<String, StageStatus>>,
    output_tx: RwLock<Option<mpsc::Sender<Observation>>>,
}

impl DefaultPerceptionCoordinator {
    pub fn new(config: CoordinatorConfig) -> Self {
        let manager = Arc::new(DefaultPipelineManager::new());

        let stage_names = [
            "observer",
            "normalizer",
            "detector",
            "entities",
            "context",
            "anomaly",
            "fusion",
        ];

        let mut statuses = HashMap::new();
        for name in &stage_names {
            let active = config.enabled_stages.contains(&name.to_string());
            statuses.insert(
                name.to_string(),
                StageStatus {
                    name: name.to_string(),
                    active,
                    bypass: false,
                    observations_in: 0,
                    observations_out: 0,
                    errors: 0,
                    p99_latency_us: 0.0,
                },
            );

            let _ = manager.register_stage(
                name,
                StageChannelConfig {
                    name: name.to_string(),
                    input_capacity: config.channel_capacity,
                    output_capacity: config.channel_capacity,
                    timeout_ms: config.stage_timeout_ms,
                },
            );
        }

        for i in 0..stage_names.len() - 1 {
            let _ = manager.connect(stage_names[i], stage_names[i + 1], config.channel_capacity);
        }

        Self {
            config: RwLock::new(config),
            pipeline_manager: manager,
            observer: RwLock::new(None),
            desktop_observer: RwLock::new(None),
            normalizer: RwLock::new(None),
            state_detector: RwLock::new(None),
            entity_extractor: RwLock::new(None),
            context_enricher: RwLock::new(None),
            anomaly_detector: RwLock::new(None),
            fusion_engine: RwLock::new(None),
            running: AtomicBool::new(false),
            start_time: RwLock::new(None),
            total_processed: AtomicU64::new(0),
            total_dropped: AtomicU64::new(0),
            backpressure_events: AtomicU64::new(0),
            stage_statuses: RwLock::new(statuses),
            output_tx: RwLock::new(None),
        }
    }

    fn stage_status_mut(&self, name: &str) -> Option<StageStatus> {
        self.stage_statuses
            .read()
            .ok()
            .and_then(|s| s.get(name).cloned())
    }

    fn update_stage_status(&self, name: &str, f: impl FnOnce(&mut StageStatus)) {
        if let Ok(mut statuses) = self.stage_statuses.write() {
            if let Some(s) = statuses.get_mut(name) {
                f(s);
            }
        }
    }
}

#[async_trait]
impl PerceptionCoordinator for DefaultPerceptionCoordinator {
    async fn register_observer(&self, observer: Arc<dyn Observer>) -> CoordinatorResult<()> {
        let mut o = self
            .observer
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        *o = Some(observer);
        Ok(())
    }

    async fn register_desktop_observer(
        &self,
        observer: Arc<dyn DesktopObservationProvider>,
    ) -> CoordinatorResult<()> {
        let mut d = self
            .desktop_observer
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        *d = Some(observer);
        Ok(())
    }

    async fn observe_desktop(&self) -> perception_core::PerceptionResult<DesktopState> {
        let observer = self
            .desktop_observer
            .read()
            .map_err(|_| {
                perception_core::PerceptionError::ConfigurationError("lock poisoned".into())
            })?
            .clone()
            .ok_or_else(|| {
                perception_core::PerceptionError::ConfigurationError(
                    "no desktop observer registered".into(),
                )
            })?;
        observer.observe().await
    }

    async fn register_normalizer(&self, normalizer: Arc<dyn Normalizer>) -> CoordinatorResult<()> {
        let mut n = self
            .normalizer
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        *n = Some(normalizer);
        Ok(())
    }

    async fn register_state_detector(
        &self,
        detector: Arc<dyn StateDetector>,
    ) -> CoordinatorResult<()> {
        let mut d = self
            .state_detector
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        *d = Some(detector);
        Ok(())
    }

    async fn register_entity_extractor(
        &self,
        extractor: Arc<dyn EntityExtractor>,
    ) -> CoordinatorResult<()> {
        let mut e = self
            .entity_extractor
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        *e = Some(extractor);
        Ok(())
    }

    async fn register_context_enricher(
        &self,
        enricher: Arc<dyn ContextEnricher>,
    ) -> CoordinatorResult<()> {
        let mut c = self
            .context_enricher
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        *c = Some(enricher);
        Ok(())
    }

    async fn register_anomaly_detector(
        &self,
        detector: Arc<dyn AnomalyDetector>,
    ) -> CoordinatorResult<()> {
        let mut d = self
            .anomaly_detector
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        *d = Some(detector);
        Ok(())
    }

    async fn register_fusion_engine(&self, engine: Arc<dyn FusionEngine>) -> CoordinatorResult<()> {
        let mut f = self
            .fusion_engine
            .write()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?;
        *f = Some(engine);
        Ok(())
    }

    async fn start(&self) -> CoordinatorResult<()> {
        if self.running.load(Ordering::SeqCst) {
            return Err(CoordinatorError::AlreadyRunning);
        }

        let has_observer = self
            .observer
            .read()
            .map_err(|_| CoordinatorError::AssemblyFailed("lock poisoned".into()))?
            .is_some();
        if !has_observer {
            return Err(CoordinatorError::AssemblyFailed(
                "no observer registered".into(),
            ));
        }

        self.running.store(true, Ordering::SeqCst);
        if let Ok(mut st) = self.start_time.write() {
            *st = Some(Timestamp::now());
        }

        for name in &[
            "observer",
            "normalizer",
            "detector",
            "entities",
            "context",
            "anomaly",
            "fusion",
        ] {
            self.update_stage_status(name, |s| {
                s.active = true;
            });
        }

        Ok(())
    }

    async fn stop(&self) -> CoordinatorResult<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Err(CoordinatorError::NotRunning);
        }

        self.running.store(false, Ordering::SeqCst);

        for name in &[
            "observer",
            "normalizer",
            "detector",
            "entities",
            "context",
            "anomaly",
            "fusion",
        ] {
            self.update_stage_status(name, |s| {
                s.active = false;
            });
        }

        Ok(())
    }

    async fn reload_config(&self, config: CoordinatorConfig) -> CoordinatorResult<()> {
        if let Ok(mut c) = self.config.write() {
            *c = config;
        }
        Ok(())
    }

    async fn status(&self) -> PipelineStatus {
        let stages: Vec<StageStatus> = self
            .stage_statuses
            .read()
            .map(|s| s.values().cloned().collect())
            .unwrap_or_default();

        let uptime = self
            .start_time
            .read()
            .ok()
            .and_then(|g| *g)
            .map(|t| {
                let now = Timestamp::now();
                (now.as_nanos() - t.as_nanos()) as f64 / 1_000_000_000.0
            })
            .unwrap_or(0.0);

        PipelineStatus {
            running: self.running.load(Ordering::SeqCst),
            stages,
            total_observations_processed: self.total_processed.load(Ordering::SeqCst),
            total_observations_dropped: self.total_dropped.load(Ordering::SeqCst),
            backpressure_events: self.backpressure_events.load(Ordering::SeqCst),
            uptime_seconds: uptime,
        }
    }

    async fn observation_stream(&self) -> tokio::sync::mpsc::Receiver<Observation> {
        let (tx, rx) = mpsc::channel(10000);
        if let Ok(mut output) = self.output_tx.write() {
            *output = Some(tx);
        }
        rx
    }
}

// ── Coordinator events ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStarted {
    pub config: CoordinatorConfig,
    pub timestamp: Timestamp,
}

impl Event for PipelineStarted {
    fn event_type(&self) -> &'static str {
        "perception.pipeline.started"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStopped {
    pub timestamp: Timestamp,
}

impl Event for PipelineStopped {
    fn event_type(&self) -> &'static str {
        "perception.pipeline.stopped"
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_new_not_running() {
        let config = CoordinatorConfig::default();
        let coord = DefaultPerceptionCoordinator::new(config);
        let status = coord.status().await;
        assert!(!status.running);
    }

    #[tokio::test]
    async fn test_coordinator_start_without_observer_fails() {
        let config = CoordinatorConfig::default();
        let coord = DefaultPerceptionCoordinator::new(config);
        let result = coord.start().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_coordinator_stop_without_start_fails() {
        let config = CoordinatorConfig::default();
        let coord = DefaultPerceptionCoordinator::new(config);
        let result = coord.stop().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_coordinator_reload_config() {
        let config = CoordinatorConfig::default();
        let coord = DefaultPerceptionCoordinator::new(config);
        let new_config = CoordinatorConfig {
            channel_capacity: 5000,
            ..CoordinatorConfig::default()
        };
        coord.reload_config(new_config).await.unwrap();
    }

    #[tokio::test]
    async fn test_pipeline_manager_register_stage() {
        let manager = DefaultPipelineManager::new();
        let config = StageChannelConfig {
            name: "test".into(),
            input_capacity: 100,
            output_capacity: 100,
            timeout_ms: 1000,
        };
        manager.register_stage("test", config).unwrap();
        assert!(manager
            .register_stage(
                "test",
                StageChannelConfig {
                    name: "test".into(),
                    input_capacity: 200,
                    output_capacity: 200,
                    timeout_ms: 2000,
                }
            )
            .is_err());
    }

    #[tokio::test]
    async fn test_pipeline_manager_connect() {
        let manager = DefaultPipelineManager::new();
        manager.connect("stage_a", "stage_b", 100).unwrap();
        let levels = manager.channel_fill_levels();
        assert!(levels.is_empty());
    }

    #[tokio::test]
    async fn test_pipeline_manager_bypass() {
        let manager = DefaultPipelineManager::new();
        manager.set_bypass("normalizer", true).unwrap();
    }

    #[tokio::test]
    async fn test_pipeline_manager_remove_stage() {
        let manager = DefaultPipelineManager::new();
        let config = StageChannelConfig {
            name: "test".into(),
            input_capacity: 100,
            output_capacity: 100,
            timeout_ms: 1000,
        };
        manager.register_stage("test", config).unwrap();
        manager.remove_stage("test").unwrap();
    }

    #[tokio::test]
    async fn test_coordinator_status_default() {
        let config = CoordinatorConfig::default();
        let coord = DefaultPerceptionCoordinator::new(config);
        let status = coord.status().await;
        assert_eq!(status.stages.len(), 7);
        assert_eq!(status.total_observations_processed, 0);
    }
}
